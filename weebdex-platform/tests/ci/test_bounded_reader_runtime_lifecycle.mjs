// SOURCE-AUTHORED, UNEXECUTED. The actual runtime module, controller, transport,
// validators, endpoint and navigation-intent modules execute. Svelte lifecycle,
// authority stores and local-library observations are explicit controlled seams;
// this is not compiled Svelte, real IndexedDB, cookies or browser qualification.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash, webcrypto } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import { row, id, deferred, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url)), frozen = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
function store(value) { const listeners = new Set(); return { get value() { return value; }, subscribe(listener) { listeners.add(listener); listener(value); return () => listeners.delete(listener); }, set(next) { value = next; for (const listener of listeners) listener(value); }, touch() { for (const listener of listeners) listener(value); } }; }
async function fixture(publicMode = false, title = false) {
  const mounts = [], destroys = [], effects = [], calls = [], navigated = [], timers = new Map(), native = store({ actor: null, epoch: 0 }), local = store({}), prefs = Object.assign(store({}), { applyChapterPreferences(query) { query.append('tlang', 'en'); } });
  const source = { admission: { selectionEpoch: 0, authority: { actor: undefined, epoch: 0 } }, selectionPending: false, context: { generation: 1, mode: 'device' }, mode: 'device', ready: false, revision: 0, accountRevision: 0, scopeStatus: 'pending', scope: undefined, reason: undefined, observedCatalogue: undefined };
  let dbReads = 0; Object.defineProperty(source, 'db', { get() { dbReads++; throw Error('Ordinary navigation must not open device history'); } });
  const current = row(publicMode, 2), chapter = { ...current, relationships: { groups: [], manga: { id: id(publicMode, 999), title: 'Fixture' } } }, catalogIdentity = { catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
  const pageHref = 'https://reader.test/chapter/' + current.id + '/1', api = 'https://native-api.test/custom/api', capabilityUrl = publicMode ? 'https://reader.test/api/protocol/capabilities' : api + '/protocol/capabilities';
  let responder;
  const envelope = url => { const operation = url.searchParams.get('operation'), target = operation === 'next' || operation === 'last' ? row(publicMode, 3) : operation === 'previous' || operation === 'first' ? row(publicMode, 1) : current; return { version: 1, mangaId: chapter.relationships.manga.id, operation, orderingProfile: publicMode ? 'public-retained-v1' : 'native-aggregate-v1', catalogIdentity, scope: 'a'.repeat(64), consistency: 'live-keyset-v1', crossRequestSnapshot: false, history: 'device-local-only', continueProof: 'observed-only', state: 'ready', restartRequired: false, data: [target], ...(url.searchParams.has('current') ? { current } : {}), nextCursor: null, limit: Number(url.searchParams.get('limit')), ...(operation === 'around' ? { next: row(publicMode, 3), previous: row(publicMode, 1) } : {}), ...(['first', 'last'].includes(operation) ? { hasMultipleEditions: false } : {}) }; };
  responder = url => url.href === capabilityUrl ? { catalog_namespace: catalogIdentity.catalogNamespace, service_key: catalogIdentity.serviceKey } : envelope(url);
  const context = vm.createContext({ console, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, AbortController, queueMicrotask, crypto: webcrypto,
    window: { location: { href: pageHref } }, $effect(fn) { effects.push(fn); fn(); },
    setTimeout(fn, ms) { const key = {}; timers.set(key, { fn, ms }); return key; }, clearTimeout(key) { timers.delete(key); },
    async fetch(url, options) { const parsed = new URL(url); calls.push({ url: parsed, options }); const value = await responder(parsed); return value instanceof Response ? value : new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } }); } });
  const stubs = new Map([
    ['svelte', { onMount: fn => mounts.push(fn), onDestroy: fn => destroys.push(fn) }],
    ['$app/navigation', { goto: async target => { navigated.push(target); }, beforeNavigate() {} }], ['$app/paths', { resolve: value => value }],
    ['$env/static/public', { PUBLIC_API_URL: api }], ['$lib/stores', { prefs }],
    ['platform-integrations/local-library/library.ts', { localLibraryState: local, readerLibrarySelection: local, readerLibrarySource: () => source, assertLibraryAdmission() {} }],
    ['platform-integrations/public-api/library-staging.ts', { publicUuidLibraryEnabled: publicMode }],
    ['platform-integrations/frontend-behavior/mutation-authority.ts', { nativeAuthority: native, captureNativeAuthority: () => native.value }]
  ]), modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (stubs.has(relative)) { const values = stubs.get(relative), mod = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context }); modules.set(relative, mod); return mod; }
    const localPath = path.join(root, relative), file = fs.existsSync(localPath) ? localPath : path.join(frozen, relative), input = fs.readFileSync(file, 'utf8');
    const mod = new vm.SourceTextModule(relative.endsWith('.ts') ? stripTypeScriptTypes(input, { mode: 'transform' }) : input, { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(specifier.startsWith('.') ? path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts'))) : specifier))); links.set(relative, linked); await linked; return mod;
  }
  const mod = await load('platform-integrations/bounded-reader/runtime.svelte.ts'); await mod.evaluate();
  const controller = mod.namespace.createBoundedReader({ manga: () => chapter.relationships.manga.id, ...(title ? {} : { current: () => chapter }), filters: () => new URLSearchParams({ tlang: 'en' }), queryEpoch: () => pageHref });
  const settle = async () => { for (let i = 0; i < 20; i++) { await turns(); if (!controller.snapshot.pending) return; } assert.fail('Owned runtime did not settle'); };
  return { controller, source, local, native, calls, navigated, catalogIdentity, capabilityUrl, envelope, get dbReads() { return dbReads; }, set responder(value) { responder = value; }, async mount() { mounts.forEach(fn => fn()); await settle(); }, settle, dispose() { destroys.forEach(fn => fn()); }, observe(scope) { source.observedCatalogue = { url: capabilityUrl, scope, generation: 1 }; local.touch(); } };
}
for (const publicMode of [false, true]) {
  test((publicMode ? 'public' : 'native configured API') + ': actual runtime navigates without opening unavailable IndexedDB', async () => {
    const f = await fixture(publicMode); await f.mount(); assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.calls.length, 2); assert.equal(f.calls[0].url.href, f.capabilityUrl); assert.equal(f.calls[0].options.credentials, 'omit'); assert.equal(f.calls[1].options.credentials, publicMode ? 'omit' : 'same-origin');
    await f.controller.semantic('next'); assert.equal(f.navigated[0], '/chapter/' + id(publicMode, 3) + '/1'); assert.equal(f.dbReads, 0); f.dispose();
  });
  for (const pending of [false, true]) test((publicMode ? 'public' : 'native') + ': positive identity observation ' + (pending ? 'during action' : 'after ready') + ' invalidates cached identity and requires reload', async () => {
    const f = await fixture(publicMode); await f.mount(); const gate = deferred(); let action;
    if (pending) { f.responder = async url => { const body = f.envelope(url); await gate.promise; return body; }; action = f.controller.semantic('next'); await turns(); }
    const count = f.calls.length; f.observe({ origin: new URL(f.capabilityUrl).origin, ...f.catalogIdentity, serviceKey: 'b'.repeat(64) }); gate.resolve(); if (action) await action; await f.settle();
    assert.equal(f.controller.snapshot.requiresReload, true); assert.equal(f.controller.snapshot.current, undefined); assert.equal(f.navigated.length, 0); await f.controller.verifyIdentity(); assert.equal(f.calls.length, count); assert.equal(f.dbReads, 0); f.dispose();
  });
  test((publicMode ? 'public' : 'native') + ': history revision may refresh around but a changed selector identity cannot publish or navigate', async () => {
    const f = await fixture(publicMode); await f.mount(); f.source.revision++; f.local.touch(); await f.settle(); assert.equal(f.calls.length, 3); assert.equal(f.calls.at(-1).url.searchParams.get('operation'), 'around');
    f.responder = url => ({ ...f.envelope(url), catalogIdentity: { ...f.catalogIdentity, catalogNamespace: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb' } }); await f.controller.semantic('next'); assert.equal(f.controller.snapshot.requiresReload, true); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.current, undefined); f.dispose();
  });
}
test('actual runtime dispose cancels late capability publication without IndexedDB or navigation', async () => {
  const f = await fixture(), gate = deferred(); f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const mounted = f.mount(); await turns(); f.dispose(); gate.resolve(); await mounted; assert.equal(f.calls.length, 1); assert.equal(f.controller.snapshot.current, undefined); assert.equal(f.navigated.length, 0); assert.equal(f.dbReads, 0);
});

for (const replacement of [false, true]) test('actual runtime keeps verified identity across generic read and verification failures: ' + (replacement ? 'replacement requires reload' : 'same identity can retry'), async () => {
  const f = await fixture(); await f.mount(); f.responder = () => { throw Error('Generic read unavailable'); }; await f.controller.semantic('next'); assert.equal(f.controller.snapshot.status, 'error');
  await f.controller.verifyIdentity(); assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.calls.length, 4);
  f.responder = url => url.href === f.capabilityUrl ? { catalog_namespace: replacement ? 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb' : f.catalogIdentity.catalogNamespace, service_key: null } : f.envelope(url);
  await f.controller.verifyIdentity(); assert.equal(f.navigated.length, 0);
  if (replacement) { assert.equal(f.controller.snapshot.requiresReload, true); assert.equal(f.controller.snapshot.current, undefined); assert.equal(f.calls.length, 5); }
  else { assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.controller.snapshot.current.id, 'chapter2'); assert.equal(f.calls.length, 6); }
  f.dispose();
});

function nativeAccount(f) {
  f.source.mode = 'account'; f.source.context = { generation: 1, mode: 'account', accountId: 'cookie-account' };
  f.source.admission = { selectionEpoch: 1, authority: { actor: 'cookie-account', epoch: 4 } }; f.native.set({ actor: 'cookie-account', epoch: 4 });
}
function accountResponse(f, url) {
  if (url.href === f.capabilityUrl) return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: f.catalogIdentity.serviceKey };
  if (url.pathname.endsWith('/continue')) {
    const controls = new Set(['accountEpoch', 'catalogNamespace', 'serviceKey', 'requestId']);
    const filters = Object.fromEntries([...new Set(url.searchParams.keys())].filter(key => !controls.has(key)).sort().map(key => [key, [...new Set(url.searchParams.getAll(key))].sort()]));
    const target = row(false, 2), scope = createHash('sha256').update(JSON.stringify([{ ...f.catalogIdentity }, 'native-aggregate-v1', 'native-continue', id(false, 999), 'cookie-account', filters])).digest('hex');
    return { version: 1, operation: 'native-continue', orderingProfile: 'native-aggregate-v1', mangaId: id(false, 999), catalogIdentity: { ...f.catalogIdentity }, account: { id: 'cookie-account', epoch: 4 }, requestId: url.searchParams.get('requestId'), scope, consistency: 'live-request-v1', continueProof: 'native-account-snapshot-v1', crossRequestSnapshot: false, state: 'ready', reason: 'oldest-unread', target: { volume: target.volume, chapter: target.chapter, anchorChapterId: target.id }, winner: target, hasMultipleEditions: false };
  }
  if (url.pathname.endsWith('/completion')) return { version: 1, mangaId: id(false, 999), catalogIdentity: { ...f.catalogIdentity }, account: { id: 'cookie-account', epoch: 4 }, consistency: 'live-request-v1', continueProof: 'observed-only', data: url.searchParams.getAll('id').map(id => ({ id, completed: false })) };
  return { ...f.envelope(url), ...(['editions', 'resolve-group'].includes(url.searchParams.get('operation')) ? { groupAnchor: row(false, 2), winner: row(false, 2), hasMultipleEditions: false } : {}) };
}
test('actual runtime uses native account completion for an observed candidate without device reads or automatic navigation', async () => {
  const f = await fixture(false, true); nativeAccount(f); f.responder = url => accountResponse(f, url); await f.mount(); await f.controller.continueScan();
  assert.equal(f.controller.snapshot.status, 'candidate'); assert.equal(f.controller.snapshot.candidate.id, 'chapter2'); assert.equal(f.navigated.length, 0); assert.equal(f.dbReads, 0);
  const personal = f.calls.filter(call => call.url.pathname.endsWith('/completion')); assert.equal(personal.length, 2); assert(personal.every(call => call.options.credentials === 'include' && call.options.headers['X-WeebDex-Expected-Account'] === 'cookie-account'));
  await f.controller.openCandidate(); assert.deepEqual(f.navigated, ['/chapter/chapter2/1']); f.dispose();
});
for (const fault of ['namespace', 'account', 'coverage-conflict']) test('native ' + fault + ' reload fence survives Start, Continue, Verify and context changes', async () => {
  const f = await fixture(false, true); nativeAccount(f); f.responder = url => {
    const value = accountResponse(f, url); if (!url.pathname.endsWith('/completion')) return value;
    if (fault === 'coverage-conflict') return new Response('', { status: 409 });
    if (fault === 'namespace') value.catalogIdentity.catalogNamespace = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; else value.account.id = 'other-cookie-account'; return value;
  };
  await f.mount(); await f.controller.continueScan(); assert.equal(f.controller.snapshot.requiresReload, true); const message = f.controller.snapshot.message, count = f.calls.length;
  assert.match(message, fault === 'namespace' ? /catalogue changed/ : fault === 'account' ? /account changed/ : /account, catalogue or requested chapter admission changed/);
  f.responder = url => accountResponse(f, url); await f.controller.start(); await f.controller.continueScan(true); f.controller.cancel(); await f.controller.verifyIdentity();
  f.source.context = { ...f.source.context, generation: 2 }; f.source.admission = { ...f.source.admission, selectionEpoch: 3 }; f.local.touch(); await f.settle(); await f.controller.start(); await f.controller.continueScan();
  assert.equal(f.controller.snapshot.requiresReload, true); assert.equal(f.controller.snapshot.message, message); assert.equal(f.controller.snapshot.candidate, undefined); assert.equal(f.calls.length, count); assert.equal(f.navigated.length, 0); assert.equal(f.dbReads, 0); f.dispose();
});

test('actual title with no current chapter can Start through the native runtime before any reload fence', async () => {
  const f = await fixture(false, true); nativeAccount(f); f.responder = url => accountResponse(f, url); await f.mount();
  assert.equal(f.calls.length, 1); await f.controller.start(); assert.deepEqual(f.navigated, ['/chapter/chapter1/1']);
  assert.equal(f.calls.length, 3); assert(f.calls.slice(1).every(call => call.url.searchParams.get('operation') === 'first' && !call.url.searchParams.has('current'))); assert.equal(f.dbReads, 0); f.dispose();
});

test('actual title runtime automatically Continues the native account without device or legacy badge history', async () => {
  const f = await fixture(false, true); nativeAccount(f); f.responder = url => accountResponse(f, url); await f.mount(); await f.controller.continueNative();
  assert.deepEqual(f.navigated, ['/chapter/chapter2/1']); assert.equal(f.calls.length, 3); assert(f.calls.slice(1).every(call => call.url.pathname.endsWith('/continue') && call.options.credentials === 'include'));
  assert.equal(f.dbReads, 0); assert.equal(f.source.ready, false); f.dispose();
});
for (const fault of ['namespace', 'account', 'conflict']) test('actual runtime native automatic Continue ' + fault + ' is a sticky reload fence', async () => {
  const f = await fixture(false, true); nativeAccount(f); f.responder = url => {
    const body = accountResponse(f, url); if (!url.pathname.endsWith('/continue')) return body;
    if (fault === 'conflict') return new Response('', { status: 409 }); if (fault === 'namespace') body.catalogIdentity.catalogNamespace = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; else body.account.id = 'other'; return body;
  };
  await f.mount(); await f.controller.continueNative(); assert.equal(f.controller.snapshot.requiresReload, true); const count = f.calls.length;
  f.responder = url => accountResponse(f, url); await f.controller.start(); await f.controller.continueNative(); f.controller.cancel(); await f.controller.verifyIdentity();
  f.source.admission = { ...f.source.admission, selectionEpoch: 4 }; f.local.touch(); await f.controller.continueNative(); assert.equal(f.calls.length, count); assert.equal(f.navigated.length, 0); assert.equal(f.dbReads, 0); f.dispose();
});
for (const publicMode of [false, true]) test((publicMode ? 'public' : 'native') + ': unused title controls survive library context updates without stale-selection warning', async () => {
  const f = await fixture(publicMode, true); await f.mount(); const requests = f.calls.length;
  f.source.context.generation++; f.local.touch(); await f.settle();
  assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.controller.snapshot.message, '');
  assert.equal(f.calls.length, requests); assert.equal(f.controller.snapshot.requiresReload, undefined);
  await f.controller.start(); assert.equal(f.navigated.length, 1); f.dispose();
});
