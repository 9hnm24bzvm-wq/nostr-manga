// Source-bound controls, authored only until separate execution admission.
// Node --experimental-vm-modules --test --test-concurrency=1. No dependencies,
// browser, socket, cookie jar or server. Synthetic fetch cannot prove cookies.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
const sourceUrl = new URL('../../platform-integrations/public-api/owned-json.ts', import.meta.url);
const beforeUrl = new URL('../fixtures/bounded-reader/owned-json-before-native-completion.ts.txt', import.meta.url);
const nativeEndpoint = { origin: 'https://native.test:8443', base: '/native/api', profile: 'native-aggregate-v1' };
const input = { mangaId: 'manga:One', ids: ['chapter:3', 'chapter-1', 'chapter_2'], accountId: 'account:One', accountEpoch: 4,
  catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
const limits = { bytes: 16384, fragments: 4096, emptyFragments: 128, deadlineMs: 10000, cleanupMs: 250 };
const encoded = value => new TextEncoder().encode(JSON.stringify(value));
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const turns = async () => { for (let i = 0; i < 100; i++) await Promise.resolve(); };
async function fixture(options = {}) {
  const calls = [], allocations = [], timers = new Map(), observed = { read: 0, cancel: 0, release: 0 }, unhandled = [];
  let endpoint = { ...nativeEndpoint }, current = true, sourceOrigin = 'https://page.test', responseIndex = 0;
  const source = fs.readFileSync(options.old ? beforeUrl : sourceUrl, 'utf8');
  const context = vm.createContext({ URL, URLSearchParams, AbortController, TextDecoder, setTimeout, clearTimeout,
    window: { location: { origin: sourceOrigin } }, Uint8Array: new Proxy(Uint8Array, { construct(target, args) { allocations.push(args[0]); return Reflect.construct(target, args); } }) });
  const mod = new vm.SourceTextModule(stripTypeScriptTypes(source, { mode: 'transform' }), { context }); await mod.link(() => { throw Error('Unexpected import'); }); await mod.evaluate();
  const check = () => { if (!current) throw Error('Caller authority changed'); };
  const deps = {
    setTimer(fn, ms) { const token = {}; timers.set(token, { fn, ms }); return token; }, clearTimer(token) { timers.delete(token); },
    async fetch(url, init) {
      calls.push({ url, init }); const slot = responseIndex++;
      if (slot === 0 && options.fetch) await options.fetch.promise;
      if (slot === 0 && options.fetchError) throw options.fetchError;
      let index = 0; const parts = slot === 0 ? options.parts ?? [encoded({ source: 'unknown-unvalidated-body' })] : [encoded({ retry: true })];
      const reader = {
        read() { observed.read++; if (slot === 0 && options.onRead) options.onRead(); return slot === 0 && options.read ? options.read.promise : Promise.resolve(index < parts.length ? { done: false, value: parts[index++] } : { done: true }); },
        cancel() { observed.cancel++; return slot === 0 && options.cancel ? options.cancel.promise : Promise.resolve(); },
        releaseLock() { observed.release++; if (slot === 0 && options.onRelease) options.onRelease(); if (slot === 0 && options.releaseError) throw Error('Release refused'); }
      };
      return { status: slot === 0 ? options.status ?? 200 : 200, headers: new Headers({ 'content-type': options.mime ?? 'application/json', ...(options.declared === undefined ? {} : { 'content-length': options.declared }) }), body: slot === 0 && options.missing ? null : { getReader: () => reader } };
    }
  };
  const native = () => mod.namespace.createNativeCompletionJsonReader(() => endpoint, { ...deps, origin: () => 'https://injected.invalid', headers: { Secret: 'not-authorized' }, credentials: 'omit' });
  const generic = () => mod.namespace.createOwnedJsonReader(limits, { ...deps, origin: () => sourceOrigin });
  const onUnhandled = error => unhandled.push(error);
  return { calls, allocations, observed, timers, unhandled, mod, native, generic, check,
    options: { assertCurrent: check }, setEndpoint(value) { endpoint = value; }, mutateEndpoint(key, value) { endpoint[key] = value; }, stale() { current = false; }, setOrigin(value) { sourceOrigin = value; },
    expire(ms) { for (const timer of [...timers.values()]) if (timer.ms === ms) timer.fn(); },
    observeUnhandled() { process.on('unhandledRejection', onUnhandled); return () => process.removeListener('unhandledRejection', onUnhandled); }
  };
}

test('frozen pre-change owner is exact independently reviewed source', () => {
  assert.equal(createHash('sha256').update(fs.readFileSync(beforeUrl)).digest('hex'), 'c1ede7e37d425b730e8e804055114a1abe9f26a15e7bb66c90b6350787d04717');
});
for (const old of [true, false]) for (const credentials of ['omit', 'same-origin']) test(`${old ? 'old' : 'current'} generic ${credentials}: exact headers, fixed allocation, healthy EOF`, async () => {
  const f = await fixture({ old }), signal = new AbortController(), read = f.generic();
  assert.equal((await read('/anonymous', { ...f.options, signal: signal.signal, credentials })).status, 200); signal.abort();
  assert.equal(f.calls[0].url, 'https://page.test/anonymous');
  assert.equal(f.calls[0].init.credentials, credentials); assert.deepEqual(Object.entries(f.calls[0].init.headers), [['Accept', 'application/json']]);
  assert.equal(f.calls[0].init.redirect, 'error'); assert.equal(f.calls[0].init.signal.aborted, false);
  assert.deepEqual(f.allocations, [16384]); assert.deepEqual(f.observed, { read: 2, cancel: 0, release: 1 }); assert.equal(f.timers.size, 0);
});
for (const old of [true, false]) test(`${old ? 'old' : 'current'} generic: cross-origin and destination drift refuse`, async () => {
  const gate = deferred(), f = await fixture({ old, fetch: gate }), read = f.generic();
  await assert.rejects(read('https://elsewhere.test/path', { ...f.options, credentials: 'same-origin' }), /same-origin/); assert.equal(f.calls.length, 0);
  const result = read('/public', { ...f.options, credentials: 'omit' }), rejected = assert.rejects(result, /destination changed/);
  await turns(); f.setOrigin('https://new.test'); gate.resolve(); await rejected; assert.equal(f.observed.read, 0); assert.equal(f.observed.cancel, 1);
});
test('generic runtime input cannot widen credentials or add arbitrary headers', async () => {
  const f = await fixture(), read = f.generic();
  await assert.rejects(read('/arbitrary', { ...f.options, credentials: 'include', headers: { 'X-WeebDex-Expected-Account': input.accountId } }), /credentials/); assert.equal(f.calls.length, 0);
  await read('/anonymous', { ...f.options, credentials: 'omit', headers: { Secret: 'not-authorized' } }); assert.deepEqual(Object.keys(f.calls[0].init.headers), ['Accept']);
});
test('native exact route, GET, only mandatory headers, include, no-store and redirect error; body stays unknown', async () => {
  const f = await fixture(), abort = new AbortController(), chunks = [], read = f.native();
  const response = await read({ ...input, url: 'https://evil.invalid/upload', history: ['never-upload'], completed: true }, { ...f.options, signal: abort.signal, consume: bytes => chunks.push(bytes), credentials: 'omit', headers: { Secret: 'no' } });
  assert.equal(response.status, 200); assert.equal(response.body.source, 'unknown-unvalidated-body');
  const { url, init } = f.calls[0], parsed = new URL(url); assert.equal(parsed.origin, nativeEndpoint.origin);
  assert.equal(parsed.pathname, '/native/api/user/history/manga/manga%3AOne/completion');
  assert.deepEqual([...parsed.searchParams], [['accountEpoch', '4'], ['catalogNamespace', input.catalogNamespace], ['serviceKey', 'null'], ...input.ids.map(id => ['id', id])]);
  assert.deepEqual(Object.keys(init).sort(), ['cache', 'credentials', 'headers', 'method', 'redirect', 'signal']);
  assert.deepEqual(Object.entries(init.headers), [['Accept', 'application/json'], ['X-WeebDex-Expected-Account', input.accountId]]);
  assert.equal(init.method, 'GET'); assert.equal(init.credentials, 'include'); assert.equal(init.cache, 'no-store'); assert.equal(init.redirect, 'error');
  assert.equal(chunks.reduce((a, b) => a + b, 0), encoded(response.body).byteLength); assert.deepEqual(f.allocations, [16384]);
  assert.equal(f.observed.cancel, 0); assert.equal(f.observed.release, 1); assert.equal(f.timers.size, 0); abort.abort(); assert.equal(init.signal.aborted, false);
});
for (const base of ['', '/api', '/configured-v1/sub_api']) test('native canonical configured base: ' + (base || '(root)'), async () => {
  const f = await fixture(); f.setEndpoint({ ...nativeEndpoint, base }); await f.native()(input, f.options);
  assert.equal(new URL(f.calls[0].url).pathname, base + '/user/history/manga/manga%3AOne/completion');
});
test('native 50 maximum-length IDs preserve caller order, explicit key and maximum safe epoch', async () => {
  const f = await fixture(), ids = Array.from({ length: 50 }, (_, i) => String(i).padStart(2, '0') + ':'.repeat(126));
  await f.native()({ ...input, ids, mangaId: ':'.repeat(128), accountId: 'a'.repeat(128), serviceKey: 'a'.repeat(64), accountEpoch: Number.MAX_SAFE_INTEGER }, f.options);
  const url = new URL(f.calls[0].url); assert.deepEqual(url.searchParams.getAll('id'), ids); assert.equal(url.searchParams.get('accountEpoch'), String(Number.MAX_SAFE_INTEGER));
  assert.equal(url.searchParams.get('serviceKey'), 'a'.repeat(64)); assert.ok(url.search.length < 24 * 1024);
});
const badInputs = [
  ['empty IDs', { ids: [] }], ['over 50', { ids: Array.from({ length: 51 }, (_, i) => 'c' + i) }], ['duplicate', { ids: ['c', 'c'] }], ['sparse IDs', { ids: Array(1) }],
  ...['', '/', '.', '..', 'https://evil.test', 'a?b', 'a#b', 'a%2Fb', 'a b', 'a\\b', 'a\r\nHeader:yes', '日本', 'a'.repeat(129), null, 1].flatMap(id => [['chapter ' + String(id), { ids: [id] }], ['manga ' + String(id), { mangaId: id }], ['account ' + String(id), { accountId: id }]]),
  ...[-1, -0, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, '0', '01', '1e3', undefined, null].map(accountEpoch => ['epoch ' + String(accountEpoch), { accountEpoch }]),
  ...[undefined, null, '', input.catalogNamespace.toUpperCase(), 'bad', input.catalogNamespace + ' '].map(catalogNamespace => ['namespace ' + String(catalogNamespace), { catalogNamespace }]),
  ...[undefined, '', 'null', 'NULL', 'A'.repeat(64), 'a'.repeat(63), 42].map(serviceKey => ['service ' + String(serviceKey), { serviceKey }])
];
for (const [name, changes] of badInputs) test('native invalid input before dispatch: ' + name, async () => {
  const f = await fixture(); await assert.rejects(f.native()({ ...input, ...changes }, f.options)); assert.equal(f.calls.length, 0); assert.equal(f.timers.size, 0); assert.equal(f.allocations.length, 0);
});
const badEndpoints = [
  { profile: 'public-retained-v1' }, { profile: '' }, { origin: 'file:///tmp' }, { origin: 'https://native.test/' }, { origin: 'https://u:p@native.test' }, { origin: 'https://native.test?x=1' },
  ...['//elsewhere.test', '/api/', '/api//x', '/api/../elsewhere', '/api/.', '/api/%2e%2e', '/api%2felsewhere', '/api?x=1', '/api#x', '/api\\x', '/api\n', 'api'].map(base => ({ base }))
];
for (const changes of badEndpoints) test('native invalid/non-native endpoint before dispatch: ' + JSON.stringify(changes), async () => {
  const f = await fixture(); f.setEndpoint({ ...nativeEndpoint, ...changes }); await assert.rejects(f.native()(input, f.options)); assert.equal(f.calls.length, 0);
});
test('native pre-aborted or stale caller does not dispatch', async () => {
  const f = await fixture(), abort = new AbortController(); abort.abort(); await assert.rejects(f.native()(input, { ...f.options, signal: abort.signal }), /cancelled/);
  f.stale(); await assert.rejects(f.native()(input, f.options), /authority changed/); assert.equal(f.calls.length, 0);
});
for (const stage of ['dispatch', 'fetch', 'read', 'consume', 'release']) for (const [key, value] of [['origin', 'https://changed.test'], ['base', '/elsewhere'], ['profile', 'public-retained-v1']]) test(`endpoint ${key} drift at ${stage} fences response`, async () => {
  const gate = deferred(); let f;
  f = await fixture({ ...(stage === 'fetch' ? { fetch: gate } : {}), ...(stage === 'read' ? { read: gate } : {}), ...(stage === 'release' ? { onRelease: () => f.mutateEndpoint(key, value) } : {}) });
  const result = f.native()(input, { ...f.options, ...(stage === 'consume' ? { consume: () => f.mutateEndpoint(key, value) } : {}) }), rejected = assert.rejects(result, /endpoint/);
  if (stage === 'dispatch') f.mutateEndpoint(key, value);
  if (stage === 'fetch' || stage === 'read') { await turns(); f.mutateEndpoint(key, value); gate.resolve(stage === 'read' ? { done: false, value: encoded({}) } : undefined); }
  await rejected; assert.equal(f.calls.length, stage === 'dispatch' ? 0 : 1); if (stage === 'fetch') assert.equal(f.observed.read, 0);
});
test('caller authority changing during read is rejected before bytes/DTO reach consumer', async () => {
  const gate = deferred(), f = await fixture({ read: gate }), consumed = [], result = f.native()(input, { ...f.options, consume: size => consumed.push(size) }), rejected = assert.rejects(result, /authority changed/);
  await turns(); f.stale(); gate.resolve({ done: false, value: encoded({ completed: false }) }); await rejected; assert.equal(consumed.length, 0); assert.equal(f.observed.cancel, 1);
});
for (const status of [301, 302, 307, 401, 403, 404, 409, 503]) test('non-200 ' + status + ' never follows, parses or accepts an error body', async () => {
  const f = await fixture({ status }), response = await f.native()(input, f.options);
  assert.equal(response.status, status); assert.equal(response.body, null); assert.equal(f.calls.length, 1); assert.equal(f.calls[0].init.redirect, 'error');
  assert.equal(f.observed.read, 0); assert.equal(f.observed.cancel, 1); assert.equal(f.observed.release, 1); assert.equal(f.calls[0].init.signal.aborted, true); assert.equal(f.allocations.length, 0);
});
test('fetch redirect/network rejection has no cookie fallback and observes failure', async () => {
  const f = await fixture({ fetchError: new TypeError('Redirect refused') }); await assert.rejects(f.native()(input, f.options), /Redirect refused/); assert.equal(f.calls.length, 1);
  assert.equal((await f.native()(input, f.options)).status, 200); assert.equal(f.calls.length, 2);
});
const badBodies = [
  ['oversize bytes', { parts: [new Uint8Array(16385)] }, /byte budget/], ['too many fragments', { parts: Array.from({ length: 4097 }, () => new Uint8Array([32])) }, /fragment budget/],
  ['too many empty fragments', { parts: Array.from({ length: 129 }, () => new Uint8Array()) }, /empty-fragment budget/], ['wrong MIME', { mime: 'text/plain' }, /Non-JSON/],
  ['oversize declared', { declared: '16385' }, /byte budget/], ['noncanonical declared', { declared: '01' }, /byte budget/], ['negative declared', { declared: '-1' }, /byte budget/],
  ['missing body', { missing: true }, /Missing/], ['invalid UTF-8', { parts: [new Uint8Array([255])] }, /encoded|encoding|valid/i], ['invalid JSON', { parts: [new Uint8Array([123])] }, /JSON|property|end/i]
];
for (const [name, options, error] of badBodies) test('native body bound: ' + name, async () => {
  const f = await fixture(options); await assert.rejects(f.native()(input, f.options), error); assert.equal(f.calls.length, 1);
  if (options.missing) { assert.deepEqual(f.observed, { read: 0, cancel: 0, release: 0 }); assert.deepEqual(f.allocations, []); }
  else assert.equal(f.observed.release, 1);
  if (name === 'too many fragments') assert.equal(f.observed.read, 4096); if (name === 'too many empty fragments') assert.equal(f.observed.read, 129);
  assert.ok(f.allocations.length <= 1); if (f.allocations.length) assert.deepEqual(f.allocations, [16384]);
  assert.equal(f.timers.size, 0);
});
test('exact 16 KiB body accepted without joining fragments or aborting healthy EOF', async () => {
  const bytes = new TextEncoder().encode('"' + 'x'.repeat(16382) + '"'), f = await fixture({ parts: [bytes.subarray(0, 8192), new Uint8Array(), bytes.subarray(8192)], declared: '16384' });
  const response = await f.native()(input, f.options); assert.equal(response.body.length, 16382); assert.deepEqual(f.allocations, [16384]); assert.equal(f.observed.cancel, 0); assert.equal(f.calls[0].init.signal.aborted, false);
});
for (const stage of ['fetch', 'read', 'cancel']) for (const terminal of ['resolve', 'reject']) test(`ignored abort: late ${stage} ${terminal} owns remounts until actual settlement`, async () => {
  const gate = deferred(), f = await fixture({ [stage]: gate, ...(stage === 'cancel' ? { status: 403 } : {}) }), abort = new AbortController(), stop = f.observeUnhandled();
  try {
    const read = f.native(), pending = read(input, { ...f.options, signal: abort.signal }), rejected = assert.rejects(pending, error => error.unsettled === true);
    await turns(); abort.abort(); await turns(); f.expire(250); await rejected;
    await assert.rejects(f.native()(input, f.options), error => error.unsettled === true); assert.equal(f.calls.length, 1);
    f.setEndpoint({ ...nativeEndpoint, profile: 'public-retained-v1' }); await assert.rejects(f.native()(input, f.options), /native endpoint/);
    f.setEndpoint({ ...nativeEndpoint, base: '/replacement/api' }); await assert.rejects(f.native()(input, f.options), error => error.unsettled === true); assert.equal(f.calls.length, 1);
    if (terminal === 'reject') gate.reject(Error('Late ' + stage + ' failure')); else gate.resolve(stage === 'read' ? { done: false, value: encoded({ ignored: true }) } : undefined);
    await turns(); assert.equal((await f.native()(input, f.options)).status, 200); assert.equal(f.calls.length, 2);
    await new Promise(resolve => setImmediate(resolve)); assert.deepEqual(f.unhandled, []); assert.equal(f.timers.size, 0);
  } finally { stop(); }
});
test('cancel settling alone cannot free an outstanding read; read terminal is required', async () => {
  const read = deferred(), cancel = deferred(), f = await fixture({ read, cancel }), abort = new AbortController();
  const pending = f.native()(input, { ...f.options, signal: abort.signal }), rejected = assert.rejects(pending, error => error.unsettled === true);
  await turns(); abort.abort(); await turns(); cancel.resolve(); await turns(); f.expire(250); await rejected;
  await assert.rejects(f.native()(input, f.options), error => error.unsettled === true); read.reject(Error('Read finally failed')); await turns(); assert.equal((await f.native()(input, f.options)).status, 200);
});
test('deadline and cleanup grace are finite but do not release ignored fetch', async () => {
  const gate = deferred(), f = await fixture({ fetch: gate }), pending = f.native()(input, f.options), rejected = assert.rejects(pending, error => error.unsettled === true);
  await turns(); assert.deepEqual([...f.timers.values()].map(t => t.ms), [10000]); f.expire(10000); await turns();
  assert.deepEqual([...f.timers.values()].map(t => t.ms), [250]); f.expire(250); await rejected; assert.equal(f.timers.size, 0);
  await assert.rejects(f.native()(input, f.options), error => error.unsettled === true); gate.resolve(); await turns(); assert.equal((await f.native()(input, f.options)).status, 200);
});
test('releaseLock failure cannot falsely free module-owned native slot', async () => {
  const f = await fixture({ releaseError: true }); await assert.rejects(f.native()(input, f.options), error => error.unsettled === true);
  await assert.rejects(f.native()(input, f.options), error => error.unsettled === true); assert.equal(f.calls.length, 1);
});
test('request is copied before deferred dispatch; local completion fields never become wire controls', async () => {
  const f = await fixture(), ids = [...input.ids], request = { ...input, ids }, pending = f.native()(request, f.options);
  ids[0] = 'different'; request.accountId = 'different-account'; request.serviceKey = 'b'.repeat(64); request.accountEpoch = 9;
  await pending; const call = f.calls[0], url = new URL(call.url);
  assert.deepEqual(url.searchParams.getAll('id'), input.ids); assert.equal(call.init.headers['X-WeebDex-Expected-Account'], input.accountId);
  assert.equal(url.searchParams.get('accountEpoch'), '4'); assert.equal(url.searchParams.get('serviceKey'), 'null');
});
test('native ownership does not widen or occupy an independent anonymous owner', async () => {
  const gate = deferred(), f = await fixture({ fetch: gate }), abort = new AbortController(), pending = f.native()(input, { ...f.options, signal: abort.signal }), rejected = assert.rejects(pending, error => error.unsettled === true);
  await turns(); abort.abort(); await turns(); f.expire(250); await rejected;
  const response = await f.generic()('/public', { ...f.options, credentials: 'omit' }); assert.equal(response.status, 200); assert.equal(f.calls[1].init.credentials, 'omit');
  await assert.rejects(f.native()(input, f.options), error => error.unsettled === true); gate.resolve(); await turns(); assert.equal((await f.native()(input, f.options)).status, 200);
});
