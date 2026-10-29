// Source-only until the parent releases a browser/runtime window.
// Exports an owned loopback server; never launches a browser or contacts a relay.
import fs from 'node:fs';
import path from 'node:path';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
const here = path.dirname(fileURLToPath(import.meta.url)), root = path.resolve(here, '../../..');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
export async function createRemoteSignerBrowserFixture({ frontend, domDependencies }) {
  frontend = path.resolve(frontend); domDependencies = path.resolve(domDependencies);
  const require = createRequire(path.join(frontend, 'package.json'));
  const { build } = await import(pathToFileURL(require.resolve('vite')));
  const { svelte } = await import(pathToFileURL(require.resolve('@sveltejs/vite-plugin-svelte')));
  const { WebSocketServer } = require(path.join(domDependencies, 'ws'));
  const deps = path.join(root, '.build/remote-signer-nostr-tools-2.25.2/node_modules');
  const { schnorr } = await import(pathToFileURL(path.join(deps, '@noble/curves/secp256k1.js')));
  const { sha256 } = await import(pathToFileURL(path.join(deps, '@noble/hashes/sha2.js')));
  const { v2: nip44 } = await import(pathToFileURL(path.join(deps, 'nostr-tools/lib/esm/nip44.js')));
  const stage = fs.mkdtempSync(path.join(root, '.build/remote-browser-owned-'));
  const owned = path.join(root, 'platform-integrations/local-library'), boundary = path.join(here, 'boundaries.ts');
  fs.symlinkSync(path.join(frontend, 'node_modules'), path.join(stage, 'node_modules'));
  fs.writeFileSync(path.join(stage, 'package.json'), '{"type":"module"}');
  fs.writeFileSync(path.join(stage, 'index.html'), '<!doctype html><meta charset="utf-8"><title>Owned remote signer fixture</title><div id="app"></div><script type="module" src="/entry.js"></script>');
  fs.writeFileSync(path.join(stage, 'entry.js'), `import {mount} from 'svelte';import App from ${JSON.stringify(path.join(here, 'App.svelte'))};mount(App,{target:document.querySelector('#app')});`);
  const sources = ['RemoteSignerPanel.svelte', 'RemoteSignerStatus.svelte', 'remote-signer-session.ts', 'remote-bunker.ts', 'public-identity-input.ts', 'protocol/nostr.js', 'protocol/codec.js'].map(name => ({ path: 'platform-integrations/local-library/' + name, sha256: sha(fs.readFileSync(path.join(owned, name))) }));
  for (const area of ['dialog', 'button', 'input']) for (const name of fs.readdirSync(path.join(frontend, 'src/lib/components/ui', area)).sort()) { const file = path.join(frontend, 'src/lib/components/ui', area, name); if (fs.statSync(file).isFile()) sources.push({ path: 'qualified-frontend/src/lib/components/ui/' + area + '/' + name, sha256: sha(fs.readFileSync(file)) }); }
  for (const name of ['App.svelte', 'boundaries.ts', 'fixture.mjs', 'checks.mjs', 'control.js', 'launch.mjs']) sources.push({ path: 'scripts/ci/remote-signer-browser/' + name, sha256: sha(fs.readFileSync(path.join(here, name))) });
  let server, ws, origin, stopped = false;
  const sockets = new Set(), held = [], conversations = new Set();
  let account = 'SyntheticAccountA', revision = 1, mode = 'hold-public-key', connections = 0, activeConnections = 0, requests = 0, releases = 0, failures = 0, pending = 0;
  // Synthetic server keys exist only in this owned process. Never returned/logged.
  const bunkerSecret = schnorr.utils.randomSecretKey(), userSecret = schnorr.utils.randomSecretKey();
  const hex = value => Buffer.from(value).toString('hex'), bunkerKey = hex(schnorr.getPublicKey(bunkerSecret)), userKey = hex(schnorr.getPublicKey(userSecret));
  const sign = template => { const event = { ...template, pubkey: bunkerKey }; const digest = sha256(new TextEncoder().encode(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]))); return { ...event, id: hex(digest), sig: hex(schnorr.sign(digest, bunkerSecret)) }; };
  const snapshot = () => ({ connections, activeConnections, httpSockets: sockets.size, requests, pending, releases, failures, accountChanged: account !== 'SyntheticAccountA', linkChanged: revision !== 1, stopped });
  function release() { releases++; for (const fn of held.splice(0)) fn(); }
  async function close() { if (stopped) return; stopped = true; held.length = 0; for (const conversation of conversations) conversation.fill(0); conversations.clear(); for (const socket of ws?.clients ?? []) socket.terminate(); for (const socket of sockets) socket.destroy(); if (ws) await new Promise(resolve => ws.close(resolve)); if (server?.listening) await new Promise(resolve => server.close(resolve)); bunkerSecret.fill(0); userSecret.fill(0); fs.rmSync(stage, { recursive: true, force: true }); }
  try {
    await build({ configFile: false, root: stage, logLevel: 'error', publicDir: false, plugins: [{ name: 'explicit-synthetic-native-boundaries', enforce: 'pre', resolveId(source, importer) { if (source === '$lib/stores' || importer?.startsWith(owned) && ['./publication', './native-identity-link', './protocol-identity'].includes(source)) return boundary; } }, svelte({ configFile: false })], resolve: { dedupe: ['svelte'], alias: [{ find: '@owned', replacement: owned }, { find: '$lib/stores', replacement: boundary }, { find: '$lib', replacement: path.join(frontend, 'src/lib') }, { find: 'nostr-tools/nip44', replacement: path.join(deps, 'nostr-tools/lib/esm/nip44.js') }, { find: '@noble/curves', replacement: path.join(deps, '@noble/curves') }, { find: '@noble/hashes', replacement: path.join(deps, '@noble/hashes') }, { find: '@scure/base', replacement: path.join(deps, '@scure/base/index.js') }] }, build: { outDir: path.join(stage, 'dist'), emptyOutDir: true, minify: false } });
    server = createServer((req, res) => {
      try {
        const url = new URL(req.url, origin);
        if (req.headers.host !== new URL(origin).host || url.origin !== origin || req.headers.origin && req.headers.origin !== origin) { res.writeHead(403).end(); return; }
        res.setHeader('cache-control', 'no-store');
        res.setHeader('content-security-policy', `default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' ${origin.replace('http:', 'ws:')}; img-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`);
        if (req.method === 'POST' && url.pathname === '/fixture/control' && req.headers.origin === origin && req.headers['content-type'] === 'application/json') {
          let body = ''; req.on('data', chunk => { body += chunk; if (body.length > 256) req.destroy(); }); req.on('end', () => { try { const value = JSON.parse(body); if (Object.keys(value).join(',') !== 'action') throw Error(); if (value.action === 'release') release(); else if (value.action === 'account') account = account === 'SyntheticAccountA' ? 'SyntheticAccountB' : 'SyntheticAccountA'; else if (value.action === 'link') revision++; else if (value.action === 'hold') mode = 'hold-public-key'; else if (value.action === 'normal') mode = 'normal'; else throw Error(); res.setHeader('content-type', 'application/json'); res.end(JSON.stringify(snapshot())); } catch { res.writeHead(400).end(); } }); return;
        }
        if (req.method !== 'GET') { res.writeHead(403).end(); return; }
        if (url.pathname === '/fixture/state') { res.setHeader('content-type', 'application/json'); res.end(JSON.stringify({ account, revision, linkKey: userKey, relayOrigin: origin.replace('http:', 'ws:') })); return; }
        if (url.pathname === '/fixture/public-link') { res.setHeader('content-type', 'application/json'); res.end(JSON.stringify({ link: `bunker://${bunkerKey}?relay=${encodeURIComponent(origin.replace('http:', 'ws:') + '/relay')}` })); return; }
        if (url.pathname === '/fixture/observations') { res.setHeader('content-type', 'application/json'); res.end(JSON.stringify(snapshot())); return; }
        if (url.pathname === '/controls') { res.setHeader('content-type', 'text/html'); res.end('<!doctype html><meta charset="utf-8"><title>Owned synthetic controls</title><h1>Owned synthetic relay/account controls</h1><p>This separate tab controls only the disposable fixture. No real account or identity is changed.</p><button data-action="hold">Hold next public-key reply</button><button data-action="normal">Reply normally</button><button data-action="release">Release held reply</button><button data-action="account">Switch synthetic native account</button><button data-action="link">Change synthetic linked revision</button><button id="refresh">Refresh redacted observations</button><pre><output></output></pre><script type="module" src="/control.js"></script>'); return; }
        if (url.pathname === '/control.js') { res.setHeader('content-type', 'text/javascript'); res.end(fs.readFileSync(path.join(here, 'control.js'))); return; }
        const relative = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
        if (!/^(?:index\.html|assets\/[A-Za-z0-9_.-]+\.(?:js|css))$/.test(relative)) { res.writeHead(404).end(); return; }
        res.setHeader('content-type', relative.endsWith('.js') ? 'text/javascript' : relative.endsWith('.css') ? 'text/css' : 'text/html'); res.end(fs.readFileSync(path.join(stage, 'dist', relative)));
      } catch { failures++; res.writeHead(500).end(); }
    });
    server.on('connection', socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
    server.on('connect', (_request, socket) => socket.destroy());
    ws = new WebSocketServer({ noServer: true, maxPayload: 65536 });
    server.on('upgrade', (req, socket, head) => { if (req.url !== '/relay' || req.headers.host !== new URL(origin).host || req.headers.origin !== origin) { socket.destroy(); return; } ws.handleUpgrade(req, socket, head, value => ws.emit('connection', value)); });
    ws.on('connection', socket => { connections++; activeConnections++; socket.once('close', () => activeConnections--); let subscription;
      socket.on('message', raw => { try {
        const frame = JSON.parse(raw.toString());
        if (frame[0] === 'REQ') { subscription = frame[1]; return; }
        if (frame[0] === 'CLOSE') return;
        if (frame[0] !== 'EVENT' || !subscription) throw Error();
        const event = frame[1], digest = sha256(new TextEncoder().encode(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content])));
        if (event.kind !== 24133 || event.id !== hex(digest) || JSON.stringify(event.tags) !== JSON.stringify([['p', bunkerKey]]) || !schnorr.verify(Buffer.from(event.sig, 'hex'), digest, Buffer.from(event.pubkey, 'hex'))) throw Error();
        const conversation = nip44.utils.getConversationKey(bunkerSecret, event.pubkey); conversations.add(conversation); const request = JSON.parse(nip44.decrypt(event.content, conversation)); requests++;
        const reply = result => { const content = nip44.encrypt(JSON.stringify({ id: request.id, result }), conversation); if (socket.readyState === 1) socket.send(JSON.stringify(['EVENT', subscription, sign({ kind: 24133, created_at: Math.floor(Date.now() / 1000), tags: [['p', event.pubkey]], content })])); conversation.fill(0); conversations.delete(conversation); };
        if (request.method === 'connect') { if (request.params[0] !== bunkerKey || request.params[1] !== '') throw Error(); reply('ack'); }
        else if (request.method === 'get_public_key') { if (mode === 'hold-public-key') { pending++; held.push(() => { pending--; reply(userKey); }); } else reply(userKey); }
        else if (request.method === 'switch_relays') reply('null');
        else throw Error('No application-event signing is available in this focus fixture.');
      } catch { failures++; socket.close(); } });
    });
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
    origin = `http://127.0.0.1:${server.address().port}`;
    return { origin, bunkerLink: `bunker://${bunkerKey}?relay=${encodeURIComponent(origin.replace('http:', 'ws:') + '/relay')}`, sources,
      snapshot, release,
      setMode(value) { if (!['hold-public-key', 'normal'].includes(value)) throw Error('Unknown owned case'); mode = value; },
      switchAccount() { account = 'SyntheticAccountB'; }, changeLink() { revision++; }, close };
  } catch (error) { await close(); throw error; }
}
