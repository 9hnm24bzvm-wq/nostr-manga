// Private owner scheduling extensions, not MangaDex publication-success parity.
import { PublicApiContractError, publicUuid, record, resource } from './validation';
import type { Resource } from './types';
export function scheduledUtc(value: unknown): string {
  if (typeof value !== 'string' || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d$/.test(value) || value.startsWith('0000')) throw new PublicApiContractError('Enter UTC as YYYY-MM-DDTHH:MM:SS, without a timezone suffix.', 422);
  const date = new Date(value + 'Z');
  if (!Number.isFinite(date.valueOf()) || date.toISOString().slice(0, 19) !== value) throw new PublicApiContractError('The UTC date or time is invalid.', 422);
  return value;
}
export type ScheduleReceipt = { chapterId: string; state: 'scheduled' | 'starting' | 'blocked' | 'complete' | 'cancelled'; publicationStatus: string; publishAt?: string; retryWindowSeconds: number; canCancel: boolean; canRetry: boolean; sourceAccepted: boolean | null; readerAvailable: boolean; blockedReason?: string; sourceState?: string; resource: Resource };
export type ScheduleEntry = { sessionId: string; receipt: ScheduleReceipt; uncertain?: 'cancel' | 'retry' };
export type RememberedSchedule = { sessionId: string; chapterId: string; publishAt?: string; uncertain?: 'cancel' | 'retry' };
export type ScheduleLimits = { maximumOutstanding: number; maximumRetainedBytes: number; maximumHorizonSeconds: number; retryWindowSeconds: number; timezone: 'UTC' };
function fail(message: string, status = 502): never { throw new PublicApiContractError(message, status); }
const integer = (value: unknown) => { if (!Number.isSafeInteger(value) || Number(value) < 1) fail('Invalid scheduling limit'); return Number(value); };
const flag = (value: unknown) => { if (typeof value !== 'boolean') fail('Invalid scheduling authority flag'); return value as boolean; };
export function scheduleReceipt(value: unknown): ScheduleReceipt {
  const body = record(value), chapter = resource(body, 'chapter'), facts = record(body.nostr), state = facts.scheduleState;
  if (facts.scheduleAccepted !== true || !['scheduled', 'starting', 'blocked', 'complete', 'cancelled'].includes(String(state)) || !['scheduled', 'blocked', 'source_accepted', 'readable', 'completed_unavailable'].includes(String(facts.publicationStatus))) fail('Invalid private schedule receipt');
  const sourceAccepted = facts.sourceAccepted;
  if (sourceAccepted !== null && typeof sourceAccepted !== 'boolean') fail('Invalid scheduled source acceptance');
  const readerAvailable = flag(facts.readerAvailable), canCancel = flag(facts.canCancel), canRetry = facts.canRetry === undefined ? false : flag(facts.canRetry), retryWindowSeconds = integer(facts.retryWindowSeconds);
  if (retryWindowSeconds > 86400 || state === 'scheduled' && !canCancel || state === 'starting' && canCancel || canRetry && state !== 'blocked' || canCancel && ['complete', 'cancelled'].includes(String(state)) || readerAvailable && (state !== 'complete' || facts.publicationStatus !== 'readable')) fail('Inconsistent private schedule receipt');
  if (state !== 'complete' && (chapter.relationships.length || chapter.attributes.pages !== 0 || chapter.attributes.version !== undefined || chapter.attributes.readableAt !== null || chapter.attributes.nostr !== undefined || facts.eventId !== undefined)) fail('Unpublished schedule contains a chapter-success presentation');
  if (state !== 'complete') { for (const field of ['volume', 'chapter', 'title']) if (chapter.attributes[field] !== null && typeof chapter.attributes[field] !== 'string') fail('Invalid scheduled metadata'); if (typeof chapter.attributes.translatedLanguage !== 'string' || !chapter.attributes.translatedLanguage) fail('Invalid scheduled language'); }
  if (readerAvailable) { integer(chapter.attributes.version); if (chapter.attributes.isUnavailable === true || chapter.relationships.filter(row => row.type === 'manga').length !== 1) fail('Readable schedule lacks an exact chapter presentation'); }
  for (const field of ['blockedReason', 'sourceState']) if (facts[field] !== undefined && facts[field] !== null && (typeof facts[field] !== 'string' || String(facts[field]).length > 2048)) fail('Invalid scheduling diagnostic');
  return { chapterId: publicUuid(chapter.id), state: state as ScheduleReceipt['state'], publicationStatus: String(facts.publicationStatus), ...(state === 'complete' ? {} : { publishAt: scheduledUtc(chapter.attributes.publishAt) }), retryWindowSeconds, sourceAccepted: sourceAccepted as boolean | null, readerAvailable, canCancel, canRetry, ...(typeof facts.blockedReason === 'string' ? { blockedReason: facts.blockedReason } : {}), ...(typeof facts.sourceState === 'string' ? { sourceState: facts.sourceState } : {}), resource: chapter };
}
type Stored = { sessionId: string; chapterId: string; publishAt?: string; uncertain?: { action: 'cancel' | 'retry'; attempt: string } };
type Response = { status: number; body?: unknown; headers?: Record<string, string | null> };
type Store = { get(key: string): string | null; set(key: string, value: string): void; delete(key: string): void };
export interface UploadSchedules {
  pending(sessionId: string): 'cancel' | 'retry' | undefined;
  list(): Promise<{ entries: ScheduleEntry[]; recent: RememberedSchedule[]; limits: ScheduleLimits }>;
  get(sessionId: string): Promise<ScheduleEntry>;
  cancel(sessionId: string): Promise<ScheduleEntry>;
  retry(sessionId: string): Promise<ScheduleEntry>;
  readableChapter(sessionId: string): Promise<Chapter | undefined>;
}
export function createUploadSchedules(context: { key: string; accountId: string; assert(): void; store: Store; lock<T>(key: string, action: () => T): Promise<T>; attempt(): string; request(path: string, method: 'GET' | 'POST' | 'DELETE', guard: () => void): Promise<Response>; present(receipt: Resource): Promise<Chapter | undefined> }) {
  const account = publicUuid(context.accountId), key = context.key + ':' + account, captured = '?expectedAccountId=' + account;
  function stored(): Stored[] {
    context.assert(); const raw = context.store.get(key); if (raw === null) return [];
    try {
      if (raw.length > 32768) fail('Scheduling recovery state exceeds its budget');
      const rows = JSON.parse(raw); if (!Array.isArray(rows) || rows.length > 64) fail('Invalid scheduling recovery state');
      const result: Stored[] = rows.map(value => { const row = record(value); let uncertain: Stored['uncertain']; if (row.uncertain !== undefined) { const item = record(row.uncertain); if (!['cancel', 'retry'].includes(String(item.action)) || typeof item.attempt !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(item.attempt)) fail('Invalid scheduling recovery attempt'); uncertain = { action: item.action as 'cancel' | 'retry', attempt: item.attempt }; } return { sessionId: publicUuid(row.sessionId), chapterId: publicUuid(row.chapterId), ...(row.publishAt === undefined ? {} : { publishAt: scheduledUtc(row.publishAt) }), ...(uncertain ? { uncertain } : {}) }; });
      if (new Set(result.map(row => row.sessionId)).size !== result.length) fail('Duplicate scheduling recovery identity'); return result;
    } catch { return fail('Saved scheduling recovery is unreadable. No scheduling mutation was sent.', 409); }
  }
  function save(rows: Stored[]) { const encoded = JSON.stringify(rows); context.store.set(key, encoded); if (context.store.get(key) !== encoded) fail('Scheduling recovery was not persisted exactly. No further operation was sent.', 409); }
  const entry = (sessionId: string, receipt: ScheduleReceipt): ScheduleEntry => { const saved = stored().find(row => row.sessionId === sessionId); return { sessionId, receipt: { ...receipt, publishAt: receipt.publishAt ?? saved?.publishAt }, uncertain: saved?.uncertain?.action }; };
  async function remember(sessionId: string, receipt: ScheduleReceipt) {
    const id = publicUuid(sessionId);
    await context.lock(key, () => {
      context.assert(); const all = stored(), before = all.find(row => row.sessionId === id);
      if (before && (before.chapterId !== receipt.chapterId || before.publishAt && receipt.publishAt && before.publishAt !== receipt.publishAt)) fail('Schedule receipt changed its immutable identity', 409);
      // Terminal facts resolve a lost operation; an unchanged/nonterminal GET
      // never proves a dispatched retry/cancellation did not execute later.
      const uncertain = ['complete', 'cancelled'].includes(receipt.state) ? undefined : before?.uncertain;
      const next: Stored = { sessionId: id, chapterId: receipt.chapterId, ...(receipt.publishAt || before?.publishAt ? { publishAt: receipt.publishAt ?? before?.publishAt } : {}), ...(uncertain ? { uncertain } : {}) };
      const rest = all.filter(row => row.sessionId !== id);
      while (rest.length >= 64) { const index = rest.findIndex(row => !row.uncertain); if (index < 0) fail('Unresolved schedule recovery is full. Reconcile existing operations first.', 409); rest.splice(index, 1); }
      save([...rest, next]);
    }); context.assert();
  }
  function checked(response: Response, expected = 200) {
    if (response.status !== expected) fail(`Private scheduling request refused (HTTP ${response.status}). Existing work was kept.`, response.status);
    const cache = new Set((response.headers?.['cache-control'] ?? '').toLowerCase().split(',').map(item => item.trim()));
    if (!cache.has('private') || !cache.has('no-store') || !(response.headers?.vary ?? '').toLowerCase().split(',').map(item => item.trim()).includes('authorization')) fail('Private scheduling response headers were not confirmed');
    return record(response.body);
  }
  const path = (id: string) => `/nostr/uploads/${publicUuid(id)}/schedule`;
  async function get(sessionId: string): Promise<ScheduleEntry> {
    context.assert(); const id = publicUuid(sessionId), body = checked(await context.request(path(id) + captured, 'GET', context.assert)); context.assert();
    if (body.result !== 'ok' || publicUuid(body.uploadSessionId) !== id) fail('Schedule status escaped its exact private session');
    const receipt = scheduleReceipt(body.receipt); await remember(id, receipt); return entry(id, receipt);
  }
  async function mutate(sessionId: string, action: 'cancel' | 'retry') {
    context.assert(); const id = publicUuid(sessionId);
    if (stored().find(row => row.sessionId === id)?.uncertain) fail('This schedule operation is unresolved. Check its saved status; no operation was replayed.', 409);
    const fresh = await get(id); context.assert();
    if (!(action === 'cancel' ? fresh.receipt.canCancel : fresh.receipt.canRetry)) fail('This immutable schedule does not allow that operation.', 409);
    const attempt = context.attempt(); if (!/^[A-Za-z0-9_-]{1,128}$/.test(attempt)) fail('Invalid scheduling attempt');
    const guard = () => { context.assert(); const pending = stored().find(row => row.sessionId === id)?.uncertain; if (pending?.attempt !== attempt || pending.action !== action) fail('Schedule recovery changed in another view. No operation was replayed.', 409); };
    await context.lock(key, () => { context.assert(); const rows = stored(), row = rows.find(row => row.sessionId === id); if (!row || row.uncertain) fail('Schedule recovery changed in another view', 409); row.uncertain = { action, attempt }; save(rows); });
    let dispatched = false;
    const clear = async () => { await context.lock(key, () => { context.assert(); const rows = stored(), row = rows.find(row => row.sessionId === id); if (row?.uncertain?.attempt !== attempt) fail('Schedule recovery changed; its fence was kept', 409); delete row.uncertain; save(rows); }); };
    try {
      guard(); dispatched = true;
      const response = await context.request((action === 'cancel' ? `/upload/${id}` : path(id) + '/retry') + captured, action === 'cancel' ? 'DELETE' : 'POST', guard); guard();
      if (response.status >= 400 && response.status < 500) { const body = record(response.body); if (body.result === 'error' && Array.isArray(body.errors) && body.errors.length && body.errors.every(value => record(value).status === response.status)) { await clear(); fail(`Scheduling operation refused (HTTP ${response.status}).`, response.status); } }
      const body = checked(response, action === 'cancel' ? 200 : 202);
      if (action === 'cancel') { if (body.result !== 'ok') fail('Cancellation was not confirmed'); }
      else {
        const received = scheduleReceipt(body), immutable = (receipt: ScheduleReceipt) => JSON.stringify(Object.keys(receipt.resource.attributes).sort().map(key => [key, receipt.resource.attributes[key]]));
        if (!['scheduled', 'starting'].includes(received.state) || received.canRetry || received.chapterId !== fresh.receipt.chapterId || received.publishAt !== fresh.receipt.publishAt || received.retryWindowSeconds !== fresh.receipt.retryWindowSeconds || (!fresh.receipt.canCancel && received.canCancel) || immutable(received) !== immutable(fresh.receipt)) fail('Retry receipt changed its immutable intent or expected retry state');
      }
      await clear(); return await get(id);
    } catch (error) {
      // Only original dispatch's affirmative unsent failure can remove its fence.
      // A failed later GET must never be mistaken for proof the mutation was unsent.
      if (stored().find(row => row.sessionId === id)?.uncertain?.attempt === attempt && (!dispatched || error && typeof error === 'object' && 'outcome' in error && error.outcome === 'unsent')) await clear();
      throw error;
    }
  }
  const manager: UploadSchedules = {
    pending: id => stored().find(row => row.sessionId === publicUuid(id))?.uncertain?.action,
    get, cancel: id => mutate(id, 'cancel'), retry: id => mutate(id, 'retry'),
    async list() {
      context.assert(); const body = checked(await context.request('/nostr/uploads/schedules' + captured, 'GET', context.assert)); context.assert();
      if (body.result !== 'ok' || !Array.isArray(body.data) || body.data.length > 1000) fail('Invalid private schedule list');
      const values = record(body.limits); if (values.timezone !== 'UTC') fail('Unsupported schedule timezone');
      const limits: ScheduleLimits = { maximumOutstanding: integer(values.maximumOutstanding), maximumRetainedBytes: integer(values.maximumRetainedBytes), maximumHorizonSeconds: integer(values.maximumHorizonSeconds), retryWindowSeconds: integer(values.retryWindowSeconds), timezone: 'UTC' };
      const entries: ScheduleEntry[] = []; const known = stored(), seen = new Set<string>();
      for (const value of body.data) { const row = record(value), id = publicUuid(row.uploadSessionId); if (seen.has(id)) fail('Duplicate private schedule entry'); seen.add(id); const receipt = scheduleReceipt(row.receipt); await remember(id, receipt); entries.push(entry(id, receipt)); }
      // Do not serially fetch 64 missing identities or hide the active list when
      // an old exact status is unavailable. Absence is not publication success.
      const recent = known.filter(row => !seen.has(row.sessionId)).map(row => ({ sessionId: row.sessionId, chapterId: row.chapterId, publishAt: row.publishAt, uncertain: row.uncertain?.action }));
      context.assert(); return { entries, recent, limits };
    },
    async readableChapter(id) { const fresh = await get(id); if (!fresh.receipt.readerAvailable) return; const chapter = await context.present(fresh.receipt.resource); context.assert(); if (chapter && publicUuid(chapter.id) !== fresh.receipt.chapterId) fail('Reader presentation changed its chapter'); return chapter; },
  };
  return { manager, remember };
}
