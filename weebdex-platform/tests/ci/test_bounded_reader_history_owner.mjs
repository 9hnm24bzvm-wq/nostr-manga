// SOURCE-AUTHORED, UNEXECUTED: actual local history owner and IDB request/terminal
// helpers, with a finite event-controlled IDB adapter. No HTTP, relay or service.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
const root = fileURLToPath(new URL('../../', import.meta.url)), fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
const turns = async () => { for (let i = 0; i < 60; i++) await Promise.resolve(); };
async function fixture({ ignoredAbort = false, hold = false } = {}) {
  const timers = new Map(), transactions = [], keys = [], records = new Map();
  const scope = { origin: 'https://reader.test', serviceKey: null, catalogNamespace: 'a'.repeat(32) };
  const source = { admission: { selectionEpoch: 0, authority: { actor: undefined, epoch: 0 } }, selectionPending: false, context: { generation: 1, mode: 'device' }, ready: true, mode: 'device', revision: 7, accountRevision: 0, scopeStatus: 'verified', scope, reason: undefined };
  const meta = { revision: 7, instanceScope: scope };
  source.db = { transaction(names, mode) {
    assert.deepEqual([...names], ['records', 'meta']); assert.equal(mode, 'readonly');
    const requests = []; const tx = { error: null, aborts: 0, terminal: false,
      objectStore(name) { return { get(key) { keys.push([name, key]); const query = { result: name === 'meta' ? meta[key] : records.get(key[1]) }; requests.push(query); return query; }, getAll() { throw Error('Unbounded local hydration'); }, openCursor() { throw Error('History scans are forbidden'); } }; },
      finish() { if (tx.terminal) return; tx.terminal = true; for (const query of requests) query.onsuccess?.(); tx.oncomplete?.(); },
      abort() { tx.aborts++; if (ignoredAbort || tx.terminal) return; tx.terminal = true; tx.error = Error('IDB abort'); for (const query of requests) { query.error = tx.error; query.onerror?.(); } tx.onabort?.(); }
    }; transactions.push(tx); if (!hold) queueMicrotask(() => tx.finish()); return tx;
  } };
  const context = vm.createContext({ console, AbortController, setTimeout(fn, ms) { const key = {}; timers.set(key, { fn, ms }); return key; }, clearTimeout(key) { timers.delete(key); } });
  const modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (relative === 'platform-integrations/local-library/library.ts') { const mod = new vm.SyntheticModule(['readerLibrarySource', 'assertLibraryAdmission'], function () { this.setExport('readerLibrarySource', () => ({ ...source, admission: { ...source.admission, authority: { ...source.admission.authority } } })); this.setExport('assertLibraryAdmission', expected => { if (expected.selectionEpoch !== source.admission.selectionEpoch || expected.authority.actor !== source.admission.authority.actor || expected.authority.epoch !== source.admission.authority.epoch) throw Error('Library admission changed'); }); }, { context }); modules.set(relative, mod); return mod; }
    const local = path.join(root, relative), file = fs.existsSync(local) ? local : path.join(fallback, relative), sourceCode = fs.readFileSync(file, 'utf8');
    const mod = new vm.SourceTextModule(relative.endsWith('.ts') ? stripTypeScriptTypes(sourceCode, { mode: 'transform' }) : sourceCode, { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts')))))); links.set(relative, linked); await linked; return mod;
  }
  const mod = await load('platform-integrations/bounded-reader/history.ts'); await mod.evaluate();
  const budget = () => { const abort = new AbortController(); return { abort, signal: abort.signal, rows: 0, accept(rows) { this.rows += rows; }, check() { if (abort.signal.aborted) throw Error('Budget cancelled'); } }; };
  return { source, meta, records, transactions, keys, timers, budget, access: () => mod.namespace.localHistoryAccess('manga'), expire() { for (const timer of [...timers.values()]) if (timer.ms === 250) timer.fn(); } };
}
test('only requested completion keys and revision/scope are read in one transaction', async () => {
  const f = await fixture(); f.records.set('two', { family: 'chapters', key: 'two', id: 'two', mangaId: 'manga', completed: true });
  assert.equal(await f.access().completed(['one', 'two'], f.budget()), true); assert.equal(f.transactions.length, 1); assert.equal(f.transactions[0].aborts, 0);
  assert.deepEqual(JSON.parse(JSON.stringify(f.keys)), [['meta', 'revision'], ['meta', 'instanceScope'], ['records', ['chapters', 'one']], ['records', ['chapters', 'two']]]);
});
for (const field of ['revision', 'scope', 'account', 'mode']) test('held local read rejects changed ' + field + ' before result publication', async () => {
  const f = await fixture({ hold: true }), result = f.access().completed(['one'], f.budget()), failure = assert.rejects(result);
  if (field === 'revision') f.source.revision++; else if (field === 'scope') f.source.scope = { ...f.source.scope, catalogNamespace: 'b'.repeat(32) }; else if (field === 'mode') f.source.mode = 'account'; else f.source.context = { ...f.source.context, generation: 2 };
  f.transactions[0].finish(); await failure; assert.equal(f.timers.size, 0);
});
test('same-transaction stored revision mismatch is not accepted as empty history', async () => {
  const f = await fixture(); f.meta.revision++; await assert.rejects(f.access().completed(['one'], f.budget()), /revision or catalogue/);
});
test('ignored abort retains shared slot across remount until requests and transaction actually terminate', async () => {
  const f = await fixture({ hold: true, ignoredAbort: true }), budget = f.budget(), first = f.access().completed(['one'], budget), rejection = assert.rejects(first, /cleanup has not settled/);
  budget.abort.abort(); await turns(); f.expire(); await rejection;
  await assert.rejects(f.access().completed(['two'], f.budget()), /cleanup has not settled/); assert.equal(f.transactions.length, 1);
  f.transactions[0].finish(); await turns(); const second = f.access().completed(['two'], f.budget()); f.transactions[1].finish(); assert.equal(await second, false);
});
test('cancelled late local rejection has no unhandled cancellation and cannot publish', async () => {
  const unhandled = [], observe = error => unhandled.push(error); process.on('unhandledRejection', observe);
  try { const f = await fixture({ hold: true }), budget = f.budget(), result = f.access().completed(['one'], budget), failure = assert.rejects(result); budget.abort.abort(); await failure; await turns(); assert.deepEqual(unhandled, []); } finally { process.off('unhandledRejection', observe); }
});
for (const status of ['pending', 'offline', 'migration_required']) test(status + ' history and account mode are never silent device/empty fallbacks', async () => {
  const f = await fixture(); f.source.scopeStatus = status; await assert.rejects(f.access().completed(['one'], f.budget())); assert.equal(f.transactions.length, 0);
  f.source.scopeStatus = 'verified'; f.source.mode = 'account'; assert.equal(f.access().status, 'unavailable'); await assert.rejects(f.access().completed(['one'], f.budget())); assert.equal(f.transactions.length, 0);
});
test('51 keys fail before creating a storage owner or transaction', async () => { const f = await fixture(); await assert.rejects(f.access().completed(Array.from({ length: 51 }, (_, i) => String(i)), f.budget())); assert.equal(f.transactions.length, 0); });

test('pending device to account selection prevents new local reads before committed mode changes', async () => {
  const f = await fixture(); f.source.selectionPending = true; f.source.admission.selectionEpoch++;
  assert.equal(f.access().status, 'pending'); await assert.rejects(f.access().completed(['one'], f.budget())); assert.equal(f.transactions.length, 0);
});
test('account to device to account selection epoch rejects an old local read even when committed source returns', async () => {
  const f = await fixture({ hold: true }), access = f.access(), pending = access.completed(['one'], f.budget()), rejected = assert.rejects(pending);
  f.source.admission.selectionEpoch += 2; f.transactions[0].finish(); await rejected;
  assert.notEqual(f.access().key, access.key);
});
test('harmless native authority generation refresh does not change the device history read key', async () => {
  const f = await fixture({ hold: true }), access = f.access(), pending = access.completed(['one'], f.budget());
  f.source.admission.authority.generation = 10; assert.equal(f.access().key, access.key); f.transactions[0].finish(); assert.equal(await pending, false);
});
