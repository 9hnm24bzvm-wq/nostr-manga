// Source-only fixture. Executes current projection/controller/transport/IDB helper
// modules when separately admitted. Only storage events, HTTP and library state
// are controlled seams. No application owner is replaced by a no-op.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const root = fileURLToPath(new URL('../../../', import.meta.url));
export const turns = async () => { for (let i = 0; i < 200; i++) await Promise.resolve(); };
export const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
export async function badgeFixture(mode = 'account', count = 51) {
  const sourceRoot = process.env.BADGE_SOURCE_ROOT ?? root, fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
  const calls = [], transactions = [], keys = [], timers = new Map(), gates = [], records = new Map();
  const identity = { origin: 'https://badge.test', catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
  const source = { context: { generation: 1, key: mode, mode, accountId: 'account:one' }, admission: { selectionEpoch: 1, authority: { actor: 'account:one', epoch: 2 } }, selectionPending: false, mode, ready: true, revision: 1, accountRevision: 1, scopeStatus: 'verified', scope: identity };
  const control = { hold: 0, fail: 0, partial: 0, ignoredAbort: false, page: 1, ids: Array.from({ length: count }, (_, i) => 'chapter:' + i) };
  const meta = { revision: 1, instanceScope: identity };
  source.db = { transaction(names, intent) {
    assert.deepEqual([...names], ['records', 'meta']); assert.equal(intent, 'readonly'); const reads = [];
    const tx = { error: null, terminal: false, objectStore(name) { return {
      get(key) { keys.push([name, key]); const request = { result: name === 'meta' ? meta[key] : records.get(key[1]) }; reads.push(request); return request; },
      getAll() { throw Error('Unbounded snapshot forbidden'); }, openCursor() { throw Error('Unbounded scan forbidden'); }
    }; }, finish() { if (tx.terminal) return; tx.terminal = true; for (const read of reads) read.onsuccess?.(); tx.oncomplete?.(); }, abort() { if (control.ignoredAbort || tx.terminal) return; tx.terminal = true; tx.error = Error('Aborted'); for (const read of reads) { read.error = tx.error; read.onerror?.(); } tx.onabort?.(); } };
    transactions.push(tx); if (transactions.length !== control.hold) queueMicrotask(() => tx.finish()); return tx;
  } };
  const snapshot = () => ({ ...source, context: { ...source.context }, admission: { ...source.admission, authority: { ...source.admission.authority } } });
  const envelope = ids => ({ version: 1, mangaId: 'manga:one', catalogIdentity: { catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey }, account: { id: source.admission.authority.actor, epoch: source.admission.authority.epoch }, consistency: 'live-request-v1', continueProof: 'observed-only', data: ids.map(id => ({ id, completed: id.endsWith(':0') })) });
  const context = vm.createContext({ console, URL, URLSearchParams, AbortController, TextEncoder, TextDecoder, Uint8Array, queueMicrotask,
    setTimeout(fn, ms) { const key = {}; timers.set(key, { fn, ms }); return key; }, clearTimeout(key) { timers.delete(key); },
    async fetch(url, options) {
      const parsed = new URL(url), ordinal = calls.length + 1; calls.push({ url: parsed, options });
      const body = envelope(parsed.searchParams.getAll('id'));
      if (control.partial === ordinal) body.data.pop();
      if (control.hold === ordinal) { const gate = deferred(); gates.push(gate); await gate.promise; }
      return new Response(JSON.stringify(body), { status: control.fail === ordinal ? 500 : 200, headers: { 'content-type': 'application/json' } });
    }
  });
  const modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (relative === 'platform-integrations/local-library/library.ts') {
      const values = { readerLibrarySource: snapshot, assertLibraryAdmission(expected) { assert.equal(expected.selectionEpoch, source.admission.selectionEpoch); assert.equal(expected.authority.actor, source.admission.authority.actor); assert.equal(expected.authority.epoch, source.admission.authority.epoch); } };
      const mod = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context }); modules.set(relative, mod); return mod;
    }
    const local = path.join(sourceRoot, relative), file = fs.existsSync(local) ? local : path.join(fallback, relative);
    const text = fs.readFileSync(file, 'utf8');
    const mod = new vm.SourceTextModule(relative.endsWith('.ts') ? stripTypeScriptTypes(text, { mode: 'transform' }) : text, { context, identifier: relative }); modules.set(relative, mod);
    const link = Promise.resolve().then(() => mod.link((specifier, importer) => load(path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts')))))); links.set(relative, link); await link; return mod;
  }
  const local = await load('platform-integrations/bounded-reader/history.ts'); await local.evaluate();
  const native = await load('platform-integrations/bounded-reader/native-history.ts'); await native.evaluate();
  const badges = await load('platform-integrations/bounded-reader/visible-badges.ts'); await badges.evaluate();
  const nativeAccess = native.namespace.createNativeHistoryAccess(() => ({ origin: identity.origin, base: '/api', profile: 'native-aggregate-v1' }));
  const budgets = [], views = [];
  const make = () => {
    const instance = new badges.namespace.VisibleChapterBadges(() => ({ key: JSON.stringify([local.namespace.historySourceKey(snapshot()), identity, control.page, control.ids]), target: JSON.stringify([source.context, source.admission, control.page]), ids: control.ids }), async budget => {
      budgets.push(budget); const access = source.mode === 'account' ? nativeAccess('manga:one', identity) : local.namespace.localHistoryAccess('manga:one');
      assert.equal(typeof access.projection, 'function', 'Old source does not expose actual per-ID coverage'); return access;
    });
    instance.subscribe(value => views.push(value)); return instance;
  };
  return { controller: make(), make, source, identity, meta, control, records, calls, transactions, keys, budgets, views, gates,
    expireCleanup() { for (const timer of [...timers.values()]) if (timer.ms === 250) timer.fn(); },
    expireDeadline() { for (const timer of [...timers.values()]) if (timer.ms === 10000) timer.fn(); },
    settleHeld() { gates.forEach(gate => gate.resolve()); transactions.forEach(tx => tx.finish()); }
  };
}
