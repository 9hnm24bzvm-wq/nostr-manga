import { nativeContinueFilters, type NativeContinueInput } from '../public-api/owned-json';
import { chapterRow, ReaderIdentityChanged, sameRow, type Row } from './contract';
export type NativeContinueResult = Readonly<{
  scope: string; state: 'ready' | 'boundary'; reason: 'oldest-unread' | 'all-completed-newest' | 'empty';
  winner: Row | null; hasMultipleEditions: boolean; accountRevision?: number;
}>;
function plain(value: unknown): value is Record<string, unknown> { return typeof value === 'object' && value !== null && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype; }
function object(value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (!plain(value) || Object.keys(value).length !== keys.length || Object.keys(value).some(key => !keys.includes(key))) throw Error('Invalid native Continue response');
  return value;
}
// This reproduces the pinned server's canonical eligibility sets, not chapter
// ordering. All admitted filter values are ASCII, so Rust and JS ordering agree.
let digestOwner: object | undefined;
export async function nativeContinueScope(input: NativeContinueInput, check: () => void, signal?: AbortSignal): Promise<string> {
  check(); if (signal?.aborted) throw Error('Native Continue scope cancelled');
  if (digestOwner) throw Error('Native Continue scope computation has not settled');
  const query = nativeContinueFilters(input.filters), effective: Record<string, string[]> = {};
  for (const [key, value] of query) (effective[key] ??= []).push(value);
  const bytes = new TextEncoder().encode(JSON.stringify([
    { catalogNamespace: input.catalogNamespace, serviceKey: input.serviceKey },
    'native-aggregate-v1', 'native-continue', input.mangaId, input.accountId, effective
  ]));
  if (bytes.byteLength > 32 * 1024) throw Error('Native Continue scope budget exceeded');
  check(); const owner = digestOwner = {};
  // WebCrypto cannot be aborted. A caller may stop waiting, but remounts cannot
  // accumulate digest work: only actual promise settlement releases this slot.
  const work = Promise.resolve().then(() => { check(); return crypto.subtle.digest('SHA-256', bytes); });
  const settled = work.then(value => { if (digestOwner === owner) digestOwner = undefined; return value; }, error => { if (digestOwner === owner) digestOwner = undefined; throw error; });
  let rejectAbort: (error: Error) => void = () => {};
  const aborted = new Promise<never>((_, reject) => { rejectAbort = reject; });
  const cancel = () => rejectAbort(Error('Native Continue scope cancelled; pending computation remains owned'));
  signal?.addEventListener('abort', cancel, { once: true }); if (signal?.aborted) cancel();
  let digest: ArrayBuffer;
  try { digest = await Promise.race([settled, aborted]); check(); }
  finally { signal?.removeEventListener('abort', cancel); }
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('');
}
export function nativeContinueResult(value: unknown, input: NativeContinueInput, scope: string): NativeContinueResult {
  const body = object(value, ['version', 'operation', 'orderingProfile', 'mangaId', 'catalogIdentity', 'account', 'requestId', 'scope', 'consistency', 'continueProof', 'crossRequestSnapshot', 'state', 'reason', 'target', 'winner', 'hasMultipleEditions']);
  const identity = object(body.catalogIdentity, ['catalogNamespace', 'serviceKey']), account = object(body.account, ['id', 'epoch']);
  if (identity.catalogNamespace !== input.catalogNamespace || identity.serviceKey !== input.serviceKey || account.id !== input.accountId || account.epoch !== input.accountEpoch) throw new ReaderIdentityChanged('Native account or catalogue identity changed. Reload before continuing; no fallback was made.');
  if (body.version !== 1 || body.operation !== 'native-continue' || body.orderingProfile !== 'native-aggregate-v1' || body.mangaId !== input.mangaId ||
      body.requestId !== input.requestId || body.scope !== scope || !/^[0-9a-f]{64}$/.test(scope) || body.consistency !== 'live-request-v1' ||
      body.continueProof !== 'native-account-snapshot-v1' || body.crossRequestSnapshot !== false || typeof body.hasMultipleEditions !== 'boolean') throw Error('Native Continue request witnesses did not match');
  if (body.state === 'boundary') {
    if (body.reason !== 'empty' || body.target !== null || body.winner !== null || body.hasMultipleEditions) throw Error('Invalid native Continue boundary');
    return Object.freeze({ scope, state: 'boundary', reason: 'empty', winner: null, hasMultipleEditions: false });
  }
  if (body.state !== 'ready' || body.reason !== 'oldest-unread' && body.reason !== 'all-completed-newest') throw Error('Invalid native Continue selection state');
  const winner = chapterRow(body.winner, 'native-aggregate-v1'), target = object(body.target, ['volume', 'chapter', 'anchorChapterId']);
  if (target.anchorChapterId !== winner.id || target.volume !== winner.volume || target.chapter !== winner.chapter) throw Error('Native Continue target does not match its winner');
  return Object.freeze({ scope, state: 'ready', reason: body.reason, winner, hasMultipleEditions: body.hasMultipleEditions });
}
export const sameContinueSelection = (a: NativeContinueResult, b: NativeContinueResult) => a.scope === b.scope && a.accountRevision === b.accountRevision && a.state === b.state && a.reason === b.reason && a.hasMultipleEditions === b.hasMultipleEditions && sameRow(a.winner, b.winner);
export function singletonContinueFilters(original: string, selected: string): string {
  const query = nativeContinueFilters(original), ids = query.getAll('id');
  // Native id values form an OR set. Replace only after proving intersection;
  // append would widen that set. No exclusion or unrelated filter is removed.
  if (ids.length && !ids.includes(selected)) throw Error('The selected edition is outside the original ID filter');
  query.delete('id'); query.set('id', selected); return nativeContinueFilters(query.toString()).toString();
}
