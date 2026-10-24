// Finite source-bound primitive controls. No sockets, browser, services or keys.
import fs from 'node:fs';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import assert from 'node:assert/strict';
const source = name => fs.readFileSync(new URL('../../platform-integrations/local-library/' + name + '.ts', import.meta.url), 'utf8');
const rejected = name => fs.readFileSync(new URL('../fixtures/' + name + '-rejected-a147c4f.ts.txt', import.meta.url), 'utf8');
const defer = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const flush = async () => { for (let i = 0; i < 80; i++) await Promise.resolve(); };
const encoded = text => new TextEncoder().encode(text);
function response(parts = [], options = {}) {
  const calls = { read: 0, cancel: 0, release: 0, acquire: 0, locked: false };
  const reader = {
    read() { calls.read++; return options.read ? options.read(calls.read) : Promise.resolve(parts.length ? { done: false, value: parts.shift() } : { done: true }); },
    cancel() { calls.cancel++; return options.cancel ? options.cancel() : Promise.resolve(); },
    releaseLock() { calls.release++; calls.locked = false; options.release?.(); },
  };
  return { calls, value: { status: 200, headers: { get: () => options.mime ?? 'application/json' }, body: { getReader() { calls.acquire++; calls.locked = true; return reader; } } } };
}
async function fixture({ oldTransport = false, oldPagination = false } = {}) {
  const timers = new Map(), allocations = [], calls = []; let now = 0;
  const f = { fetch: async () => response([encoded('{}')]).value };
  const context = vm.createContext({ URL, TextEncoder, TextDecoder, AbortController,
    Date: class extends Date { static now() { return now; } },
    Uint8Array: new Proxy(Uint8Array, { construct(target, args) { allocations.push(args[0]); return Reflect.construct(target, args); } }),
    setTimeout(fn, ms) { const token = {}; timers.set(token, { fn, ms }); return token; }, clearTimeout(token) { timers.delete(token); },
    fetch(url, options) { calls.push({ url, options }); return f.fetch(url, options); },
  });
  const mod = text => new vm.SourceTextModule(stripTypeScriptTypes(text, { mode: 'transform' }), { context });
  const instant = mod(source('device-time')); await instant.link(() => { throw Error('Unexpected instant import'); }); await instant.evaluate();
  const pagination = mod(oldPagination ? rejected('history-pagination') : source('history-pagination')); await pagination.link(specifier => { assert.equal(specifier, './device-time'); return instant; }); await pagination.evaluate();
  const transport = mod(oldTransport || oldPagination ? rejected('history-transport') : source('history-transport')); await transport.link(() => pagination); await transport.evaluate();
  const budget = new pagination.namespace.HistoryBudget(() => {});
  return Object.assign(f, { budget, calls, timers, allocations, api: pagination.namespace, read: (b = budget) => transport.namespace.historyJson('/synthetic-history', b),
    expire(ms) { const found = [...timers].find(([, row]) => row.ms === ms); assert.ok(found, 'owned timer exists: ' + ms); timers.delete(found[0]); now += ms; found[1].fn(); },
  });
}
test('healthy EOF uses one fixed allocation for tiny/zero fragments and detaches without abort/cancel', async () => {
  const f = await fixture(), text = JSON.stringify({ value: 'x'.repeat(20000) }), parts = [...encoded(text)].map(value => new Uint8Array([value]));
  parts.splice(20, 0, ...Array.from({ length: 100 }, () => new Uint8Array(0)));
  const r = response(parts); f.fetch = async () => r.value;
  assert.equal((await f.read()).body.value.length, 20000); assert.deepEqual(f.allocations, [f.api.HISTORY_LIMITS.responseBytes]);
  assert.equal(f.budget.bytes, encoded(text).length); assert.equal(r.calls.cancel, 0); assert.equal(r.calls.release, 1); assert.equal(r.calls.locked, false);
  f.budget.cancel(); assert.equal(f.calls[0].options.signal.aborted, false); assert.equal(r.calls.cancel, 0); f.budget.close(); assert.equal(f.timers.size, 0);
});
for (const empty of [true, false]) test((empty ? 'zero' : 'tiny') + '-fragment flood has a separate finite work bound', async () => {
  const f = await fixture(), r = response([], { read: async () => ({ done: false, value: new Uint8Array(empty ? 0 : 1) }) }); f.fetch = async () => r.value;
  await assert.rejects(f.read(), /fragment budget/); assert.equal(r.calls.cancel, 1); assert.equal(r.calls.release, 1);
  assert.equal(r.calls.read, empty ? f.api.HISTORY_LIMITS.emptyFragments + 1 : f.api.HISTORY_LIMITS.fragments); assert.deepEqual(f.allocations, [f.api.HISTORY_LIMITS.responseBytes]); f.budget.close();
});
test('pre-aborted action makes no fetch; abort at fetch resolution disposes body without reading', async () => {
  const f = await fixture(); f.budget.cancel(); await assert.rejects(f.read(), /cancelled/); assert.equal(f.calls.length, 0); f.budget.close();
  const g = await fixture(), r = response([encoded('{}')]); g.fetch = async () => { g.budget.cancel(); return r.value; };
  await assert.rejects(g.read(), /cancelled/); assert.equal(r.calls.read, 0); assert.equal(r.calls.cancel, 1); assert.equal(r.calls.release, 1); g.budget.close();
});
test('cancellation waits for both pending read and cancellation settlement before releasing lock', async () => {
  const f = await fixture(), read = defer(), cancel = defer(), r = response([], { read: () => read.promise, cancel: () => { read.resolve({ done: true }); return cancel.promise; } }); f.fetch = async () => r.value;
  let ended = false; const pending = f.read().finally(() => { ended = true; }); const check = assert.rejects(pending, /cancelled/);
  await flush(); f.budget.cancel(); await flush(); assert.equal(r.calls.cancel, 1); assert.equal(ended, false); assert.equal(r.calls.locked, true); assert.equal(r.calls.release, 0);
  cancel.resolve(); await check; assert.equal(r.calls.release, 1); assert.equal(r.calls.locked, false); f.budget.close(); assert.equal(f.timers.size, 0);
});
test('cancel rejection is observed, pending read settles, and no successful page or next request escapes', async () => {
  const f = await fixture(), read = defer(), r = response([], { read: () => read.promise, cancel: () => { read.reject(Error('read aborted')); return Promise.reject(Error('cancel refused')); } }); f.fetch = async () => r.value;
  const pending = f.read(), check = assert.rejects(pending, /cancelled/); await flush(); f.budget.cancel(); await check;
  assert.equal(r.calls.release, 1); assert.equal(r.calls.locked, false); assert.equal(f.calls.length, 1); f.budget.close();
});
test('late fetch within grace remains owned until body cancellation settles', async () => {
  const f = await fixture(), fetch = defer(), cancel = defer(), r = response([], { cancel: () => cancel.promise }); f.fetch = () => fetch.promise;
  let ended = false; const pending = f.read().finally(() => { ended = true; }); const check = assert.rejects(pending, /deadline/);
  await flush(); f.expire(f.api.HISTORY_LIMITS.deadlineMs); await flush(); fetch.resolve(r.value); await flush();
  assert.equal(r.calls.read, 0); assert.equal(r.calls.cancel, 1); assert.equal(r.calls.locked, true); assert.equal(ended, false);
  cancel.resolve(); await check; assert.equal(r.calls.release, 1); f.budget.close(); assert.equal(f.timers.size, 0);
});
for (const late of ['fulfillment', 'rejection']) test('fetch ' + late + ' after grace is observed and blocks every new read until settlement', async () => {
  const f = await fixture(), fetch = defer(), cancel = defer(), r = response([], { cancel: () => cancel.promise }); f.fetch = () => fetch.promise;
  const pending = f.read(), check = assert.rejects(pending, /cleanup is unconfirmed/);
  await flush(); f.expire(f.api.HISTORY_LIMITS.deadlineMs); await flush(); f.expire(f.api.HISTORY_LIMITS.cleanupMs); await check; f.budget.close();
  const fresh = new f.api.HistoryBudget(() => {}); await assert.rejects(f.read(fresh), /cleanup is unconfirmed/); assert.equal(f.calls.length, 1);
  if (late === 'fulfillment') {
    fetch.resolve(r.value); await flush(); assert.equal(r.calls.cancel, 1); assert.equal(r.calls.release, 0); assert.equal(r.calls.locked, true);
    await assert.rejects(f.read(fresh), /cleanup is unconfirmed/); assert.equal(f.calls.length, 1);
    cancel.reject(Error('late cancellation rejection')); await flush(); assert.equal(r.calls.release, 1);
  } else { fetch.reject(Error('late fetch rejection')); await flush(); }
  f.fetch = async () => response([encoded('{}')]).value; assert.ok((await f.read(fresh)).body); assert.equal(f.calls.length, 2); fresh.close(); assert.equal(f.timers.size, 0);
});
test('stalled cancellation poisons the page instead of converting the deadline into Continue', async () => {
  const f = await fixture(), read = defer(), cancel = defer(), r = response([], { read: () => read.promise, cancel: () => { read.resolve({ done: true }); return cancel.promise; } }); f.fetch = async () => r.value; f.budget.close();
  const key = { id: 'chapter', mangaId: 'parent', readAt: '2026-01-01T00:00:00Z' };
  const pager = new f.api.HistoryPager([key], async (_keys, budget) => { await f.read(budget); return []; }, () => {});
  const pending = pager.load(), check = assert.rejects(pending, /cleanup is unconfirmed/); await flush(); f.expire(f.api.HISTORY_LIMITS.deadlineMs); await flush(); f.expire(f.api.HISTORY_LIMITS.cleanupMs); await check;
  assert.equal(r.calls.locked, true); assert.equal(r.calls.release, 0); await assert.rejects(pager.load(), /cleanup is unconfirmed/); assert.equal(f.calls.length, 1);
  cancel.resolve(); await flush(); assert.equal(r.calls.release, 1); await assert.rejects(pager.load(), /cleanup is unconfirmed/); assert.equal(f.calls.length, 1); pager.dispose(); assert.equal(f.timers.size, 0);
});
test('read refusing to settle retains lock even after cancel settles; late read rejection is observed', async () => {
  const f = await fixture(), read = defer(), r = response([], { read: () => read.promise }); f.fetch = async () => r.value;
  const pending = f.read(), check = assert.rejects(pending, /cleanup is unconfirmed/); await flush(); f.budget.cancel(); await flush(); f.expire(f.api.HISTORY_LIMITS.cleanupMs); await check;
  assert.equal(r.calls.cancel, 1); assert.equal(r.calls.release, 0); assert.equal(r.calls.locked, true);
  read.reject(Error('late read failure')); await flush(); assert.equal(r.calls.release, 1); f.budget.close();
});
test('EOF plus malformed JSON and invalid MIME never count as missing; only unfinished body is cancelled', async () => {
  for (const malformed of [true, false]) {
    const f = await fixture(), r = response([encoded(malformed ? '{bad' : '{}')], malformed ? {} : { mime: 'text/html; application/json' }); f.fetch = async () => r.value;
    await assert.rejects(f.read(), malformed ? /JSON/ : /non-JSON/); assert.equal(r.calls.cancel, malformed ? 0 : 1); assert.equal(r.calls.release, 1); assert.equal(f.calls[0].options.signal.aborted, !malformed); f.budget.close();
  }
});
test('executed rejected transport counterfactual accepts unlimited empty fragments and releases before cancellation settles', async () => {
  const f = await fixture({ oldTransport: true }), r = response([...Array.from({ length: 1000 }, () => new Uint8Array(0)), encoded('{}')]); f.fetch = async () => r.value;
  assert.ok((await f.read()).body); assert.equal(r.calls.read, 1002); assert.deepEqual(f.allocations, [2]); f.budget.close();
  const g = await fixture({ oldTransport: true }), cancel = defer(), bad = response([], { mime: 'text/html', cancel: () => cancel.promise }); g.fetch = async () => bad.value;
  await assert.rejects(g.read(), /non-JSON/); assert.equal(bad.calls.cancel, 1); assert.equal(bad.calls.release, 1); cancel.resolve(); await flush(); g.budget.close();
});
test('executed rejected pager counterfactual sorts timestamps lexically and overscans limit 99', async () => {
  const f = await fixture({ oldPagination: true }); f.budget.close();
  const key = (id, readAt) => ({ id, mangaId: 'parent', readAt });
  const pager = new f.api.HistoryPager([key('a', '2026-01-01T00:00:00.900Z'), key('z', '2026-01-01T00:00:00Z')], async keys => keys.map(row => ({ ...row, relationships: { manga: { id: row.mangaId } } })), () => {});
  assert.deepEqual(Array.from((await pager.load()).data, row => row.id), ['z', 'a']);
  const sparse = new f.api.HistoryPager(Array.from({ length: 2000 }, (_, i) => key(String(i), '2026-01-01T00:00:00Z')), async () => [], () => {}, 99);
  assert.equal((await sparse.load()).scanned, 1089);
});

test('abort during the first body read cannot start a second read or return its bytes', async () => {
  const f = await fixture(), r = response([], { read: async () => { f.budget.cancel(); return { done: false, value: encoded('{}') }; } }); f.fetch = async () => r.value;
  await assert.rejects(f.read(), /cancelled/); assert.equal(r.calls.read, 1); assert.equal(r.calls.cancel, 1); assert.equal(r.calls.release, 1); assert.equal(f.budget.bytes, 0); f.budget.close();
});
test('actual ReadableStream lock remains held until asynchronous cancellation settles', async () => {
  const f = await fixture(), cancel = defer(); let started = 0, releases = 0;
  const value = new Response(new ReadableStream({ pull: () => new Promise(() => {}), cancel() { started++; return cancel.promise; } }), { headers: { 'Content-Type': 'application/json' } });
  const get = value.body.getReader.bind(value.body); value.body.getReader = () => { const reader = get(), release = reader.releaseLock.bind(reader); reader.releaseLock = () => { releases++; return release(); }; return reader; }; f.fetch = async () => value;
  let ended = false; const pending = f.read().finally(() => { ended = true; }), check = assert.rejects(pending, /cancelled/);
  await flush(); f.budget.cancel(); await flush(); assert.equal(started, 1); assert.equal(ended, false); assert.equal(value.body.locked, true); assert.equal(releases, 0);
  cancel.resolve(); await check; assert.equal(value.body.locked, false); assert.equal(releases, 1); f.budget.close();
});
