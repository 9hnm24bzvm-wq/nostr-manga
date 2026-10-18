// Finite source/config seams. This is not nginx -t or a live proxy execution.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
const root = new URL('../../', import.meta.url);
const config = fs.readFileSync(new URL('deployment/nginx.example.conf', root), 'utf8');
const router = fs.readFileSync(new URL('backend/src/router.rs', root), 'utf8');
const defaults = fs.readFileSync(new URL('backend/src/config.rs', root), 'utf8');

// A bounded directive/block reader, honoring quoted regex braces and comments.
// Its accepted input is this example only, not arbitrary nginx configuration.
function parse(source) {
  const tokens = []; let word = '', started = false, quoted = false, quote = '', comment = false;
  const flush = () => { if (started) tokens.push({ value: word, quoted }); word = ''; started = quoted = false; };
  for (let i = 0; i < source.length; i++) {
    const c = source[i];
    if (comment) { if (c === '\n') comment = false; else continue; }
    if (quote) {
      if (c === quote) quote = '';
      else if (c === '\\' && [quote, '\\'].includes(source[i + 1])) word += source[++i];
      else word += c;
    } else if (c === '"' || c === "'") { quote = c; started = quoted = true; }
    else if (c === '#') { flush(); comment = true; }
    else if (/\s/.test(c)) flush();
    else if ('{};'.includes(c)) { flush(); tokens.push({ symbol: c }); }
    else { started = true; word += c; }
  }
  assert.equal(quote, '', 'unterminated quote'); flush(); let i = 0;
  function block(nested = false) {
    const nodes = [];
    while (i < tokens.length) {
      if (tokens[i].symbol === '}') { assert.ok(nested, 'unexpected closing brace'); i++; return nodes; }
      const args = [];
      while (i < tokens.length && !tokens[i].symbol) args.push(tokens[i++]);
      assert.ok(args.length && i < tokens.length, 'unterminated directive');
      const symbol = tokens[i++].symbol;
      if (symbol === ';') nodes.push({ args });
      else { assert.equal(symbol, '{', 'unterminated directive before closing brace'); nodes.push({ args, children: block(true) }); }
    }
    assert.equal(nested, false, 'unterminated block'); return nodes;
  }
  return block();
}
const ast = parse(config);
const values = node => node.args.map(x => x.value);
const directive = (node, name) => node.children.find(x => x.args[0].value === name);
const servers = ast.filter(x => x.args[0].value === 'server');
const publicServer = servers.find(x => values(directive(x, 'listen')).includes('443') && values(directive(x, 'server_name')).includes('weebdex.example.invalid'));
const nativeServer = servers.find(x => values(directive(x, 'listen')).includes('443') && values(directive(x, 'server_name')).includes('protocol.example.invalid'));
const locations = publicServer.children.filter(x => x.args[0].value === 'location');
function select(uri, rows = locations) {
  const path = uri.split('?')[0];
  const regex = rows.find(x => x.args[1].value === '~' && new RegExp(x.args[2].value).test(path));
  return regex ?? rows.filter(x => x.args.length === 2 && path.startsWith(x.args[1].value)).sort((a, b) => b.args[1].value.length - a.args[1].value.length)[0];
}
function budget(location) {
  const size = directive(location, 'client_max_body_size') ?? directive(publicServer, 'client_max_body_size');
  const time = directive(location, 'client_body_timeout') ?? directive(publicServer, 'client_body_timeout');
  assert.match(size.args[1].value, /^\d+m$/); assert.match(time.args[1].value, /^\d+s$/);
  return [Number.parseInt(size.args[1].value) * 1024 * 1024, Number.parseInt(time.args[1].value)];
}
const uuid = '[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}';
const ids = ['11111111-2222-8333-8444-555555555555', 'AAAAAAAA-BBBB-8CCC-8DDD-EEEEEEEEEEEE', '00000000-0000-0000-0000-000000000000'];
const patterns = [new RegExp(`^/api/v1/upload/${uuid}$`), new RegExp(`^/api/v1/nostr/uploads/${uuid}/files/${uuid}/replace$`), new RegExp(`^/api/v1/cover/${uuid}$`)];
const signedPaths = id => [`/api/v1/upload/${id}`, `/api/v1/nostr/uploads/${id}/files/${ids[0]}/replace`, `/api/v1/cover/${id}`];
const uploadSeconds = Number(defaults.match(/upload_body_timeout_seconds:\s*(\d+),/)[1]);
const jsonSeconds = Number(defaults.match(/json_body_timeout_seconds:\s*(\d+),/)[1]);
const jsonBytes = 1024 * 1024;

test('all seven regex locations quote brace quantifiers as one nginx argument', () => {
  const rows = locations.filter(x => x.args[1].value === '~'); assert.equal(rows.length, 7);
  for (const row of rows) { assert.equal(row.args.length, 3); assert.equal(row.args[2].quoted, true); assert.ok(row.children.length); }
  assert.throws(() => parse('location ~ ^/api/upload/[A-Za-z0-9]{10}$ { proxy_pass http://upstream; }'), /unterminated directive/);
});
test('source fence binds exact signed Rust routes, default deadlines and budgets', () => {
  assert.match(router, /\["api", "v1", "nostr", "uploads", session, "files", file, "replace"\]/);
  assert.match(router, /Method::POST && signed_upload_replacement_path\(&parts\)[\s\S]*?return \(40 \* 1024 \* 1024, Duration::from_secs\(state.config.upload_body_timeout_seconds\)\)/);
  for (const [kind, size] of [['cover', 40], ['upload', 96]]) assert.match(router, new RegExp(`parts\\[\\.\\.3\\] == \\["api", "v1", "${kind}"\\][\\s\\S]*?return \\(${size} \\* 1024 \\* 1024, Duration::from_secs\\(state.config.upload_body_timeout_seconds\\)\\)`));
  assert.equal(uploadSeconds, 180); assert.equal(jsonSeconds, 30); assert.match(defaults, /maximum_json_bytes: 1024 \* 1024,/);
  assert.match(fs.readFileSync(new URL('backend/src/public_api/uploads.rs', root), 'utf8'), /\.route\("\/upload\/\{p0\}", post\(upload\)/);
  assert.match(fs.readFileSync(new URL('backend/src/public_api/upload_extensions.rs', root), 'utf8'), /\.route\("\/nostr\/uploads\/\{session\}\/files\/\{file\}\/replace", post\(replace\)/);
  assert.match(fs.readFileSync(new URL('backend/src/public_api/cover_writes.rs', root), 'utf8'), /\.route\("\/cover\/\{p0\}",post\(upload\)\.put\(update\)\.delete\(remove\)\)/);
});
for (const id of ids) test(`signed upload, replacement and cover proxy ceilings match Rust for ${id}`, () => {
  for (const [i, path] of signedPaths(id).entries()) {
    assert.deepEqual(budget(select(path)), [[96, 40, 40][i] * 1024 * 1024, uploadSeconds]);
    assert.deepEqual(budget(select(path + '?expectedAccountId=' + ids[0])), budget(select(path)));
  }
});
test('legacy short-ID routes retain their supported ceilings and methods', () => {
  for (const [path, bytes] of [['/api/upload/Abc123Def0', 96], ['/api/upload/Abc123Def0/files/Ghi456Jkl0/replace', 40], ['/api/cover/Abc123Def0', 40]]) assert.deepEqual(budget(select(path)), [bytes * 1024 * 1024, uploadSeconds]);
  assert.deepEqual(values(directive(select('/originals/' + 'a'.repeat(64)), 'limit_except')), ['limit_except', 'GET']);
});
test('JSON, control, malformed, preview and lookalike paths retain one MiB/30s proxy limits', () => {
  const id = ids[0];
  for (const path of ['/api/upload/begin', '/api/v1/upload/begin', '/api/v1/upload/check-approval-required', `/api/v1/upload/begin/${id}`, `/api/v1/upload/${id}/commit`, `/api/v1/upload/${id}/batch`, `/api/v1/nostr/uploads/${id}`, `/api/v1/nostr/uploads/${id}/files/${id}/preview`, `/api/v1/upload/${id}/`, `/api/v1/cover/${id}/other`, `/api/v1/upload/${id.replace('1', 'g')}`, `/api/v1/nostr/uploads/${id}/files/Abc123Def0/replace`, `/api/v1/upload/${id}/files/${id}/replace`, '/api/v1/cover/Abc123Def0', '/api/v1/manga', '/auth/self-service/login']) assert.deepEqual(budget(select(path)), [jsonBytes, jsonSeconds], path);
});
test('new proxy locations preserve transport and leave non-POST JSON guards in Rust', () => {
  for (const path of signedPaths(ids[0])) {
    const row = select(path); assert.deepEqual(values(directive(row, 'proxy_pass')), ['proxy_pass', 'http://weebdex_platform']);
    assert.equal(directive(row, 'limit_except'), undefined); assert.equal(directive(row, 'rewrite'), undefined);
    assert.deepEqual(values(directive(row, 'limit_req')), ['limit_req', 'zone=weebdex_api', 'burst=40', 'nodelay']);
    assert.ok(row.children.every(x => x.args[0].value !== 'proxy_set_header'));
  }
  assert.match(router, /if \*method == Method::POST && signed_upload_replacement_path/);
  assert.match(router, /if \*method == Method::POST && parts.len\(\) == 4/);
  assert.match(router, /state.config.maximum_json_bytes,\s*Duration::from_secs\(state.config.json_body_timeout_seconds\)/);
});
test('origin forwarding, streaming, native WebSocket, admin restriction and safe logs remain', () => {
  for (const [name, value] of [['proxy_request_buffering', 'off'], ['proxy_buffering', 'off'], ['proxy_intercept_errors', 'off']]) assert.equal(directive(publicServer, name).args[1].value, value);
  const headers = publicServer.children.filter(x => x.args[0].value === 'proxy_set_header').map(values);
  for (const pair of [['Host', '$http_host'], ['X-Forwarded-Proto', 'https'], ['X-Forwarded-For', '$remote_addr']]) assert.ok(headers.some(x => x[1] === pair[0] && x[2] === pair[1]));
  assert.ok(headers.every(x => !['Authorization', 'Cookie', 'Range', 'If-Range', 'If-None-Match', 'X-WeebDex-Upload-Version'].includes(x[1])));
  const admin = nativeServer.children.find(x => x.args[0].value === 'location' && x.args[1].value === '/admin/'); assert.ok(admin.children.some(x => values(x).join(' ') === 'deny all'));
  const nativeRoot = nativeServer.children.find(x => x.args[0].value === 'location' && x.args[1].value === '/'); assert.ok(nativeRoot.children.some(x => values(x).join(' ') === 'proxy_set_header Upgrade $http_upgrade'));
  const log = ast.find(x => x.args[0].value === 'log_format'); for (const field of ['$args', '$request_uri', '$http_authorization', '$http_cookie', '$request_body']) assert.ok(!log.args.some(x => x.value.includes(field)));
});
test('pre-fix omitted signed locations reproduce the one MiB rejection witness', () => {
  const omitted = locations.filter(row => !(row.args[1].value === '~' && patterns.some(pattern => pattern.source === new RegExp(row.args[2].value).source)));
  assert.equal(omitted.length, locations.length - 3);
  for (const path of signedPaths(ids[0])) assert.deepEqual(budget(select(path, omitted)), [jsonBytes, jsonSeconds]);
});
test('native root and restricted admin preserve the complete effective header set', () => {
  for (const path of ['/', '/admin/']) {
    const row = nativeServer.children.find(x => x.args[0].value === 'location' && x.args[1].value === path);
    const own = row.children.filter(x => x.args[0].value === 'proxy_set_header');
    // nginx inherits the entire parent set only if the child defines none.
    const effective = (own.length ? own : nativeServer.children.filter(x => x.args[0].value === 'proxy_set_header')).map(values);
    for (const [key, value] of [['Host', '$http_host'], ['X-Forwarded-Proto', 'https'], ['X-Forwarded-For', '$remote_addr']]) assert.ok(effective.some(x => x[1] === key && x[2] === value), path + ' ' + key);
    assert.ok(effective.some(x => x[1] === 'Connection' && x[2] === (path === '/' ? '$weebdex_connection' : '')));
    if (path === '/') assert.ok(effective.some(x => x[1] === 'Upgrade' && x[2] === '$http_upgrade'));
    else assert.ok(row.children.some(x => values(x).join(' ') === 'deny all'));
    assert.ok(effective.every(x => !['Authorization','Cookie','Range','If-Range','If-None-Match','X-WeebDex-Upload-Version'].includes(x[1])));
  }
});

test('HTTP redirect disables inherited request logging while preserving exact redirect URI', () => {
  const redirect = servers.find(row => values(directive(row, 'listen')).includes('80'));
  assert.deepEqual(values(directive(redirect, 'access_log')), ['access_log', 'off']);
  assert.deepEqual(values(directive(redirect, 'return')), ['return', '308', 'https://$host$request_uri']);
  const parentCombined = ['access_log', '/var/log/nginx/access.log', 'combined'];
  const effective = row => directive(row, 'access_log') ? values(directive(row, 'access_log')) : parentCombined;
  assert.deepEqual(effective(redirect), ['access_log', 'off']);
  const previous = { ...redirect, children: redirect.children.filter(row => row.args[0].value !== 'access_log') };
  assert.deepEqual(effective(previous), parentCombined, 'omitted directive recreates sensitive default inheritance');
});
