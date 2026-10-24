import { readerLibrarySource, assertLibraryAdmission } from '../local-library/library';
import { completed, request } from '../local-library/protocol/indexeddb.js';
import type { ReaderBudget } from './client';
import type { CompletionAccess, CompletionRow } from './completion-contract';
// IDB structured-clones records, including equivalent plain records supplied
// by a separately-realm IDB adapter. Network DTOs retain their stricter validator.
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value) && Object.prototype.toString.call(value) === '[object Object]';
const scopeKey = (scope: unknown) => isObject(scope) ? JSON.stringify([scope.origin, scope.serviceKey, scope.catalogNamespace]) : '';
export const historySourceKey = (source: ReturnType<typeof readerLibrarySource>) => JSON.stringify([source.context, source.admission.selectionEpoch, source.admission.authority.actor, source.admission.authority.epoch, source.selectionPending, source.mode, source.ready, source.revision, source.accountRevision, source.scopeStatus, source.scope, source.reason]);
const cleanupMs = 250;
let storageOwner: object | undefined;
export class HistoryOwnerUnsettled extends Error { constructor() { super('Local history cleanup has not settled. Close or reload this reader before retrying; no completion result was accepted.'); } }
export function localHistoryAccess(mangaId: string): CompletionAccess {
  const source = readerLibrarySource();
  const key = historySourceKey(source);
  const status = source.selectionPending ? 'pending' : source.mode === 'account' ? 'unavailable' : !source.ready ? 'pending' : source.scopeStatus !== 'verified' || source.reason || !source.db || !source.scope ? 'incomplete' : 'complete-local';
  const projection = async (ids: readonly string[], budget: ReaderBudget): Promise<readonly CompletionRow[]> => {
      budget.check(); if (status !== 'complete-local' || !source.db || !source.scope) throw Error('Local history is not complete. No empty-history assumption was made.');
      if (ids.length > 50 || new Set(ids).size !== ids.length) throw Error('Local history key-read budget exceeded');
      const current = () => { budget.check(); assertLibraryAdmission(source.admission); const now = readerLibrarySource(); if (now.db !== source.db || historySourceKey(now) !== key) throw Error('Local history changed. Restart the scan.'); };
      current(); if (!ids.length) return Object.freeze([]); if (storageOwner) throw new HistoryOwnerUnsettled();
      // This module-owned slot survives component/controller disposal and remount.
      const owner = storageOwner = {};
      let tx: IDBTransaction;
      try { tx = source.db.transaction(['records', 'meta'], 'readonly'); }
      catch (error) { if (storageOwner === owner) storageOwner = undefined; throw error; }
      const done = completed(tx); let rejectCancellation: (error: Error) => void = () => {};
      const cancelled = new Promise<never>((_, reject) => { rejectCancellation = reject; }); cancelled.catch(() => {});
      const abort = () => { try { tx.abort(); } catch {} };
      const cancel = () => { abort(); rejectCancellation(Error('Local history read cancelled or expired')); };
      const work = (async () => {
        const meta = tx.objectStore('meta'), rows = tx.objectStore('records');
        // Enqueue all bounded keys synchronously in the same readonly transaction.
        const revisionRead = request(meta.get('revision')), scopeRead = request(meta.get('instanceScope'));
        const recordReads = ids.map(id => request(rows.get(['chapters', id])));
        const [revision, scope, records] = await Promise.all([revisionRead, scopeRead, Promise.all(recordReads)]);
        current(); await done; current();
        if (revision !== source.revision || scopeKey(scope) !== scopeKey(source.scope)) throw Error('Local history revision or catalogue changed. Restart the scan.');
        const projection: CompletionRow[] = [];
        for (let i = 0; i < records.length; i++) {
          const row: unknown = records[i]; if (row === undefined) { projection.push(Object.freeze({ id: ids[i], completed: false })); continue; }
          if (!isObject(row) || row.family !== 'chapters' || row.key !== ids[i] || row.id !== ids[i] || row.mangaId !== mangaId || typeof row.completed !== 'boolean') throw Error('Local completion record is invalid or belongs to another title. No empty-history assumption was made.');
          projection.push(Object.freeze({ id: ids[i], completed: row.completed }));
        }
        return Object.freeze(projection);
      })(); work.catch(() => {});
      budget.signal.addEventListener('abort', cancel, { once: true }); if (budget.signal.aborted) cancel();
      let healthy = false;
      try { const result = await Promise.race([work, cancelled]); current(); budget.accept(ids.length); healthy = true; return result; }
      finally {
        budget.signal.removeEventListener('abort', cancel);
        if (healthy) { if (storageOwner === owner) storageOwner = undefined; }
        else {
          abort();
          // A finite caller deadline is not evidence that IDB released its work.
          // Keep the slot until both requests and the transaction are terminal.
          const settled = Promise.allSettled([work, done]).then(() => { if (storageOwner === owner) storageOwner = undefined; return true; });
          let timer: ReturnType<typeof setTimeout> | undefined;
          const grace = new Promise<boolean>(resolve => { timer = setTimeout(() => resolve(false), cleanupMs); });
          const released = await Promise.race([settled, grace]); if (timer !== undefined) clearTimeout(timer);
          if (!released) throw new HistoryOwnerUnsettled();
        }
      }
  };
  return { key, status, reason: source.selectionPending ? 'A library selection is pending. Wait before scanning either history source.' : source.mode === 'account' ? 'Account history completeness is not available here. Start is available; select device history explicitly in Library to scan it.' : source.reason ?? 'Verified, complete device history is required before scanning.',
    projection,
    async completed(ids, budget) { const rows = await projection(ids, budget); budget.check(); assertLibraryAdmission(source.admission); if (historySourceKey(readerLibrarySource()) !== key) throw Error('Local history changed. Restart the scan.'); return rows.some(row => row.completed); }
  };
}
