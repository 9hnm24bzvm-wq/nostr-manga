import { createUploadSchedules, scheduleReceipt, scheduledUtc, type UploadSchedules, type ScheduleReceipt } from './upload-schedules';
import { publicWriteLanguage } from './language';
// Explicit browser-account upload adapter. Shared canonical lifecycle, byte hashes, CAS and unknown-outcome fences; no signer or authentication fallback.
import { onDestroy, createContext } from 'svelte';
import { Error as VisibleError } from '$lib/constants/Error';
import { get } from 'svelte/store';
import { Session } from '$lib/stores';
import { createNativeContributionScope, type NativeContributionScope, type NativeContributionDependencies } from './native-contribution-scope';
const canonicalOrigin=(value:string)=>new URL(value).origin;
import { PublicUploadRequestError, type PublicUploadRequest, type PublicUploadResponse } from './nostr-upload-request';
import { serializeUploadMultipart, serializeUploadReplacementMultipart, type UploadImageFile } from './upload-multipart';
import { publicCatalogueRead } from './site-client';
import { chapterEditAffordances, type ChapterEditAccess } from './chapter-access';
import {createChapterCommandFence,chapterUnconfirmed,type ChapterFencePhase} from './native-chapter-command-fence';
import type {Resource} from './types';
import { publicUuid, record, resource, PublicApiContractError } from './validation';
export const [getUploadOwner, setUploadOwner] = createContext<PublicUploadOwner>();

type Scope = { origin: string; catalogNamespace: string };
type Response<T = unknown> = { status: number; body?: T; headers?: Record<string, string | null>; accepted?: true; sessionId?: string; publicationStatus?: string; receipt?: unknown; scheduled?: ScheduleReceipt };
export type UploadSnapshot = { draft: ChapterDraft; page_order: string[] };
export type UploadAccountFacts = { canBegin: boolean; canManage: boolean; publication: Record<string, unknown>; limits: Record<string, unknown>; permissions: string[] };
type FileFacts = { mimeType: string; fileSize: number; dimensions?: { width: number; height: number }; previewUrl?: string };
declare global {
  interface User { permissions?: string[] }
  interface UploadSessionFile {
    public_upload?: FileFacts;
    upload_recovery?: { state: 'unsent' | 'rejected' | 'unknown'; session: string; before_ids: string[]; hash?: string };
  }
}
type PublicFile = UploadSessionFile & { public_upload?: FileFacts };
export type PublicSession = UploadSession & { manga_id: string; chapter_id?: string; edit_version?: number; draft?: ChapterDraft | null; page_order: string[]; can_mutate: boolean; can_cancel: boolean; can_retry_commit: boolean; is_expired: boolean; expires_at: string | null; state: string; schedule?: ScheduleReceipt; publication_status?: string; target_status: string; receipt?: Chapter; committed_result?: Chapter; public_upload: { groupIds: string[]; receipt: unknown; receiptStatus: 'none' | 'available' | 'unavailable'; commitState?: string } };
type ChapterBeginFence = { chapterId: string; parentId: string; version: number; replace: boolean; sessionId?: string };
type Fence = { sessionId: string; state: 'uncertain' | 'accepted-unavailable'; attempt?: string; schedule?: { accountId: string; publishAt: string; fingerprint: string } };
export type UploadReplacementStatus = Readonly<{ state: 'uncertain'; sessionId: string; fileId: string; expectedVersion: number }>;
export type UploadReplacementResult = { session: PublicSession; file: UploadSessionFile };
type ReplacementFile = { id: string; name: string; hash: string; size: number; mime: string };
type ReplacementFence = UploadReplacementStatus & { attempt: string; targetId: string; targetKind: 'manga' | 'chapter'; mangaId: string; chapterId?: string; draftHash: string; groupIds: string[]; order: string[]; before: ReplacementFile[]; replacement: ReplacementFile };
export interface UploadFenceStore { get(key: string): string | null; set(key: string, value: string): void; delete(key: string): void }
export interface PublicUploadOwner {
  readonly id: string; readonly actor: string; readonly schedulingAvailable:boolean; readonly schedules: UploadSchedules;
  assert(): void; alive(): boolean; dispose(): void;
  commitState(): Fence['state'] | undefined;
  pendingCommitSession(): string | undefined;
  pendingReplacement(): UploadReplacementStatus | undefined;
  chapterAccess(version:number):Promise<ChapterEditAccess>;
  chapterPagesPending(): boolean;
  chapterBeginPending(): boolean;
  reconcileChapterBegin(): Promise<PublicSession>;
  chapterPending():boolean;
  chapterPendingPhase():ChapterFencePhase|undefined;
  request<T = unknown>(route: string, method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE', payload?: unknown): Promise<Response<T>>;
}
export interface PublicUploadDependencies {
  nativeAccount(): string;
  nativeDependencies: NativeContributionDependencies;
  publicOrigin(): string;
  catalogueRead(operation: string, options: { id: string }): Promise<Response>;
  multipart: typeof serializeUploadMultipart;
  replacementMultipart: typeof serializeUploadReplacementMultipart;
  digest(bytes: Uint8Array): Promise<string>;
  decodeImage(blob: Blob): Promise<{ width: number; height: number }>;
  destroy(callback: () => void): void;
  newAttempt(): string;
  fences: UploadFenceStore;
  replacementFences: UploadFenceStore;
  chapterFences: UploadFenceStore;
  scheduleFences: UploadFenceStore;
  scheduleLock<T>(key: string, action: () => T): Promise<T>;
  replacementLock<T>(key: string, action: () => T): Promise<T>;
}
const defaultFences: UploadFenceStore = {
  get: key => window.sessionStorage.getItem(key),
  set: (key, value) => window.sessionStorage.setItem(key, value),
  delete: key => window.sessionStorage.removeItem(key),
};
const defaults: PublicUploadDependencies = {
  nativeAccount: () => get(Session)?.identity?.id ?? "", nativeDependencies: {}, publicOrigin: () => window.location.origin,
  catalogueRead: (operation, options) => publicCatalogueRead(operation, options),
  multipart: serializeUploadMultipart, replacementMultipart: serializeUploadReplacementMultipart,
  digest: async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new Uint8Array(bytes)))].map(byte => byte.toString(16).padStart(2, '0')).join(''),
  decodeImage: async blob => { const image = await createImageBitmap(blob); try { return { width: image.width, height: image.height }; } finally { image.close(); } },
  destroy: onDestroy, newAttempt: () => crypto.randomUUID(), fences: defaultFences, chapterFences: defaultFences,
  scheduleLock: async <T>(key: string, action: () => T): Promise<T> => { const locks = globalThis.navigator?.locks; if (!locks) fail('Cross-tab scheduling locking is unavailable. Saved recovery was kept.', 409); return await locks.request(key, { mode: 'exclusive', ifAvailable: true }, lock => { if (!lock) fail('Scheduling recovery is busy in another view.', 409); return action(); }); },
  scheduleFences: { get: key => window.localStorage.getItem(key), set: (key, value) => window.localStorage.setItem(key, value), delete: key => window.localStorage.removeItem(key) },
  replacementFences: { get: key => window.localStorage.getItem(key), set: (key, value) => window.localStorage.setItem(key, value), delete: key => window.localStorage.removeItem(key) },
  replacementLock: async <T>(key: string, action: () => T): Promise<T> => { const locks = globalThis.navigator?.locks; if (!locks) fail('Cross-tab replacement locking is unavailable. Existing replacement recovery state was kept.', 409); return await locks.request(key, { mode: 'exclusive', ifAvailable: true }, lock => { if (!lock) fail('Page replacement recovery is busy in another view. Existing recovery state was kept.', 409); return action(); }); },
};
const encoder = new TextEncoder();
const unknownPage = 'Outcome unknown. This image may already be staged. An exact owner receipt is required before removing or replacing it. No image POST was replayed.';
const unknownReplacement = 'This page replacement is unresolved. The original page and preview were kept. Writes are frozen until an explicit exact-session read confirms the replacement. No replacement POST was replayed.';
const unknownCommit = 'The publication outcome is unresolved. This exact upload is read-only until its saved receipt is explicitly reconciled. No commit was replayed.';
const mimeExtensions: Record<string, string> = { 'image/png': '.png', 'image/jpeg': '.jpg', 'image/gif': '.gif', 'image/webp': '.webp' };
function fail(message: string, status = 502): never { throw new PublicApiContractError(message, status); }
// Successful bodies are validated by their routes; errors keep their actual opaque bodies.
// This legacy caller view does not establish or change the wire contract.
function callerView<T>(response: Response): Response<T> { return response as Response<T>; }
function integer(value: unknown, minimum = 0): number { if (!Number.isSafeInteger(value) || Number(value) < minimum) fail('Invalid upload integer'); return Number(value); }
function text(value: unknown, label: string): string { if (typeof value !== 'string' || !value || /[\u0000-\u001f\u007f]/.test(value)) fail(`Invalid ${label}`); return value as string; }
function boolean(value: unknown): boolean { if (typeof value !== 'boolean') fail('Invalid upload authority flag'); return value as boolean; }
function strings(value: unknown): string[] { if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) fail('Invalid account grants'); return [...value] as string[]; }
function ids(value: unknown, maximum = 500): string[] { if (!Array.isArray(value) || value.length > maximum) fail('Invalid upload UUID list'); const result = value.map(publicUuid); if (new Set(result).size !== result.length) fail('Duplicate upload UUID'); return result; }
function draft(input: unknown): ChapterDraft { const row = record(input, 'upload draft'); const output: ChapterDraft = {}; for (const [key, value] of Object.entries(row)) { if (!['title', 'volume', 'chapter', 'language'].includes(key) || value !== null && typeof value !== 'string') fail('Invalid saved chapter draft'); if (typeof value === 'string') Object.assign(output, { [key]: value }); } return output; }
function actualError(body: unknown): string | undefined { if (!body || typeof body !== 'object') return; const row = body as Record<string, unknown>; if (typeof row.error === 'string') return row.error; if (Array.isArray(row.errors)) { const first = row.errors[0]; if (first && typeof first === 'object' && typeof first.detail === 'string') return first.detail; } }
export function resumeError(status?: number, body?: unknown): VisibleError { return new VisibleError(status === 403 ? 'Upload authorization refused' : status === 409 ? 'Upload session changed' : 'Upload was not confirmed', actualError(body) ?? 'Your staged work was kept. Reload its exact current state before retrying.'); }
export const snapshot = (input: ChapterDraft, files: UploadSessionFile[]): UploadSnapshot => ({ draft: draft(Object.fromEntries(['volume', 'chapter', 'title', 'language'].flatMap(key => typeof input[key as keyof ChapterDraft] === 'string' ? [[key, input[key as keyof ChapterDraft]]] : []))), page_order: files.map(file => file.id) });
export const signature = (value: UploadSnapshot) => JSON.stringify(value);
const sameDraft = (a?: ChapterDraft | null, b?: ChapterDraft | null) => signature(snapshot(a ?? {}, [])) === signature(snapshot(b ?? {}, []));

// Production uses the defaults above. Tests provide explicit fake transports;
// no test fixture is used as proof that a browser or server accepted an upload.
export function createNativeUploadAdapter(injected: Partial<PublicUploadDependencies> = {}) {
  const deps = { ...defaults, ...injected, chapterFences: injected.chapterFences ?? injected.fences ?? defaults.chapterFences, scheduleFences: injected.scheduleFences ?? injected.fences ?? defaults.scheduleFences, scheduleLock: injected.scheduleLock ?? injected.replacementLock ?? defaults.scheduleLock };
  type Account = { user: User; actor: string; scope: Scope; access: NativeContributionScope; facts: UploadAccountFacts; assertCurrent: () => void; key: string };
  type Owned = { chapterPending?: () => boolean; account: Account; targetId: string; targetKind: 'manga' | 'chapter'; sessionId?: string; sessionEpoch: number; fence?: Fence; commitAttempt?: object; replacement?: ReplacementFence; replacementAttempt?: object; latest?: PublicSession; schedules?: ReturnType<typeof createUploadSchedules>; groups: Map<string, ScanlationGroup>; request: (input: PublicUploadRequest) => Promise<PublicUploadResponse>; assert: () => void };
  const accounts = new WeakMap<object, Account>(), owners = new WeakMap<PublicUploadOwner, Owned>();
  const replacementAuthority = new WeakMap<PublicSession, Pick<PublicSession, 'can_mutate' | 'can_cancel' | 'can_retry_commit'>>();
  function equalScope(a:Scope,b:Scope){return a.origin===b.origin&&a.catalogNamespace===b.catalogNamespace;}
  async function refresh(account:Account){account.access.check();account.assertCurrent();}
  function transport(account:Account,assertCurrent:()=>void){return async(input:PublicUploadRequest)=>{
    assertCurrent();await refresh(account);assertCurrent();
    const url=new URL(input.url);if(url.origin!==account.scope.origin||url.href!==input.url||url.hash||!url.pathname.startsWith('/api/v1/'))throw new PublicUploadRequestError('Upload route escaped its account scope','unsent');
    const route=url.pathname.slice('/api/v1'.length);
    if(input.method==='GET'&&/^\/(?:chapter|group)\/[^/]+$/.test(route)){const response=await account.access.publicRequest(input);assertCurrent();return response;}
    let nativePath:string;
    if(/^\/account\/uploads\/[^/]+\/files\/[^/]+\/preview$/.test(route)&&input.method==='GET')nativePath=route;
    else if(/^\/upload(?:\/|$)/.test(route))nativePath='/account'+route;
    else if(/^\/nostr\/uploads(?:\/|$)/.test(route))nativePath='/account'+route.slice('/nostr'.length);
    else if(/^\/chapter\/[^/]+(?:\/claim)?$/.test(route)&&input.method!=='GET')nativePath='/account'+route;
    else throw new PublicUploadRequestError('Unsupported native upload operation','unsent');
    if(url.searchParams.has('expectedAccountId')&&(url.searchParams.getAll('expectedAccountId').length!==1||url.searchParams.get('expectedAccountId')!==account.actor))throw new PublicUploadRequestError('Upload account capture changed','unsent',409);
    url.pathname='/api/v1'+nativePath;url.searchParams.set('expectedAccountId',account.actor);
    const response=await account.access.request({...input,url:url.href});assertCurrent();return response;
  };}
  const scheduleCommitKey = (account: Account) => account.key.replace('account-upload-recovery.v1:', 'account-upload-scheduled-commit.v1:') + ':' + account.actor;
  function readFence(account: Account): Fence | undefined {
    const ordinary = deps.fences.get(account.key), scheduled = deps.scheduleFences.get(scheduleCommitKey(account));
    if (ordinary !== null && scheduled !== null) fail('Conflicting publication recovery records were kept. Reopen the original account.', 409);
    const stored = scheduled ?? ordinary; if (stored === null) return;
    let row: Record<string, unknown>; try { row = record(JSON.parse(stored)); } catch { return fail('Saved upload recovery state is unreadable. No mutation was sent.', 409); }
    const sessionId = publicUuid(row.sessionId); if (!['uncertain', 'accepted-unavailable'].includes(String(row.state))) fail('Saved upload recovery state is invalid', 409);
    if (row.attempt !== undefined && (typeof row.attempt !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(row.attempt))) fail('Saved upload attempt is invalid', 409);
    let schedule: Fence['schedule'];
    if (scheduled !== null && row.schedule === undefined) fail('Scheduled dispatch recovery lacks its captured intent', 409);
    if (row.schedule !== undefined) { const value = record(row.schedule); if (publicUuid(value.accountId) !== account.actor || typeof value.fingerprint !== 'string' || !/^[0-9a-f]{64}$/.test(value.fingerprint)) fail('Scheduled commit recovery changed its captured account or intent', 409); schedule = { accountId: account.actor, publishAt: scheduledUtc(value.publishAt), fingerprint: value.fingerprint }; }
    return { sessionId, state: row.state as Fence['state'], ...(schedule ? { schedule } : {}), ...(typeof row.attempt === 'string' ? { attempt: row.attempt } : {}) };
  }
  const canonicalRecord = (value: unknown): string => JSON.stringify(value, (_key, row) => row && typeof row === 'object' && !Array.isArray(row) ? Object.fromEntries(Object.entries(row).sort(([a], [b]) => a.localeCompare(b))) : row);
  const matchesRecord = (raw: string | null, expected?: Fence): boolean => { if (raw === null) return expected === undefined; if (!expected) return false; try { return canonicalRecord(JSON.parse(raw)) === canonicalRecord(expected); } catch { return false; } };
  function applyFence(owned: Owned, value?: Fence) {
    owned.fence = value;
    if (!value) { owned.commitAttempt = undefined; if (owned.latest) delete owned.latest.public_upload.commitState; }
    if (owned.latest && value) { owned.latest.can_mutate = false; owned.latest.can_cancel = false; owned.latest.can_retry_commit = false; owned.latest.receipt = undefined; owned.latest.committed_result = undefined; owned.latest.public_upload.commitState = value.state; }
  }
  function fence(owned: Owned, value?: Fence, expected = owned.fence) {
    // Ordinary callbacks own only their captured session-storage record. They
    // cannot clear, overwrite or recreate a global scheduled dispatch marker.
    if (value?.schedule || expected?.schedule || !matchesRecord(deps.fences.get(owned.account.key), expected)) fail('Publication recovery changed; the newer record was kept.', 409);
    if (value) {
      if (deps.scheduleFences.get(scheduleCommitKey(owned.account)) !== null) fail('Another scheduled dispatch owns recovery. Its record was kept.', 409);
      deps.fences.set(owned.account.key, JSON.stringify(value));
    } else deps.fences.delete(owned.account.key);
    applyFence(owned, value);
  }
  // Only call this synchronous storage operation while holding scheduleCommitKey.
  function writeScheduledFenceLocked(owned: Owned, expected?: Fence, value?: Fence) {
    const key = scheduleCommitKey(owned.account);
    if (!matchesRecord(deps.scheduleFences.get(key), expected) || deps.fences.get(owned.account.key) !== null || value && (!value.schedule || expected && canonicalRecord(value.schedule) !== canonicalRecord(expected.schedule))) fail('Scheduled dispatch recovery changed; the newer record was kept.', 409);
    if (value) { const encoded = JSON.stringify(value); deps.scheduleFences.set(key, encoded); if (deps.scheduleFences.get(key) !== encoded) fail('Scheduled dispatch recovery was not persisted exactly. No new dispatch was sent.', 409); }
    else { deps.scheduleFences.delete(key); if (deps.scheduleFences.get(key) !== null) fail('Scheduled dispatch recovery could not be cleared. Its fence was kept.', 409); }
    applyFence(owned, value);
  }
  async function changeScheduledFence(owned: Owned, expected: Fence, value?: Fence, unsent = false) {
    if (!expected.schedule) fail('Scheduled recovery requires a captured intent', 409);
    await deps.scheduleLock(scheduleCommitKey(owned.account), () => { if (!unsent) owned.assert(); writeScheduledFenceLocked(owned, expected, value); });
    if (!unsent) owned.assert();
  }
  const replacementKey = (account: Account) => account.key.replace('account-upload-recovery.v1:', 'account-upload-replacement.v1:');
  function replacementFacts(file: UploadSessionFile): ReplacementFile {
    const name = text(file.original_file_name, 'replacement filename'), hash = text(file.file_hash, 'replacement digest'), mime = text(file.public_upload?.mimeType, 'replacement MIME'), size = integer(Number(file.file_size), 1);
    if (encoder.encode(name).byteLength > 255 || /["\\/]/.test(name) || !/^[0-9a-f]{64}$/.test(hash) || !mimeExtensions[mime] || size > 25 * 1024 * 1024) fail('Invalid replacement file facts', 409);
    return { id: publicUuid(file.id), name, hash, size, mime };
  }
  function readReplacement(account: Account): ReplacementFence | undefined {
    const stored = deps.replacementFences.get(replacementKey(account)); if (stored === null) return;
    try {
      if (stored.length > 320 * 1024) fail('Replacement recovery state exceeds budget', 409);
      const row = record(JSON.parse(stored)), order = ids(row.order), before = row.before;
      if (row.state !== 'uncertain' || typeof row.attempt !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(row.attempt) || !['manga', 'chapter'].includes(String(row.targetKind)) || typeof row.draftHash !== 'string' || !/^[0-9a-f]{64}$/.test(row.draftHash) || !Array.isArray(before) || before.length !== order.length) fail('Invalid replacement recovery context', 409);
      const facts = (value: unknown): ReplacementFile => { const file = record(value); if (typeof file.size !== 'number') fail('Invalid replacement byte count', 409); return replacementFacts({ id: file.id, original_file_name: file.name, file_hash: file.hash, file_size: String(file.size), public_upload: { mimeType: file.mime, fileSize: file.size } } as UploadSessionFile); };
      const expectedVersion = integer(row.expectedVersion, 1); if (expectedVersion > 2147483647) fail('Invalid replacement session version', 409);
      const fileId = publicUuid(row.fileId), previous = before.map(facts), replacement = facts(row.replacement);
      if (!order.includes(fileId) || replacement.id !== fileId || previous.some((file, index) => file.id !== order[index])) fail('Replacement recovery order is inconsistent', 409);
      return { state: 'uncertain', sessionId: publicUuid(row.sessionId), fileId, expectedVersion, attempt: row.attempt, targetId: publicUuid(row.targetId), targetKind: row.targetKind as ReplacementFence['targetKind'], mangaId: publicUuid(row.mangaId), ...(row.chapterId === undefined ? {} : { chapterId: publicUuid(row.chapterId) }), draftHash: row.draftHash, groupIds: ids(row.groupIds, 10), order, before: previous, replacement };
    } catch { fail('Saved page replacement recovery state is unreadable. No mutation was sent.', 409); }
  }
  function replacementPending(owned: Owned): ReplacementFence | undefined {
    const saved = readReplacement(owned.account);
    // A stale owner cannot adopt a different tab's attempt or silently forget its own.
    if (owned.replacement && JSON.stringify(saved) !== JSON.stringify(owned.replacement)) fail('Page replacement recovery changed in another view. Reload the original editor.', 409);
    return saved;
  }
  function freezeReplacement(owned: Owned, session: PublicSession) {
    if (!replacementAuthority.has(session)) replacementAuthority.set(session, { can_mutate: session.can_mutate, can_cancel: session.can_cancel, can_retry_commit: session.can_retry_commit });
    session.can_mutate = false; session.can_cancel = false; session.can_retry_commit = false;
  }
  async function installReplacement(owned: Owned, value: ReplacementFence, token: object, assertCurrent: () => void) {
    await deps.replacementLock(replacementKey(owned.account), () => {
      // Only this short synchronous storage transaction holds the cross-tab lock.
      // Never retain it during signing, network dispatch or an owner read.
      assertCurrent(); assertMutable(owned);
      deps.replacementFences.set(replacementKey(owned.account), JSON.stringify(value)); owned.replacement = value; owned.replacementAttempt = token;
      if (JSON.stringify(readReplacement(owned.account)) !== JSON.stringify(value)) fail('Page replacement fence was not persisted exactly', 409);
      if (owned.latest) freezeReplacement(owned, owned.latest);
    });
    assertCurrent();
  }
  async function clearReplacement(owned: Owned, pending: ReplacementFence, unsent = false, confirmed?: PublicSession, assertCurrent = () => {}) {
    await deps.replacementLock(replacementKey(owned.account), () => {
      if (!unsent) { owned.assert(); assertCurrent(); }
      if (JSON.stringify(readReplacement(owned.account)) !== JSON.stringify(pending) || !unsent && (owned.fence || readFence(owned.account))) fail('Page replacement recovery changed. Its fence was kept.', 409);
      deps.replacementFences.delete(replacementKey(owned.account)); owned.replacement = undefined; owned.replacementAttempt = undefined;
      if (!owned.fence && !readFence(owned.account)) for (const session of new Set([owned.latest, confirmed])) { if (session) { const authority = replacementAuthority.get(session); if (authority) Object.assign(session, authority); } }
    });
    if (!unsent) { owned.assert(); assertCurrent(); assertMutable(owned); }
  }
  function selectSession(owned: Owned, sessionId: string | undefined) {
    if (owned.sessionId !== sessionId) { owned.sessionId = sessionId; owned.sessionEpoch++; }
  }
  function assertSessionEpoch(owned: Owned, expected: number) {
    owned.assert();
    if (owned.sessionEpoch !== expected) fail('The owned upload session changed during its read', 409);
  }
  const ownedBy = (owner?: PublicUploadOwner): Owned => { if (!owner || !owners.has(owner)) fail('An explicit current upload account is required', 409); owner.assert(); return owners.get(owner)!; };
  const chapterBeginKey = (account: Account) => account.key + ':chapter-begin:' + account.actor;
  function readChapterBegin(account: Account): ChapterBeginFence | undefined {
    const saved = deps.fences.get(chapterBeginKey(account)); if (saved === null) return;
    let value: Record<string, unknown>; try { value = record(JSON.parse(saved)); } catch { return fail('Page-edit admission recovery is unreadable. No mutation was sent.', 409); }
    if (Object.keys(value).some(key => !['chapterId','parentId','version','replace','sessionId'].includes(key)) || typeof value.replace !== 'boolean') fail('Invalid page-edit admission recovery', 409);
    return { chapterId: publicUuid(value.chapterId), parentId: publicUuid(value.parentId), version: integer(value.version, 1), replace: value.replace, ...(value.sessionId === undefined ? {} : { sessionId: publicUuid(value.sessionId) }) };
  }
  function writeChapterBegin(account: Account, next: ChapterBeginFence | undefined, expected?: ChapterBeginFence) {
    const key = chapterBeginKey(account), before = readChapterBegin(account);
    if (JSON.stringify(before) !== JSON.stringify(expected)) fail('Page-edit admission recovery changed. Its fence was kept.', 409);
    if (next) deps.fences.set(key, JSON.stringify(next)); else deps.fences.delete(key);
    if (JSON.stringify(readChapterBegin(account)) !== JSON.stringify(next)) fail('Page-edit admission recovery storage is unavailable', 409);
  }
  function assertMutable(owned: Owned, ownChapterCommand = false) {
    if (!ownChapterCommand && owned.chapterPending?.()) fail(chapterUnconfirmed, 409);
    if (readChapterBegin(owned.account)) fail('Page-edit admission is unconfirmed. Explicitly recover the current owned session; no begin request was replayed.', 409);
    if (replacementPending(owned)) throw new VisibleError('Reconcile page replacement first', unknownReplacement);
    if (owned.fence || readFence(owned.account)) throw new VisibleError('Reconcile publication first', unknownCommit);
    if (owned.latest && (owned.targetKind === 'manga' ? owned.latest.manga_id !== owned.targetId || !!owned.latest.chapter_id && !owned.latest.is_committed : owned.latest.chapter_id !== owned.targetId)) fail('The discovered upload belongs to a different editor target. Open its original editor before changing it.', 409);
  }
  async function raw(owned: Owned, path: string, method: PublicUploadRequest['method'], payload?: unknown, responseType: 'json' | 'image' = 'json', ownCommit?: { token: object; sessionId: string; attempt: string }) {
    const guard = () => {
      owned.assert();
      if (method === 'GET') { if (path === '/upload' && (owned.fence || readFence(owned.account) || replacementPending(owned))) throw new VisibleError('Reconcile publication first', unknownCommit); return; }
      if (!ownCommit) return assertMutable(owned, path.startsWith('/chapter/'));
      if (owned.chapterPending?.() || readChapterBegin(owned.account)) fail('A chapter command or page admission is unresolved. Commit recovery was kept.',409);
      if (replacementPending(owned)) throw new VisibleError('Reconcile page replacement first', unknownReplacement);
      const pending = readFence(owned.account);
      if (owned.commitAttempt !== ownCommit.token || pending?.sessionId !== ownCommit.sessionId || pending.state !== 'uncertain' || pending.attempt !== ownCommit.attempt || owned.sessionId !== ownCommit.sessionId) throw new VisibleError('Publication context changed', unknownCommit);
    };
    guard(); const request = method === 'GET' && path !== '/upload' ? owned.request : transport(owned.account, guard);
    const signedPath = method === 'GET' && path === '/upload' || method === 'DELETE' && /^\/upload\/[0-9a-f-]{36}$/i.test(path) ? path + '?expectedAccountId=' + owned.account.actor : path;
    const response = await request({ url: owned.account.scope.origin + '/api/v1' + signedPath, method, responseType, ...(payload === undefined ? {} : { body: encoder.encode(JSON.stringify(payload)), contentType: 'application/json' }) }); guard(); return response;
  }
  function basicSession(input: unknown): string { const row = record(input, 'upload session resource'); if (row.type !== 'upload_session') fail('Invalid upload session resource'); const attrs = record(row.attributes); integer(attrs.version, 1); boolean(attrs.isCommitted); boolean(attrs.isDeleted); boolean(attrs.isProcessed); return publicUuid(row.id); }
  function normalizeFile(input: unknown, sessionId: string, detailed: boolean): PublicFile {
    const source = record(input, 'upload file'), row = detailed ? source : record(source.attributes, 'upload file attributes');
    const id = publicUuid(source.id); if (!detailed && source.type !== 'upload_session_file') fail('Wrong upload file resource kind');
    const name = text(row.originalFileName, 'original filename'), hash = text(row.fileHash, 'original digest'), mime = text(row.mimeType, 'image MIME');
    if (!/^[0-9a-f]{64}$/.test(hash) || !mimeExtensions[mime]) fail('Invalid original image receipt');
    const size = integer(row.fileSize, 1); if (size > 25 * 1024 * 1024) fail('Upload receipt exceeds image budget');
    const facts: FileFacts = { mimeType: mime, fileSize: size };
    if (detailed) { const dims = record(row.dimensions, 'original image dimensions'); facts.dimensions = { width: integer(dims.width, 1), height: integer(dims.height, 1) }; const expected = `/api/v1/account/uploads/${sessionId}/files/${id}/preview`; if (row.previewUrl !== expected) fail('Preview URL escaped its exact upload/file scope'); facts.previewUrl = expected; }
    return { id, original_file_name: name, file_hash: hash, file_name: name, file_ext: mimeExtensions[mime], file_size: String(size), public_upload: facts };
  }
  function normalizeSession(input: unknown, expected: string): PublicSession {
    const envelope = record(input, 'upload snapshot'); if (envelope.result !== 'ok') fail('Invalid upload snapshot envelope'); const row = record(envelope.session, 'upload snapshot session');
    const id = publicUuid(row.id); if (id !== expected) fail('Upload snapshot changed the requested session', 409);
    const editVersion = row.editVersion === null || row.editVersion === undefined ? undefined : integer(row.editVersion, 1);
    const mangaId = publicUuid(row.mangaId), chapterId = row.chapterId === null ? undefined : publicUuid(row.chapterId), groupIds = ids(row.groupIds, 10), order = ids(row.pageOrder);
    if (!Array.isArray(row.files) || row.files.length > 500) fail('Invalid staged image list'); const files = row.files.map(file => normalizeFile(file, id, true));
    if (files.length !== order.length || new Set(files.map(file => file.id)).size !== files.length || order.some(file => !files.some(row => row.id === file))) fail('Staged order does not cover the exact current image set');
    const isCommitted = boolean(row.isCommitted), isProcessed = boolean(row.isProcessed), isDeleted = boolean(row.isDeleted), canMutate = boolean(row.canMutate), canCancel = boolean(row.canCancel), canRetry = boolean(row.canRetryCommit), isExpired = boolean(row.isExpired), state = text(row.state, 'upload state');
    if (state !== 'open' && state !== 'committed' && state !== 'deleted' && state !== 'scheduled' || isCommitted !== (state === 'committed') || isDeleted !== (state === 'deleted') || (isCommitted || state === 'deleted' || isExpired) && (canMutate || canCancel) || canMutate && !canRetry || state === 'scheduled' && (canMutate || canRetry || isExpired || row.expiresAt !== null)) fail('Inconsistent upload state');
    const receipt = row.receipt === null ? null : resource(row.receipt, 'chapter'); if (receipt && (!isCommitted || !receipt.relationships.some(relation => relation.type === 'manga' && publicUuid(relation.id) === mangaId) || chapterId && publicUuid(receipt.id) !== chapterId)) fail('Upload receipt escaped its exact target');
    const receiptStatus = row.receiptStatus; if (receiptStatus !== 'none' && receiptStatus !== 'available' && receiptStatus !== 'unavailable' || !!receipt !== (receiptStatus === 'available') || isCommitted && receiptStatus === 'none' || !isCommitted && (state === 'scheduled' ? receiptStatus !== 'unavailable' : receiptStatus !== 'none')) fail('Invalid exact receipt availability');
    const schedule = row.schedule === undefined ? undefined : scheduleReceipt(row.schedule); if (state === 'scheduled' && !schedule) fail('Scheduled session lacks its private intent receipt');
    const publication = row.publicationStatus === null ? undefined : text(row.publicationStatus, 'publication status');
    return { id, created_at: text(row.createdAt, 'upload creation time'), updated_at: text(row.updatedAt, 'upload update time'), version: integer(row.version, 1), is_committed: isCommitted, is_processed: isProcessed, is_deleted: isDeleted, manga_id: mangaId, ...(chapterId ? { chapter_id: chapterId } : {}), ...(editVersion === undefined ? {} : { edit_version: editVersion }), draft: row.draft === null ? null : draft(row.draft), page_order: order, can_mutate: canMutate, can_cancel: canCancel, can_retry_commit: canRetry, is_expired: isExpired, expires_at: state === 'scheduled' ? null : text(row.expiresAt, 'upload expiry'), state, ...(schedule ? { schedule } : {}), publication_status: publication, target_status: text(row.targetStatus, 'upload target status'), relationships: { files: order.map(id => files.find(file => file.id === id)!) }, public_upload: { groupIds, receipt, receiptStatus } };
  }
  async function exact(owned: Owned, sessionId: string): Promise<PublicSession> {
    owned.fence = owned.fence ?? readFence(owned.account);
    const replacement = replacementPending(owned);
    const id = publicUuid(sessionId); if (owned.sessionId && owned.sessionId !== id || owned.fence && owned.fence.sessionId !== id || replacement && replacement.sessionId !== id) fail('The owned upload session changed', 409);
    // A read may overlap an explicit cancel and a new begin. Its captured
    // session must still own this editor before any delayed snapshot is adopted.
    let epoch = owned.sessionEpoch;
    const response = await raw(owned, `/nostr/uploads/${id}`, 'GET'); assertSessionEpoch(owned, epoch); if (response.status !== 200) throw resumeError(response.status, response.body);
    const session = normalizeSession(response.body, id); selectSession(owned, id); epoch = owned.sessionEpoch; owned.latest = session;
    if (session.is_committed && !owned.fence) fence(owned, { sessionId: id, state: 'accepted-unavailable' });
    for (const groupId of session.public_upload.groupIds) if (!owned.groups.has(groupId)) {
      assertSessionEpoch(owned, epoch); await refresh(owned.account); assertSessionEpoch(owned, epoch);
      const groupResponse = await deps.catalogueRead('group.get', { id: groupId }); assertSessionEpoch(owned, epoch); await refresh(owned.account); assertSessionEpoch(owned, epoch);
      if (groupResponse.status !== 200) throw resumeError(groupResponse.status, groupResponse.body);
      const group = record(groupResponse.body, 'actual upload group'), actual = resource(record(group.public_api, 'actual group provenance').resource, 'scanlation_group');
      if (publicUuid(group.id) !== groupId || publicUuid(actual.id) !== groupId || text(group.name, 'group name') !== actual.attributes.name) fail('Upload group presentation changed its exact UUID or name');
      owned.groups.set(groupId, groupResponse.body as ScanlationGroup);
    }
    assertSessionEpoch(owned, epoch);
    session.relationships.groups = session.public_upload.groupIds.map(groupId => owned.groups.get(groupId)!);
    const pending = owned.fence ?? readFence(owned.account); if (pending) { owned.fence = pending; session.can_mutate = false; session.can_cancel = false; session.can_retry_commit = false; session.public_upload.commitState = pending.state; }
    if (replacementPending(owned)) freezeReplacement(owned, session);
    return session;
  }
  async function discover(owned: Owned): Promise<Response<PublicSession>> {
    owned.fence = owned.fence ?? readFence(owned.account);
    // A remembered exact session is never replaced by the ambiguous current-session route.
    const replacement = replacementPending(owned);
    if (owned.fence || replacement || owned.sessionId) return { status: 200, body: await exact(owned, (owned.fence?.sessionId ?? replacement?.sessionId ?? owned.sessionId)!) };
    const epoch = owned.sessionEpoch;
    const response = await raw(owned, '/upload', 'GET'); assertSessionEpoch(owned, epoch); if (response.status !== 200) return response as Response<PublicSession>;
    const id = basicSession(response.body); selectSession(owned, id); return { ...response, body: await exact(owned, id) };
  }
  async function scheduleFingerprint(session: PublicSession): Promise<string> {
    return deps.digest(encoder.encode(JSON.stringify({ mangaId: session.manga_id, chapterId: session.chapter_id ?? null, groupIds: session.public_upload.groupIds, draft: signature(snapshot(session.draft ?? {}, [])), pages: orderedFacts(session) })));
  }
  async function confirmSchedule(owned: Owned, pending: Fence, expected?: ScheduleReceipt): Promise<PublicSession | { sessionId: string; scheduled: ScheduleReceipt }> {
    if (!pending.schedule || pending.schedule.accountId !== owned.account.actor) fail('No captured scheduling intent matches this owner', 409);
    // This account-fenced read precedes the exact staging read. Neither absence
    // from the active list nor an ordinary 202 is a schedule acceptance receipt.
    const saved = await owned.schedules!.manager.get(pending.sessionId); owned.assert();
    if (expected && saved.receipt.chapterId !== expected.chapterId || saved.receipt.publishAt && saved.receipt.publishAt !== pending.schedule.publishAt) fail('Scheduling acceptance changed its immutable identity', 409);
    if (['complete', 'cancelled'].includes(saved.receipt.state)) {
      // Terminal owner/session facts resolve dispatch uncertainty even after
      // normal cleanup deletes staging rows. This is NOT hash revalidation or
      // reader success. Remembered chapter/time identity was checked by get().
      if (JSON.stringify(readFence(owned.account)) !== JSON.stringify(pending)) fail('Terminal scheduling recovery changed in another view', 409);
      const terminal = { ...saved.receipt, publishAt: saved.receipt.publishAt ?? pending.schedule.publishAt };
      await owned.schedules!.remember(pending.sessionId, terminal); owned.assert();
      if (JSON.stringify(readFence(owned.account)) !== JSON.stringify(pending)) fail('Terminal scheduling recovery changed in another view', 409);
      await changeScheduledFence(owned, pending); selectSession(owned, undefined); owned.latest = undefined; return { sessionId: pending.sessionId, scheduled: terminal };
    }
    const session = await exact(owned, pending.sessionId); owned.assert();
    if (!session.schedule || session.schedule.chapterId !== saved.receipt.chapterId || await scheduleFingerprint(session) !== pending.schedule.fingerprint) fail('Scheduling acceptance does not match the exact staged metadata and original pages', 409);
    owned.assert(); if (JSON.stringify(readFence(owned.account)) !== JSON.stringify(pending)) fail('Scheduling recovery changed in another view', 409);
    await owned.schedules!.remember(pending.sessionId, saved.receipt); owned.assert();
    await changeScheduledFence(owned, pending); selectSession(owned, undefined); owned.latest = undefined;
    session.schedule = saved.receipt; return session;
  }
  async function presentation(owned: Owned, input: unknown, mangaId: string): Promise<{ body?: Chapter; publicationStatus?: string; receipt: unknown }> {
    const receipt = resource(input, 'chapter'), id = publicUuid(receipt.id); if (!receipt.relationships.some(relation => relation.type === 'manga' && publicUuid(relation.id) === mangaId)) fail('Commit receipt changed its manga');
    const nostr = receipt.attributes.nostr === undefined ? undefined : record(receipt.attributes.nostr, 'publication receipt');
    const publicationStatus = nostr?.publicationStatus === undefined ? undefined : text(nostr.publicationStatus, 'publication receipt status');
    if (receipt.attributes.isUnavailable === true || ['accepted_pending_projection', 'pending_projection'].includes(publicationStatus ?? '')) return { receipt, publicationStatus };
    const response = await deps.catalogueRead('chapter.get', { id }); owned.assert(); await refresh(owned.account); owned.assert();
    const row = record(response.body, 'actual chapter presentation'), relationships = record(row.relationships, 'actual chapter relationships'), manga = record(relationships.manga, 'actual chapter manga');
    const actual = resource(record(row.public_api, 'actual chapter provenance').resource, 'chapter');
    if (response.status !== 200 || publicUuid(row.id) !== id || publicUuid(actual.id) !== id || integer(actual.attributes.version, 1) < integer(receipt.attributes.version, 1) || integer(row.version, 1) !== actual.attributes.version || publicUuid(manga.id) !== mangaId || !actual.relationships.some(relation => relation.type === 'manga' && publicUuid(relation.id) === mangaId) || row.is_unavailable === true || row.external_url || !Array.isArray(row.data) || !row.data.length || row.data.some(value => { const page = record(value, 'reader page'); return typeof page.public_url !== 'string' || !Array.isArray(page.dimensions) || page.dimensions.length !== 2 || page.dimensions.some(dimension => !Number.isSafeInteger(dimension) || Number(dimension) <= 0); })) fail('Accepted chapter presentation is not yet available');
    return { receipt, publicationStatus, body: response.body as Chapter };
  }
  async function loadUploadAccount(assertCurrent=()=>{}):Promise<User>{
    assertCurrent();const nativeAccount=deps.nativeAccount();const access=await createNativeContributionScope(nativeAccount,()=>{assertCurrent();if(deps.nativeAccount()!==nativeAccount)fail('The browser account changed',409);},{...deps.nativeDependencies,origin:deps.publicOrigin});
    const captured:Scope={origin:access.origin,catalogNamespace:access.catalogNamespace};
    const account:Account={user:undefined as unknown as User,actor:access.account.id,scope:captured,access,facts:undefined as unknown as UploadAccountFacts,assertCurrent,key:access.key('upload-recovery')+':'+captured.catalogNamespace};
    const result=access.contextBody,user=record(result.user),upload=record(result.upload);
    if(upload.conditional!==true){access.dispose();fail('Upload context lacks current conditional permissions');}
    const permissions = strings(user.permissions), roles = strings(user.roles), publication = record(upload.publication), limits = record(upload.limits);
    const canBegin = boolean(upload.canBegin), canManage = boolean(upload.canManage); if (canBegin !== permissions.includes('chapter.create') || canManage !== permissions.includes('self.chapter.update') || publication.requiresOperatorOrAuthorizedGroup !== true) fail('Upload eligibility was not backed by actual native grants');
    const view: User = { id: publicUuid(user.id), name: text(user.username, 'bound username'), version: integer(user.version, 1), roles, permissions };
    account.actor = view.id; account.user = view; account.facts = { canBegin, canManage, permissions, publication, limits }; accounts.set(view, account); return view;
  }
  function publicUploadOwner(target: () => string, user: User, targetKind: 'manga' | 'chapter' = 'manga', lifecycle: 'component' | 'caller' = 'component'): PublicUploadOwner {
    const account = accounts.get(user); if (!account) fail('Load your current browser upload account first', 409);
    const id = publicUuid(target()); let disposed = false;
    const assert = () => { if (disposed || publicUuid(target()) !== id || user.id !== account.actor) fail('The upload editor target changed. No request was replayed.', 409); account.access.check(); account.assertCurrent(); if (canonicalOrigin(deps.publicOrigin()) !== account.scope.origin) fail('Public upload origin changed', 409); };
    const initialFence = readFence(account), initialReplacement = readReplacement(account), initialBegin = readChapterBegin(account);
    const owned: Owned = { account, assert, targetId: id, targetKind, groups: new Map(), request: transport(account, assert), sessionId: initialFence?.sessionId ?? initialReplacement?.sessionId ?? initialBegin?.sessionId, sessionEpoch: 0, fence: initialFence, replacement: initialReplacement };
    let chapterSnapshot:{version:number;parentId:string}|undefined;
    const chapterFence=createChapterCommandFence(deps.chapterFences,{origin:account.scope.origin,catalogNamespace:account.scope.catalogNamespace,nativeAccount:account.access.nativeAccount,accountId:account.actor,chapterId:id},deps.digest,assert);
    if (targetKind === 'chapter') owned.chapterPending = () => chapterFence.pending();
    const captureChapter=(chapter:Resource)=>{if(publicUuid(chapter.id)!==id)fail('Chapter receipt changed its exact target');const parents=chapter.relationships.filter(row=>row.type==='manga');if(parents.length!==1)fail('Chapter parent is unavailable or ambiguous');return{version:integer(chapter.attributes.version,1),parentId:publicUuid(parents[0].id)};};
    owned.schedules = createUploadSchedules({ key: account.key.replace('account-upload-recovery.v1:', 'account-upload-schedules.v1:'), accountId: account.actor, assert, store: deps.scheduleFences, lock: deps.scheduleLock, attempt: deps.newAttempt,
      request: async (path, method, guard) => { assert(); guard(); const request = transport(account, () => { assert(); guard(); }); const response = await request({ url: account.scope.origin + '/api/v1' + path, method }); assert(); guard(); return response; },
      present: async receipt => { const parents = receipt.relationships.filter(row => row.type === 'manga'); if (parents.length !== 1) fail('Scheduled reader parent is unavailable'); return (await presentation(owned, receipt, publicUuid(parents[0].id))).body; },
    });
    const owner: PublicUploadOwner = { id, actor: account.actor, schedulingAvailable:account.facts.publication.configured===true, schedules: owned.schedules.manager, assert, alive: () => { try { assert(); return true; } catch { return false; } }, dispose: () => { disposed = true; account.access.dispose(); }, commitState: () => (owned.fence ?? readFence(account))?.state, pendingCommitSession: () => (owned.fence ?? readFence(account))?.sessionId, pendingReplacement: () => { const value = replacementPending(owned); return value ? Object.freeze({ state: value.state, sessionId: value.sessionId, fileId: value.fileId, expectedVersion: value.expectedVersion }) : undefined; }, chapterPagesPending: () => !!owned.sessionId || !!readChapterBegin(account) || !!readFence(account) || !!readReplacement(account), chapterBeginPending: () => !!readChapterBegin(account), reconcileChapterBegin: async () => {
      assert(); const pending = readChapterBegin(account);
      if (!pending || targetKind !== 'chapter' || pending.chapterId !== id) fail('No page-edit admission belongs to this chapter', 409);
      // Current ownership/target/version cannot identify an uncertain admission:
      // another tab may have admitted a different mode for that same chapter.
      // Only an exact accepted receipt witnessed before the snapshot failed can
      // bind recovery to this attempt. Read-only inspection never clears it.
      if (!pending.sessionId) fail('Page-edit admission is still unconfirmed: no exact accepted session receipt was received. Current staging can be inspected read-only, but cannot prove this attempt or its mode.', 409);
      const response = await discover(owned), session = response.body;
      assert();
      if (response.status !== 200 || !session || session.chapter_id !== id || session.manga_id !== pending.parentId || session.edit_version !== pending.version || session.id !== pending.sessionId) fail('Page-edit admission is still unconfirmed. Staging was kept; no begin request was replayed.', 409);
      writeChapterBegin(account, undefined, pending); return session;
    }, chapterPending:()=>chapterFence.pending(), chapterPendingPhase:()=>chapterFence.phase(), chapterAccess: async (version:number):Promise<ChapterEditAccess> => {
      assert();if(targetKind!=='chapter')fail('Chapter affordances require an exact chapter owner',422);
      const freshUser=await loadUploadAccount(assert),fresh=accounts.get(freshUser)!;assert();
      if(fresh.actor!==account.actor||fresh.access.nativeAccount!==account.access.nativeAccount||!equalScope(fresh.scope,account.scope))fail('The bound chapter account or service changed. Reopen the editor.',409);
      account.facts=fresh.facts;user.permissions=[...freshUser.permissions??[]];user.roles=[...freshUser.roles??[]];
      const response=await raw(owned,`/chapter/${id}`,'GET');assert();
      if(response.status!==200)fail(`Chapter eligibility read refused (HTTP ${response.status})`,response.status);
      const envelope=record(response.body),chapter=resource(envelope.data,'chapter');
      if(envelope.result!=='ok'||envelope.response!=='entity'||publicUuid(chapter.id)!==id||chapter.attributes.version!==integer(version,1))fail('Chapter eligibility changed. Reload the current chapter.',409);
      const groups:Resource[]=[];
      const groupIds=[...new Set(chapter.relationships.filter(row=>row.type==='scanlation_group').map(row=>publicUuid(row.id)))];
      if(groupIds.length>10)fail('Chapter group eligibility exceeds its bounded budget',422);
      for(const groupId of groupIds){
        const groupResponse=await raw(owned,`/group/${groupId}?includes%5B%5D=leader&includes%5B%5D=member`,'GET');assert();
        if(groupResponse.status!==200)continue;
        const result=record(groupResponse.body),group=resource(result.data,'scanlation_group');
        if(result.result!=='ok'||result.response!=='entity'||publicUuid(group.id)!==groupId)fail('Chapter group eligibility changed its exact target',409);
        groups.push(group);
      }
      await refresh(account);assert();chapterSnapshot=captureChapter(chapter);
      return chapterEditAffordances(chapter,{id:account.actor,permissions:account.facts.permissions,roles:user.roles??[]},groups);
    }, request: async <T>(route: string, method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE', payload?: unknown): Promise<Response<T>> => {
      assert();
      if (route.startsWith('/chapter/')) {
        assertMutable(owned);if(chapterFence.pending())fail(chapterUnconfirmed,409);
        if (owned.sessionId) fail('Finish or explicitly cancel the owned page-edit session before changing chapter metadata or ownership.',409);
        if (targetKind !== 'chapter' || ![ `/chapter/${id}`, `/chapter/${id}/claim` ].includes(route) || !['PUT', 'POST', 'DELETE'].includes(method) || route.endsWith('/claim') !== (method === 'POST')) fail('Chapter command escaped its exact signed target', 422);
        const grants = account.facts.permissions, staff = (user.roles ?? []).some(role => ['moderator', 'staff', 'sysop'].includes(role));
        const permission = method === 'DELETE' ? 'remove' : 'update';
        if (!grants.includes(`self.chapter.${permission}`) && !(method !== 'POST' && staff && grants.includes(`chapter.${permission}`))) fail('This bound account has no chapter mutation grant', 403);
        let body: unknown, removalMode: 'request-review' | 'approve-hide' | undefined;
        if (method === 'PUT') {
          const input = record(payload); if (Object.keys(input).some(key => !['version', 'title', 'volume', 'chapter', 'language', 'groups'].includes(key))) fail('Only chapter metadata can be edited', 422);
          const output: Record<string, unknown> = { version: integer(input.version, 1), expectedAccountId: account.actor };
          for (const key of ['title', 'volume', 'chapter', 'language']) if (input[key] !== undefined) { if (typeof input[key] !== 'string') fail('Invalid chapter metadata', 422); output[key === 'language' ? 'translatedLanguage' : key] = key === 'language' ? publicWriteLanguage(input[key]) : input[key]; }
          if (input.groups !== undefined) output.groups = ids(input.groups, 10);
          body = output;
        } else if (method === 'DELETE') {
          const input = record(payload);
          if (Object.keys(input).some(key => key !== 'removalMode') || input.removalMode !== 'request-review' && input.removalMode !== 'approve-hide') fail('Confirm the exact chapter removal effect first', 422);
          removalMode = input.removalMode as 'request-review' | 'approve-hide';
          if (removalMode === 'approve-hide' && (!staff || !grants.includes('chapter.remove'))) fail('This captured account cannot approve an instance hide', 403);
        } else if (payload !== undefined) fail('Chapter claim accepts no body', 422);
        const signedRoute = method === 'PUT' ? route : route + '?expectedAccountId=' + account.actor + (removalMode ? '&removalMode=' + removalMode : '');
        if(!chapterSnapshot){const read=await raw(owned,`/chapter/${id}`,'GET');const envelope=record(read.body);if(read.status!==200||envelope.result!=='ok'||envelope.response!=='entity')fail('Read the exact chapter before editing');chapterSnapshot=captureChapter(resource(envelope.data,'chapter'));}
        const capturedChapter={...chapterSnapshot};
        if(method==='PUT'&&record(body).version!==capturedChapter.version)fail('Chapter version changed before dispatch',409);
        assert();assertMutable(owned);
        const marker=await chapterFence.install({action:method==='PUT'?'update':method==='POST'?'claim':'remove',parentId:capturedChapter.parentId,expectedVersion:capturedChapter.version,removalMode:removalMode??null},{route:signedRoute,method,body:body??null});
        try{
        const response = await raw(owned, signedRoute, method, body);
        if(response.status>=400&&response.status<500){
          const error=record(response.body);if(response.bodyAvailable!==true||error.result!=='error'||!Array.isArray(error.errors)||!error.errors.length||!error.errors.every(value=>{const row=record(value);return row.status===response.status&&typeof row.title==='string'&&typeof row.detail==='string';}))fail('Chapter refusal was not a complete typed response');
          chapterFence.clear(marker);return callerView<T>(response);
        }
        if(response.bodyAvailable!==true)fail('Chapter command response body is unavailable');
        if (method !== 'DELETE' && response.status === 200) {
          const envelope = record(response.body), chapter = resource(envelope.data, 'chapter');
          if (envelope.result !== 'ok' || envelope.response !== 'entity' || publicUuid(chapter.id) !== id) fail('Chapter mutation returned a different target');
          const next=captureChapter(chapter);if(next.parentId!==capturedChapter.parentId)fail('Chapter receipt changed its parent');
          if(method==='PUT'){
            const expected=record(body);if(next.version!==capturedChapter.version+1)fail('Chapter update receipt changed its expected version');
            for(const key of ['title','volume','chapter','translatedLanguage'])if(expected[key]!==undefined&&chapter.attributes[key]!==String(expected[key]).replace(/^\p{White_Space}+|\p{White_Space}+$/gu,''))fail('Chapter update receipt contradicted the requested metadata');
            if(expected.groups!==undefined){const actual=chapter.relationships.filter(row=>row.type==='scanlation_group').map(row=>publicUuid(row.id)).sort();if(JSON.stringify(actual)!==JSON.stringify([...(expected.groups as string[])].sort()))fail('Chapter update receipt changed its requested groups');}
          }else{const uploaders=chapter.relationships.filter(row=>row.type==='user');if(next.version<=capturedChapter.version||chapter.attributes.uploader!==account.actor||uploaders.length!==1||uploaders[0].id!==account.actor)fail('Chapter claim receipt did not prove the captured account ownership');}
          chapterSnapshot=next;
        }
        if (method === 'DELETE' && response.status === 202) {
          if (removalMode !== 'request-review') fail('Chapter removal effect was not confirmed');
          const pending = record(response.body);
          if(Object.keys(pending).sort().join(',')!=='instanceRemoval,nostr,requestId,result,sourceDeletionRequestId,sourceDeletionStatus,status')fail('Chapter removal receipt changed its schema');
          if (pending.result !== 'ok' || pending.status !== 'pending-moderation' || pending.instanceRemoval !== 'pending-moderation' || response.headers['x-weebdex-instance-removal'] !== 'pending-moderation') fail('Chapter removal was not durably queued');
          publicUuid(pending.requestId);
          const sourceStatus = pending.sourceDeletionStatus;
          if (!['not-requested'].includes(String(sourceStatus)) || response.headers['x-weebdex-source-deletion-status'] !== sourceStatus || (sourceStatus === 'moderator-review-requested') !== (pending.sourceDeletionRequestId !== null)) fail('Invalid source deletion status');
          if((response.headers['x-weebdex-source-deletion-request']??null)!==pending.sourceDeletionRequestId)fail('Source deletion receipt header contradicted its body');
          if (pending.sourceDeletionRequestId !== null && (typeof pending.sourceDeletionRequestId !== 'string' || !/^[0-9a-f]{64}$/.test(pending.sourceDeletionRequestId))) fail('Invalid source deletion receipt');
        }
        if (method === 'DELETE' && response.status === 204 && (response.body!==undefined || response.headers['x-weebdex-source-deletion-request']!=null || removalMode !== 'approve-hide' || !staff || !grants.includes('chapter.remove') || response.headers['x-weebdex-instance-removal'] !== 'soft-hidden' || !['not-requested'].includes(response.headers['x-weebdex-source-deletion-status'] ?? ''))) fail('Instance chapter removal was not confirmed');
        if(method==='DELETE'?response.status!==202&&response.status!==204:response.status!==200)fail('Chapter command outcome is unconfirmed');
        if(method==='DELETE'&&response.status===202&&record(response.body).sourceDeletionStatus==='unconfirmed'){chapterFence.sourceUnconfirmed(marker);return callerView<T>(response);}
        chapterFence.clear(marker);return callerView<T>(response);
        }catch(error){if(error instanceof PublicUploadRequestError&&error.outcome==='unsent')chapterFence.clear(marker);throw error;}
      }
      if (!route.startsWith('/upload') || /[?#\\]/.test(route)) fail('Unsupported signed upload route', 422);
      if (route === '/upload' && method === 'GET') return callerView<T>(await discover(owned));
      assertMutable(owned);
      if (route === '/upload/check-approval-required' && method === 'POST') { const value = record(payload); const locale = publicWriteLanguage(value.language ?? value.locale); const manga = owned.latest?.manga_id ?? id; const response = await raw(owned, route, method, { manga, locale }); if (response.status === 200 && (record(response.body).result !== 'ok' || typeof record(response.body).requiresApproval !== 'boolean')) fail('Invalid actual approval decision'); return callerView<T>(response); }
      if ((route === '/upload/begin' || route.startsWith('/upload/begin/')) && method === 'POST') {
        const value = record(payload); let body: unknown;
        if (route === '/upload/begin') { if (targetKind !== 'manga' || !account.facts.canBegin || !account.facts.canManage || publicUuid(value.manga) !== id) fail('The bound native account cannot begin this target upload', 403); body = { manga: id, groups: ids(value.groups, 10), expectedAccountId: account.actor }; }
        else {
          if (targetKind !== 'chapter' || publicUuid(route.slice('/upload/begin/'.length)) !== id || Object.keys(value).some(key => !['version','replace','expectedAccountId'].includes(key)) || value.replace !== undefined && typeof value.replace !== 'boolean') fail('Invalid exact page-edit admission', 422);
          const version = integer(value.version, 1), replace = value.replace === true;
          const access = await owner.chapterAccess(version); assert();
          if (!access.canEditPages || !account.facts.canManage) fail(access.pageSource === 'protocol' ? 'Original signed protocol pages are immutable.' : 'Local page-edit eligibility could not be verified.',403);
          const existing = await discover(owned); assert();
          if (existing.status !== 404 || owned.sessionId) fail('An owned upload may already exist. Resume its original editor or explicitly cancel it first.',409);
          assertMutable(owned);
          let marker: ChapterBeginFence = { chapterId:id,parentId:chapterSnapshot!.parentId,version,replace };
          writeChapterBegin(account, marker);
          let received = false;
          try {
            const guard = () => { assert(); if (chapterFence.pending() || readFence(account) || readReplacement(account) || JSON.stringify(readChapterBegin(account)) !== JSON.stringify(marker)) fail('Page-edit admission ownership changed. Recovery was kept.',409); };
            const request = transport(account, guard);
            const response = await request({ url:account.scope.origin+'/api/v1'+route,method:'POST',body:encoder.encode(JSON.stringify({version,replace,expectedAccountId:account.actor})),contentType:'application/json' }); received = true; guard();
            if (response.status >= 400 && response.status < 500 && response.bodyAvailable === true && record(response.body).result === 'error' && Array.isArray(record(response.body).errors)) { writeChapterBegin(account,undefined,marker); return callerView<T>(response); }
            if (response.status !== 200 || response.bodyAvailable !== true) fail('Page-edit admission is unconfirmed',409);
            const sessionId = basicSession(response.body), edit = record(record(record(response.body).attributes).edit);
            if (publicUuid(edit.chapterId) !== id || edit.version !== version || edit.replace !== replace) fail('Page-edit admission receipt changed its target, version or mode',409);
            const next = {...marker,sessionId}; writeChapterBegin(account,next,marker); marker=next; selectSession(owned,sessionId);
            const session = await exact(owned,sessionId); guard();
            if (session.chapter_id !== id || session.manga_id !== marker.parentId || session.edit_version !== version || session.is_committed || session.is_deleted || replace && session.page_order.length !== 0) fail('Page-edit staging differs from its signed admission receipt',409);
            writeChapterBegin(account,undefined,marker); return callerView<T>({...response,body:session});
          } catch (error) {
            // A failed later snapshot is not evidence that the admission POST was unsent.
            if (!received && error instanceof PublicUploadRequestError && error.outcome === 'unsent') writeChapterBegin(account,undefined,marker);
            throw error;
          }
        }
        if (owned.sessionId) fail('An owned upload already exists. Resume or explicitly cancel it first.', 409);
        const response = await raw(owned, route, method, body); if (response.status !== 200) return callerView<T>(response);
        const sessionId = basicSession(response.body); selectSession(owned, sessionId); return callerView<T>({ ...response, body: await exact(owned, sessionId) });
      }
      const match = /^\/upload\/([^/]+)(?:\/([^/]+))?$/.exec(route); if (!match) fail('Unsupported signed upload route', 422);
      const sessionId = publicUuid(match[1]), action = match[2]; if (owned.sessionId !== sessionId) fail('The upload request escaped its owned session', 409);
      if (action === 'commit' && method === 'POST') {
        const value = record(payload); if (value.termsAccepted !== true) fail('Explicit upload terms acceptance is required', 422);
        const candidate = { draft: draft(value.draft), page_order: ids(value.page_order) }; if (!candidate.page_order.length || !candidate.draft.language) fail('A language and staged page order are required', 422);
        const current = await exact(owned, sessionId); if (!current.can_mutate || !current.can_retry_commit || !sameDraft(current.draft, candidate.draft) || JSON.stringify(current.page_order) !== JSON.stringify(candidate.page_order)) fail('Commit must match the acknowledged private draft and exact page order', 409);
        if (targetKind === 'chapter' && (current.chapter_id !== id || current.edit_version === undefined || value.publishAt !== undefined)) fail('Local page commits require their exact edit binding and cannot be scheduled',409);
        if(value.publishAt!==undefined&&account.facts.publication.configured!==true)fail('This site does not support scheduled publishing. You can upload now instead.',409);
        const publishAt = value.publishAt === undefined ? undefined : scheduledUtc(value.publishAt);
        const schedule = publishAt === undefined ? undefined : { accountId: account.actor, publishAt, fingerprint: await scheduleFingerprint(current) }; owned.assert();
        const body = { chapterDraft: { ...(publishAt === undefined ? {} : { publishAt }), volume: candidate.draft.volume || null, chapter: candidate.draft.chapter || null, title: candidate.draft.title || null, translatedLanguage: publicWriteLanguage(candidate.draft.language) }, pageOrder: candidate.page_order, termsAccepted: true };
        // No await between the final shared-fence check and installing this
        // attempt. Only this private token may dispatch through its own fence.
        assertMutable(owned); const token = {}, attempt = deps.newAttempt(); if (!/^[A-Za-z0-9_-]{1,128}$/.test(attempt)) fail('Invalid upload attempt identity', 409);
        const original: Fence = { sessionId, state: 'uncertain', attempt, ...(schedule ? { schedule } : {}) };
        let expected: Fence | undefined = original, responseReceived = false;
        const install = () => { owned.assert(); assertMutable(owned); owned.commitAttempt = token; if (schedule) writeScheduledFenceLocked(owned, undefined, original); else fence(owned, original); };
        const change = async (next?: Fence, unsent = false) => { if (!expected) fail('This publication callback no longer owns a recovery record', 409); if (expected.schedule) await changeScheduledFence(owned, expected, next, unsent); else fence(owned, next, expected); expected = next; };
        if (schedule) await deps.scheduleLock(scheduleCommitKey(account), install); else install();
        try {
          const response = await raw(owned, route, 'POST', body, 'json', { token, sessionId, attempt }); responseReceived = true;
          if (response.status === 202 && schedule && record(response.body).nostr && record(record(response.body).nostr).scheduleAccepted === true) {
            const accepted = scheduleReceipt(response.body), pending = readFence(account)!;
            if (accepted.publishAt !== publishAt) fail('Accepted scheduling time changed');
            const saved = await confirmSchedule(owned, pending, accepted);
            return callerView<T>({ ...response, body: undefined, accepted: true, sessionId, scheduled: "scheduled" in saved ? saved.scheduled : saved.schedule });
          }
          if (response.status === 202) { await change({ ...expected!, sessionId, state: 'accepted-unavailable' }); return callerView<T>({ ...response, body: undefined, accepted: true, sessionId }); }
          if (response.status !== 200) { if (response.status >= 400 && response.status < 500 && record(response.body).result === 'error' && Array.isArray(record(response.body).errors)) { await change(); return callerView<T>(response); } throw resumeError(response.status, response.body); }
          const receipt = resource(response.body, 'chapter'); integer(receipt.attributes.version, 1);
          if (targetKind === 'chapter' && (publicUuid(receipt.id) !== id || receipt.attributes.version !== current.edit_version! + 1)) fail('Page-edit commit receipt changed the exact chapter or version',409);
          if (!receipt.relationships.some(relation => relation.type === 'manga' && publicUuid(relation.id) === current.manga_id)) fail('Accepted receipt changed its target');
          await change({ sessionId, state: 'accepted-unavailable', ...(schedule ? { schedule } : {}) });
          const result = await presentation(owned, response.body, current.manga_id);
          if (!result.body) await change({ sessionId, state: 'accepted-unavailable', ...(schedule ? { schedule } : {}) }); else { await change(); selectSession(owned, undefined); owned.latest = undefined; }
          return callerView<T>({ ...response, ...result, body: result.body, accepted: true, sessionId });
        } catch (error) {
          const persisted = readFence(account), owns = expected && persisted && canonicalRecord(expected) === canonicalRecord(persisted);
          if (owns && error && typeof error === 'object' && 'status' in error && error.status === 202) await change({ ...expected!, state: 'accepted-unavailable' });
          else if (owns && !responseReceived && expected?.state === 'uncertain' && error && typeof error === 'object' && 'outcome' in error && error.outcome === 'unsent') await change(undefined, true);
          else if (persisted) owned.fence = persisted;
          throw error;
        }
      }
      if (method === 'PATCH' && action === undefined) { const value = record(payload); const current = await exact(owned, sessionId); if (!current.can_mutate || !account.facts.canManage) fail('Upload is read-only', 409); const body = { version: integer(value.version, 1), draft: draft(value.draft), pageOrder: ids(value.page_order ?? value.pageOrder) }; if(body.draft.language)publicWriteLanguage(body.draft.language); const response = await raw(owned, `/nostr/uploads/${sessionId}`, 'PATCH', body); return callerView<T>(response.status === 200 ? { ...response, body: (owned.latest = normalizeSession(response.body, sessionId)) } : response); }
      if (method === 'DELETE' && (action === undefined || action === 'batch' || /^[0-9a-f-]{36}$/i.test(action))) {
        const current = await exact(owned, sessionId);
        if (!action && current.is_deleted) { selectSession(owned,undefined); owned.latest=undefined; return callerView<T>({status:200,body:{result:'ok'}}); }
        if (!(action ? current.can_mutate : current.can_cancel) || !account.facts.canManage) fail('Upload is read-only', 409);
        const known = new Set(current.page_order); let body: string[] | undefined; if (action === 'batch') body = ids(payload); else if (action) body = [publicUuid(action)]; if (body && (!body.length || body.some(file => !known.has(file)))) fail('Deletion must name exact current staging files', 409);
        const response = await raw(owned, route, 'DELETE', action === 'batch' ? body : undefined); if (response.status === 200) { if (record(response.body).result !== 'ok') fail('Deletion was not confirmed'); if (!action) { selectSession(owned, undefined); owned.latest = undefined; } } return callerView<T>(response);
      }
      fail('Unsupported public upload operation. No native mutation was sent.', 422);
    } };
    owners.set(owner, owned);
    // Async account setup must use the caller's already-registered onDestroy.
    // Ordinary synchronously-created upload components retain their own hook.
    if (lifecycle === 'component') deps.destroy(owner.dispose); return owner;
  }
  async function currentUpload(owner?: PublicUploadOwner): Promise<PublicSession | undefined> { const owned = ownedBy(owner), response = await discover(owned); if (response.status === 404 && !owned.sessionId && !owned.fence) return; if (response.status !== 200 || !response.body) throw resumeError(response.status, response.body); return response.body; }
  async function reconcileUploadReceipt(owner: PublicUploadOwner, sessionId: string): Promise<PublicSession | { sessionId: string; scheduled: ScheduleReceipt }> {
    const owned = ownedBy(owner), id = publicUuid(sessionId), pending = owned.fence ?? readFence(owned.account); if (!pending || pending.sessionId !== id) fail('No exact unresolved publication matches this recovery request', 409);
    if (pending.schedule) {
      let receipt; try { receipt = await owned.schedules!.manager.get(id); } catch (error) { if (!(error instanceof PublicApiContractError) || error.status !== 404) throw error; }
      if (receipt) return confirmSchedule(owned, pending, receipt.receipt);
      // A past timestamp may have used the unchanged immediate publication path.
      // Only its ordinary exact committed receipt can resolve that dispatch.
    }
    const session = await exact(owned, id); if (!session.is_committed || !session.public_upload.receipt) throw new VisibleError('Publication is still unresolved', unknownCommit);
    const shown = await presentation(owned, session.public_upload.receipt, session.manga_id); if (!shown.body) { const next: Fence = { ...pending, sessionId: id, state: 'accepted-unavailable' }; if (pending.schedule) await changeScheduledFence(owned, pending, next); else fence(owned, next, pending); return session; }
    session.receipt = shown.body; session.committed_result = shown.body; session.can_mutate = false; session.can_cancel = false; session.can_retry_commit = false; if (pending.schedule) await changeScheduledFence(owned, pending); else fence(owned, undefined, pending); selectSession(owned, undefined); return session;
  }
  async function saveResume(session: UploadSession, value: UploadSnapshot, owner?: PublicUploadOwner): Promise<PublicSession> {
    const owned = ownedBy(owner); assertMutable(owned); const previous = session as PublicSession, fresh = await exact(owned, publicUuid(session.id));
    if (!fresh.can_mutate || !sameDraft(fresh.draft, previous.draft) && !sameDraft(fresh.draft, value.draft)) throw resumeError(409);
    const known = new Set(previous.page_order ?? []), prior = (previous.page_order ?? []).filter(id => fresh.page_order.includes(id)), remote = fresh.page_order.filter(id => known.has(id));
    if (JSON.stringify(prior) !== JSON.stringify(remote) && JSON.stringify(fresh.page_order) !== JSON.stringify(value.page_order)) throw resumeError(409);
    if (value.page_order.length !== fresh.page_order.length || new Set(value.page_order).size !== value.page_order.length || value.page_order.some(id => !fresh.page_order.includes(publicUuid(id)))) throw resumeError(409);
    const response = await owner!.request<PublicSession>(`/upload/${session.id}`, 'PATCH', { version: fresh.version, ...value }); if (response.status !== 200 || !response.body) throw resumeError(response.status, response.body); return response.body;
  }
  async function cancelResume(session: UploadSession, owner?: PublicUploadOwner) { ownedBy(owner); const response = await owner!.request(`/upload/${publicUuid(session.id)}`, 'DELETE'); if (response.status !== 200) throw resumeError(response.status, response.body); }
  function matchPage(page: UploadSessionFile, fresh: PublicSession): PublicFile {
    const recovery = page.upload_recovery; if (!recovery || recovery.state !== 'unknown' || recovery.session !== fresh.id) throw new VisibleError('Page outcome is still unknown', unknownPage);
    const matches = (fresh.relationships.files as PublicFile[] ?? []).filter(file => !recovery.before_ids.includes(file.id) && file.file_hash === recovery.hash && file.original_file_name === page.original_file_name && Number(file.file_size) === page.file?.size && file.public_upload?.mimeType === page.file?.type);
    if (matches.length !== 1) throw new VisibleError('Page outcome is still unknown', unknownPage); return { ...matches[0], thumbnail: page.thumbnail };
  }
  async function resolveUploadPage(session: UploadSession, page: UploadSessionFile, owner: PublicUploadOwner): Promise<UploadSessionFile> { const owned = ownedBy(owner); if (page.upload_recovery?.state !== 'unknown') return page; return matchPage(page, await exact(owned, publicUuid(session.id))); }
  async function runUploadBatch(session: UploadSession, pages: UploadSessionFile[], owner: PublicUploadOwner, progress?: (delta: number) => void, assertCurrent = () => {}): Promise<UploadSessionFile[]> {
    const owned = ownedBy(owner), guard = () => { owner.assert(); assertCurrent(); assertMutable(owned); }; guard(); const batchRequest = transport(owned.account, guard), id = publicUuid(session.id); if (owned.sessionId !== id || pages.some(page => page.upload_recovery?.state === 'unknown')) throw new VisibleError('Page cannot be replayed', unknownPage);
    markUnsentUploadPages(pages, id);
    for (const page of pages) {
      if (!page.file) fail('An original local image is required', 422);
      let dispatched = false;
      try {
        guard(); const fresh = await exact(owned, id); guard(); if (!fresh.can_mutate || !owned.account.facts.canManage) fail('Current mutable owner staging was not confirmed', 409);
        const serialized = await deps.multipart(page.file); guard(); const original = await page.file.arrayBuffer(); guard(); const digest = await deps.digest(new Uint8Array(original)); guard();
        page.error = undefined; page.upload_recovery = { state: 'unknown', session: id, before_ids: [...fresh.page_order], hash: digest };
        dispatched = true; const response = await batchRequest({ url: owned.account.scope.origin + `/api/v1/upload/${id}`, method: 'POST', body: serialized.body, contentType: serialized.contentType }); guard();
        if (response.status !== 200) { if (response.status >= 400 && response.status < 500 && record(response.body).result === 'error' && Array.isArray(record(response.body).errors)) { page.upload_recovery.state = 'rejected'; page.error = actualError(response.body) ?? `Image refused (HTTP ${response.status})`; throw resumeError(response.status, response.body); } throw new VisibleError('Image upload is unconfirmed', unknownPage); }
        const envelope = record(response.body); if (!Array.isArray(envelope.data) || !Array.isArray(envelope.errors) || !['ok', 'error'].includes(String(envelope.result))) fail('Malformed image attachment receipt');
        if (envelope.data.length === 0 && envelope.errors.length === 1) { const rejection = record(envelope.errors[0]), context = record(rejection.context); if (envelope.result !== 'error' || integer(rejection.status) < 400 || context.originalFileName !== serialized.file.name) fail('Invalid per-image rejection'); page.upload_recovery.state = 'rejected'; page.error = text(rejection.detail, 'image rejection'); throw resumeError(400, response.body); }
        if (envelope.result !== 'ok' || envelope.data.length !== 1 || envelope.errors.length) fail('Ambiguous attachment receipt');
        const receipt = normalizeFile(envelope.data[0], id, false); if (receipt.file_hash !== digest || receipt.original_file_name !== serialized.file.name || Number(receipt.file_size) !== serialized.file.size || receipt.public_upload?.mimeType !== serialized.file.type || fresh.page_order.includes(receipt.id)) fail('Attachment receipt does not identify these exact original image bytes');
        const next = await exact(owned, id); guard(); const confirmed = matchPage(page, next); if (confirmed.id !== receipt.id) fail('Attachment response differs from owner snapshot');
        Object.assign(page, confirmed, { file: undefined, error: undefined, upload_recovery: undefined }); progress?.(serialized.file.size);
      } catch (error) {
        const unsent = !dispatched || error && typeof error === 'object' && 'outcome' in error && error.outcome === 'unsent';
        if (unsent) { page.upload_recovery = { state: 'unsent', session: id, before_ids: [] }; page.error = 'Not sent. Select a replacement or remove this local placeholder explicitly.'; }
        else if (page.upload_recovery?.state !== 'rejected') { if (owner.alive()) { try { guard(); const latest = await exact(owned, id); guard(); const confirmed = matchPage(page, latest); Object.assign(page, confirmed, { file: undefined, error: undefined, upload_recovery: undefined }); progress?.(Number(confirmed.file_size)); continue; } catch { /* Missing or ambiguous evidence never proves rejection. */ } } page.error = unknownPage; }
        throw error;
      }
    }
    return pages;
  }
  const orderedFacts = (session: PublicSession): ReplacementFile[] => session.page_order.map(id => { const page = session.relationships.files?.find(file => file.id === id); if (!page) fail('Replacement order lost a page', 409); return replacementFacts(page); });
  const sessionIdentity = (session: PublicSession) => JSON.stringify({ id: session.id, manga: session.manga_id, chapter: session.chapter_id, version: session.version, draft: signature(snapshot(session.draft ?? {}, [])), groups: session.public_upload.groupIds, order: session.page_order, files: orderedFacts(session) });
  const sameFacts = (a: ReplacementFile, b: ReplacementFile) => JSON.stringify(a) === JSON.stringify(b);
  async function replacementProof(owned: Owned, pending: ReplacementFence, session: PublicSession, successfulResponse = false) {
    const unchanged = sameFacts(pending.before.find(file => file.id === pending.fileId)!, pending.replacement), expectedVersion = pending.expectedVersion + (unchanged ? 0 : 1);
    // A no-op's old version cannot prove a lost request has finished. Only its
    // definitive original success response can authorize this unchanged case.
    if (unchanged && !successfulResponse) throw new VisibleError('Page replacement is still unresolved', unknownReplacement);
    if (session.id !== pending.sessionId || owned.targetId !== pending.targetId || owned.targetKind !== pending.targetKind || session.manga_id !== pending.mangaId || session.chapter_id !== pending.chapterId || session.is_committed || session.is_deleted || session.version !== expectedVersion || JSON.stringify(session.page_order) !== JSON.stringify(pending.order) || JSON.stringify(session.public_upload.groupIds) !== JSON.stringify(pending.groupIds)) throw new VisibleError('Page replacement is still unresolved', unknownReplacement);
    const current = orderedFacts(session), expected = pending.before.map(file => file.id === pending.fileId ? pending.replacement : file);
    if (current.some((file, index) => !sameFacts(file, expected[index])) || await deps.digest(encoder.encode(signature(snapshot(session.draft ?? {}, [])))) !== pending.draftHash) throw new VisibleError('Page replacement is still unresolved', unknownReplacement);
    owned.assert(); if (JSON.stringify(replacementPending(owned)) !== JSON.stringify(pending)) fail('Page replacement recovery changed during readback', 409);
    return session.relationships.files!.find(file => file.id === pending.fileId)!;
  }
  async function replaceUploadPage(session: UploadSession, target: UploadSessionFile, file: UploadImageFile, owner: PublicUploadOwner, assertCurrent = () => {}): Promise<UploadReplacementResult> {
    const owned = ownedBy(owner); assertMutable(owned);
    const previous = session as PublicSession, sessionId = publicUuid(previous.id), fileId = publicUuid(target.id), expectedVersion = integer(previous.version, 1), beforeIdentity = sessionIdentity(previous), beforeTarget = replacementFacts(target);
    const selected = owned.account.access;
    const originalFile = { name: file?.name, type: file?.type, size: file?.size, read: file?.arrayBuffer };
    let pending: ReplacementFence | undefined, token: object | undefined;
    const inputs = () => {
      owned.assert(); assertCurrent();
      const access = owned.account.access;
      if (!isRemoteUploadFile(target) || access !== selected || owned.sessionId !== sessionId || sessionIdentity(previous) !== beforeIdentity || !sameFacts(replacementFacts(target), beforeTarget) || file.name !== originalFile.name || file.type !== originalFile.type || file.size !== originalFile.size || file.arrayBuffer !== originalFile.read) fail('Page replacement context changed. No replacement was replayed.', 409);
    };
    const guard = () => {
      inputs();
      if (!pending) return assertMutable(owned);
      if (owned.fence || readFence(owned.account) || owned.replacementAttempt !== token || JSON.stringify(replacementPending(owned)) !== JSON.stringify(pending)) throw new VisibleError('Page replacement context changed', unknownReplacement);
    };
    guard(); if (!isRemoteUploadFile(target) || !previous.page_order.includes(fileId) || !sameFacts(orderedFacts(previous).find(page => page.id === fileId)!, beforeTarget)) fail('Replacement must name one exact current staged page', 409);
    // Validate and serialize once. The digest comes from the exact file slice in
    // that multipart body, never a second file read or rebuilt FormData payload.
    const serialized = await deps.replacementMultipart(file, expectedVersion); guard();
    const hash = await deps.digest(serialized.fileBytes); guard();
    if (serialized.file.name !== originalFile.name || serialized.file.type !== originalFile.type || serialized.file.size !== originalFile.size || !/^[0-9a-f]{64}$/.test(hash)) fail('Replacement bytes changed during validation', 409);
    const fresh = await exact(owned, sessionId); guard();
    if (!fresh.can_mutate || !owned.account.facts.canManage || sessionIdentity(fresh) !== beforeIdentity) fail('Replacement requires the exact current mutable session version and page set', 409);
    const draftHash = await deps.digest(encoder.encode(signature(snapshot(fresh.draft ?? {}, [])))); guard();
    await refresh(owned.account); guard();
    const attempt = deps.newAttempt(); if (!/^[A-Za-z0-9_-]{1,128}$/.test(attempt)) fail('Invalid replacement attempt identity', 409);
    const value: ReplacementFence = { state: 'uncertain', sessionId, fileId, expectedVersion, attempt, targetId: owned.targetId, targetKind: owned.targetKind, mangaId: fresh.manga_id, ...(fresh.chapter_id ? { chapterId: fresh.chapter_id } : {}), draftHash, groupIds: [...fresh.public_upload.groupIds], order: [...fresh.page_order], before: orderedFacts(fresh), replacement: { id: fileId, name: serialized.file.name, hash, size: serialized.file.size, mime: serialized.file.type } };
    token = {}; pending = value;
    let requested = false, originalUnsent = false;
    try {
      await installReplacement(owned, value, token, inputs); guard();
      const request = transport(owned.account, guard);
      let response: PublicUploadResponse;
      try { requested = true; response = await request({ url: owned.account.scope.origin + `/api/v1/nostr/uploads/${sessionId}/files/${fileId}/replace`, method: 'POST', body: serialized.body, contentType: serialized.contentType }); }
      catch (error) { originalUnsent = !!error && typeof error === 'object' && 'outcome' in error && error.outcome === 'unsent'; throw error; }
      guard();
      if (response.status !== 200) {
        const body = record(response.body);
        if (response.status >= 400 && response.status < 500 && body.result === 'error' && Array.isArray(body.errors) && body.errors.length && body.errors.every(value => { const error = record(value); return Number.isSafeInteger(error.status) && error.status === response.status && typeof error.title === 'string' && typeof error.detail === 'string'; })) { await clearReplacement(owned, pending, false, undefined, inputs); inputs(); throw resumeError(response.status, response.body); }
        throw new VisibleError('Page replacement was not confirmed', unknownReplacement);
      }
      const body = record(response.body), receipt = normalizeFile(body.file, sessionId, false), resultVersion = expectedVersion + (sameFacts(beforeTarget, pending.replacement) ? 0 : 1);
      if (body.result !== 'ok' || integer(body.sessionVersion, 1) !== resultVersion || !sameFacts(replacementFacts(receipt), pending.replacement)) fail('Replacement response did not identify the exact stable page and original bytes');
      const next = await exact(owned, sessionId); guard(); const confirmed = await replacementProof(owned, pending, next, true); guard(); await clearReplacement(owned, pending, false, next, inputs); inputs();
      return { session: next, file: confirmed };
    } catch (error) {
      // Only the original POST transport's affirmative unsent evidence unlocks
      // without a receipt. Later read/proof/caller errors are never such evidence.
      // An unchanged old snapshot cannot disprove a
      // delayed dispatched replacement, and nothing here replays its POST.
      if (pending && owned.replacement?.attempt === pending.attempt && (!requested || originalUnsent)) { await clearReplacement(owned, pending, true); inputs(); }
      throw error;
    }
  }
  async function reconcileUploadReplacement(owner: PublicUploadOwner): Promise<UploadReplacementResult> {
    const owned = ownedBy(owner), pending = replacementPending(owned);
    if (!pending || owned.fence || readFence(owned.account) || pending.targetId !== owned.targetId || pending.targetKind !== owned.targetKind) fail('No exact unresolved page replacement matches this editor', 409);
    const session = await exact(owned, pending.sessionId), file = await replacementProof(owned, pending, session);
    await clearReplacement(owned, pending, false, session); owned.assert(); assertMutable(owned); return { session, file };
  }
  async function uploadPreview(owner: PublicUploadOwner, session: UploadSession | string, input: UploadSessionFile, signal?: AbortSignal): Promise<Blob> {
    const owned = ownedBy(owner), sessionId = publicUuid(typeof session === 'string' ? session : session.id), fileId = publicUuid(input.id), fresh = await exact(owned, sessionId), file = (fresh.relationships.files as PublicFile[]).find(file => file.id === fileId);
    if (!file || file.file_hash !== input.file_hash || file.file_size !== input.file_size || !file.public_upload?.dimensions || !file.public_upload.previewUrl) fail('Preview no longer matches the exact staged original', 409);
    const previewGuard = () => { owned.assert(); if (replacementPending(owned)) throw new VisibleError('Reconcile page replacement first', unknownReplacement); const latest = owned.latest?.relationships.files?.find(page => page.id === fileId); if (!latest || latest.file_hash !== input.file_hash || latest.file_size !== input.file_size || latest.original_file_name !== input.original_file_name || latest.public_upload?.mimeType !== input.public_upload?.mimeType) fail('Preview page changed during loading', 409); };
    previewGuard();
    const facts = file.public_upload, response = await owned.request({ url: owned.account.scope.origin + facts.previewUrl, method: 'GET', responseType: 'image', signal, responseLimit: facts.fileSize }); previewGuard();
    const cache = new Set((response.headers['cache-control'] ?? '').toLowerCase().split(',').map(value => value.trim())), vary = new Set((response.headers.vary ?? '').toLowerCase().split(',').map(value => value.trim()));
    if (!cache.has('private') || !cache.has('no-store') || !vary.has('authorization') || response.headers['x-content-type-options']?.trim().toLowerCase() !== 'nosniff') fail('Private preview privacy headers were not confirmed');
    const mime = response.headers['content-type']?.split(';', 1)[0].trim().toLowerCase(); if (response.status !== 200 || !(response.body instanceof Uint8Array) || response.body.byteLength !== facts.fileSize || mime !== facts.mimeType || await deps.digest(response.body) !== file.file_hash) fail('Signed preview did not match the original image receipt');
    previewGuard(); const blob = new Blob([new Uint8Array(response.body)], { type: facts.mimeType }), dimensions = await deps.decodeImage(blob); previewGuard(); if (dimensions.width !== facts.dimensions!.width || dimensions.height !== facts.dimensions!.height) fail('Decoded preview dimensions differ from the original receipt'); return blob;
  }
  function uploadAccountFacts(user: User) { const account = accounts.get(user); if (!account) fail('No bound upload account context', 409); return structuredClone(account.facts); }
  return { loadUploadAccount, publicUploadOwner, uploadAccountFacts, currentUpload, saveResume, cancelResume, reconcileUploadReceipt, runUploadBatch, resolveUploadPage, replaceUploadPage, reconcileUploadReplacement, uploadPreview };
}

export function markUnsentUploadPages(pages: UploadSessionFile[], session: string) { for (const page of pages) if (page.file && page.upload_recovery?.state !== 'unknown') { page.error = 'Not sent because this batch has not reached this image. Remove or replace this local placeholder explicitly.'; page.upload_recovery = { state: 'unsent', session, before_ids: [] }; } }
export function isRemoteUploadFile(value: UploadSessionFile): boolean { try { publicUuid(value.id); return !value.file && !value.error && !value.upload_recovery && /^[0-9a-f]{64}$/.test(value.file_hash) && /^\d+$/.test(value.file_size) && Number.isSafeInteger(Number(value.file_size)) && Number(value.file_size) > 0; } catch { return false; } }
export function prepareResume(session: UploadSession): PublicSession { const value = session as PublicSession, order = new Map(value.page_order.map((id, index) => [id, index])); value.relationships.files = [...value.relationships.files ?? []].sort((a, b) => order.get(a.id)! - order.get(b.id)!); return value; }
export function acknowledge(session: UploadSession, saved: UploadSession) { const current = session as PublicSession, next = saved as PublicSession; for (const key of ['version', 'draft', 'page_order', 'can_mutate', 'can_cancel', 'can_retry_commit', 'publication_status', 'state', 'is_expired', 'expires_at', 'public_upload'] as const) Object.assign(current, { [key]: next[key] }); }
export function resumeNotice(session: UploadSession) { const value = session as PublicSession; if (value.public_upload?.commitState) return unknownCommit; if (value.is_committed) return 'The exact saved chapter receipt was restored. Reader navigation requires its current readable catalogue presentation.'; if (!value.can_mutate) return 'This saved upload is read-only with its current state or authorization. Its staged work has been kept.'; return 'Your original images, metadata and page order are staged privately on this instance.'; }
export function immutableCommit(session: UploadSession, value: UploadSnapshot): UploadSnapshot { const current = session as PublicSession; if (!current.can_mutate || !current.can_retry_commit || current.is_committed || current.public_upload?.commitState) throw new VisibleError('Reconcile publication first', resumeNotice(session)); return { draft: draft(value.draft), page_order: ids(value.page_order) }; }
const production = createNativeUploadAdapter();
export const { loadUploadAccount, publicUploadOwner, uploadAccountFacts, currentUpload, saveResume, cancelResume, reconcileUploadReceipt, runUploadBatch, resolveUploadPage, replaceUploadPage, reconcileUploadReplacement, uploadPreview } = production;
