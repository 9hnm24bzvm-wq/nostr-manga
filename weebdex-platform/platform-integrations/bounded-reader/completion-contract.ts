import { ReaderIdentityChanged, reference } from './contract';
import type { HistoryAccess } from './controller';
import type { ReaderBudget } from './client';
export type CompletionRow = Readonly<{ id: string; completed: boolean }>;
export type CompletionAccess = HistoryAccess & { projection(ids: readonly string[], budget: ReaderBudget): Promise<readonly CompletionRow[]> };
import type { NativeCompletionInput } from '../public-api/owned-json';
function exactRecord(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype && Object.keys(value).length === keys.length && Object.keys(value).every(key => keys.includes(key));
}
function record(value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (!exactRecord(value, keys)) throw Error('Invalid native completion response. No missing key was interpreted as unread.');
  return value;
}
// Only selected server IDs enter this seam. Device records/completion values
// are never uploaded, and native history membership is not progress.completed.
export function nativeCompletionRows(value: unknown, expected: NativeCompletionInput): readonly CompletionRow[] {
  const body = record(value, ['version', 'mangaId', 'catalogIdentity', 'account', 'consistency', 'continueProof', 'data']);
  const identity = record(body.catalogIdentity, ['catalogNamespace', 'serviceKey']);
  if (identity.catalogNamespace !== expected.catalogNamespace || identity.serviceKey !== expected.serviceKey) throw new ReaderIdentityChanged('The native completion catalogue changed. Reload before scanning; saved history was not reinterpreted.');
  const account = record(body.account, ['id', 'epoch']);
  if (account.id !== expected.accountId || account.epoch !== expected.accountEpoch) throw new ReaderIdentityChanged('The native completion account changed. Reload before scanning; no account or device fallback was used.');
  if (body.version !== 1 || body.mangaId !== expected.mangaId || body.consistency !== 'live-request-v1' || body.continueProof !== 'observed-only' || !Array.isArray(body.data) || body.data.length !== expected.ids.length || body.data.length < 1 || body.data.length > 50) throw Error('Native completion coverage is incomplete or invalid. No empty-history assumption was made.');
  const rows: CompletionRow[] = [];
  for (let index = 0; index < body.data.length; index++) {
    const row = record(body.data[index], ['id', 'completed']);
    if (reference(row.id, 'native-aggregate-v1') !== expected.ids[index] || typeof row.completed !== 'boolean') throw Error('Native completion rows do not exactly cover the requested IDs in order.');
    rows.push(Object.freeze({ id: expected.ids[index], completed: row.completed }));
  }
  return Object.freeze(rows);
}

// Existing Continue group reducer retains its any-completed semantics.
export function nativeCompletionResult(value: unknown, expected: NativeCompletionInput): boolean {
  return nativeCompletionRows(value, expected).some(row => row.completed);
}
