// Actual native history adapter, DTO, configured-route transport, and ReaderBudget.
// Native library/authority observations and HTTP responses are controlled seams;
// this does not qualify cookies, SQL, browser mounts, or actual account persistence.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import { deferred, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url)), fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
async function fixture() {
  const calls = [], timers = new Map(), authority = { actor: 'actor:one', epoch: 7, generation: 1 };
  const source = { context: { generation: 1, key: 'account-one', mode: 'account', accountId: authority.actor }, mode: 'account', selectionPending: false, selectionEpoch: 0, ready: false, revision: 4, accountRevision: 8, scopeStatus: 'pending', scope: undefined, reason: 'Device database is unavailable', records: [{ completed: true, privateNote: 'NEVER_UPLOAD_LOCAL_HISTORY' }] };
  const identity = { origin: 'https://native-api.test', catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
  let endpoint = { origin: identity.origin, base: '/custom/api', profile: 'native-aggregate-v1' }, cookieActor = authority.actor, responder, actionEpoch = 0;
  const snapshot = () => ({ ...source, admission: { selectionEpoch: source.selectionEpoch, authority: { ...authority } } });
  const envelope = ids => ({ version: 1, mangaId: 'manga:one', catalogIdentity: { catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey }, account: { id: cookieActor, epoch: authority.epoch }, consistency: 'live-request-v1', continueProof: 'observed-only', data: ids.map(id => ({ id, completed: id === 'chapter:two' })) });
  const context = vm.createContext({ console, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, AbortController,
    setTimeout(fn, ms) { const key = {}; timers.set(key, { fn, ms }); return key; }, clearTimeout(key) { timers.delete(key); },
    fetch: async (url, options) => { const call = { url: new URL(url), options }; calls.push(call); if (options.headers['X-WeebDex-Expected-Account'] !== cookieActor) return new Response('', { status: 409 }); const body = responder ? await responder(call) : envelope(call.url.searchParams.getAll('id')); return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } }); }
  });
  const modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (relative === 'platform-integrations/local-library/library.ts') {
      const values = { readerLibrarySource: snapshot, assertLibraryAdmission(expected) { if (expected.selectionEpoch !== source.selectionEpoch || expected.authority.actor !== authority.actor || expected.authority.epoch !== authority.epoch) throw Error('Library admission changed'); } };
      const mod = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context }); modules.set(relative, mod); return mod;
    }
    const local = path.join(root, relative), file = fs.existsSync(local) ? local : path.join(fallback, relative);
    const mod = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts')))))); links.set(relative, linked); await linked; return mod;
  }
  const native = await load('platform-integrations/bounded-reader/native-history.ts'); await native.evaluate();
  const client = await load('platform-integrations/bounded-reader/client.ts'); await client.evaluate();
  const access = native.namespace.createNativeHistoryAccess(() => endpoint);
  const budget = () => { const epoch = actionEpoch; return new client.namespace.ReaderBudget(() => { if (actionEpoch !== epoch) throw Error('Action scope changed'); }); };
  return { source, identity, authority, calls, timers, envelope, access: () => access('manga:one', identity), budget,
    set responder(value) { responder = value; }, set endpoint(value) { endpoint = value; }, set cookieActor(value) { cookieActor = value; }, invalidate() { actionEpoch++; },
    expireCleanup() { for (const item of [...timers.values()]) if (item.ms === 250) item.fn(); } };
}
test('actual configured native request sends selected IDs only and accepts exact membership coverage without IndexedDB', async () => {
  const f = await fixture(), b = f.budget(); assert.equal(f.access().status, 'available-native');
  assert.equal(await f.access().completed(['chapter:one', 'chapter:two'], b), true); b.close();
  assert.equal(f.calls.length, 1); const { url, options } = f.calls[0];
  assert.equal(url.origin + url.pathname, 'https://native-api.test/custom/api/user/history/manga/manga%3Aone/completion');
  assert.deepEqual(url.searchParams.getAll('id'), ['chapter:one', 'chapter:two']); assert.equal(url.searchParams.get('accountEpoch'), '7');
  assert.deepEqual([...new Set(url.searchParams.keys())].sort(), ['accountEpoch', 'catalogNamespace', 'id', 'serviceKey']);
  assert.equal(options.credentials, 'include'); assert.equal(options.redirect, 'error'); assert.equal(options.headers['X-WeebDex-Expected-Account'], 'actor:one');
  assert.deepEqual(Object.keys(options.headers).sort(), ['Accept', 'X-WeebDex-Expected-Account']);
  assert(!JSON.stringify([url.href, options.headers]).includes('NEVER_UPLOAD')); assert.equal(b.requests, 1); assert.equal(b.rows, 2); assert(b.bytes > 0 && b.bytes < 16384); assert.equal(f.timers.size, 0);
});
for (const change of ['account', 'epoch', 'selection', 'revision', 'context', 'mode', 'pending', 'endpoint', 'action']) test('late native completion rejects changed ' + change, async () => {
  const f = await fixture(), gate = deferred(), b = f.budget(); f.responder = async call => { const value = f.envelope(call.url.searchParams.getAll('id')); await gate.promise; return value; };
  const work = f.access().completed(['chapter:one'], b), rejected = assert.rejects(work); await turns();
  if (change === 'account') f.authority.actor = 'actor:other'; else if (change === 'epoch') f.authority.epoch++; else if (change === 'selection') f.source.selectionEpoch += 2;
  else if (change === 'revision') f.source.accountRevision++; else if (change === 'context') f.source.context = { ...f.source.context, generation: 3 };
  else if (change === 'mode') f.source.mode = 'device'; else if (change === 'pending') f.source.selectionPending = true;
  else if (change === 'endpoint') f.endpoint = { origin: 'https://native-api.test', base: '/changed', profile: 'native-aggregate-v1' }; else f.invalidate();
  gate.resolve(); await rejected; b.close(); assert.equal(b.rows, 0); assert.equal(f.calls.length, 1);
});
test('harmless native Session data generation refresh remains readable', async () => {
  const f = await fixture(), gate = deferred(), b = f.budget(); f.responder = async call => { await gate.promise; return f.envelope(call.url.searchParams.getAll('id')); };
  const work = f.access().completed(['chapter:one'], b); await turns(); f.authority.generation++; gate.resolve(); assert.equal(await work, false); b.close();
});
for (const fault of ['namespace', 'service', 'actor', 'epoch', 'missing-row', 'reordered', 'duplicate', 'foreign-parent', 'not-boolean', 'extra-field']) test('actual native DTO rejects ' + fault + ' without empty-history fallback', async () => {
  const f = await fixture(), b = f.budget(); f.responder = call => { const value = f.envelope(call.url.searchParams.getAll('id'));
    if (fault === 'namespace') value.catalogIdentity.catalogNamespace = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; else if (fault === 'service') value.catalogIdentity.serviceKey = 'a'.repeat(64);
    else if (fault === 'actor') value.account.id = 'actor:other'; else if (fault === 'epoch') value.account.epoch++;
    else if (fault === 'missing-row') value.data.pop(); else if (fault === 'reordered') value.data.reverse(); else if (fault === 'duplicate') value.data[1] = value.data[0];
    else if (fault === 'foreign-parent') value.mangaId = 'other'; else if (fault === 'not-boolean') value.data[0].completed = 1; else value.data[0].privateHistory = [];
    return value; };
  await assert.rejects(f.access().completed(['chapter:one', 'chapter:two'], b)); b.close(); assert.equal(b.rows, 0); assert.equal(f.calls.length, 1);
});
test('unobserved cookie account swap fails the expected-account request, never device fallback', async () => {
  const f = await fixture(), b = f.budget(); f.cookieActor = 'actor:other'; await assert.rejects(f.access().completed(['chapter:one'], b), /admission changed/); b.close(); assert.equal(f.calls.length, 1); assert.equal(b.rows, 0);
});
for (const blocked of ['public', 'pending', 'wrong-actor']) test(blocked + ' native history never dispatches a credentialed request', async () => {
  const f = await fixture(), b = f.budget(); if (blocked === 'public') f.endpoint = { origin: 'https://reader.test', base: '/api/v1', profile: 'public-retained-v1' }; else if (blocked === 'pending') f.source.selectionPending = true; else f.source.context.accountId = 'actor:other';
  assert.notEqual(f.access().status, 'available-native'); await assert.rejects(f.access().completed(['chapter:one'], b)); b.close(); assert.equal(f.calls.length, 0);
});
test('cancelled native lookup retains the shared network slot through remount until ignored fetch settles', async () => {
  const f = await fixture(), gate = deferred(), b = f.budget(); f.responder = async call => { await gate.promise; return f.envelope(call.url.searchParams.getAll('id')); };
  const old = f.access().completed(['chapter:one'], b), rejected = assert.rejects(old); await turns(); b.cancel(); await turns(); f.expireCleanup(); await rejected; b.close();
  const retry = f.budget(); await assert.rejects(f.access().completed(['chapter:one'], retry)); retry.close(); assert.equal(f.calls.length, 1);
  gate.resolve(); await turns(); f.responder = undefined; const fresh = f.budget(); assert.equal(await f.access().completed(['chapter:one'], fresh), false); fresh.close(); assert.equal(f.calls.length, 2);
});
