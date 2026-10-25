// Explicit public UUID/NIP-98 upload adapter. The native cookie API is never a fallback.
import { onDestroy } from 'svelte';
import { Error as VisibleError } from '$lib/constants/Error';
import { capabilities, selectedSigner } from '../local-library/publication';
import { proveService, origin as canonicalOrigin } from '../local-library/protocol/nostr.js';
import { createNostrUploadRequest, type UploadRequestContext, type UploadRequestIdentity, type PublicUploadRequest, type PublicUploadResponse } from './nostr-upload-request';
import { serializeUploadMultipart } from './upload-multipart';
import { publicCatalogueRead } from './site-client';
import { publicUuid, record, resource, PublicApiContractError } from './validation';
export { getUploadOwner, setUploadOwner } from '../frontend-behavior/upload-batch';

type Policy = Awaited<ReturnType<typeof capabilities>>;
type Scope = { origin: string; serviceKey: string; catalogNamespace: string; proofOrigin: string };
type Response<T = unknown> = { status: number; body?: T; headers?: Record<string, string | null>; accepted?: true; sessionId?: string; publicationStatus?: string; receipt?: unknown };
export type UploadSnapshot = { draft: ChapterDraft; page_order: string[] };
export type UploadAccountFacts = { canBegin: boolean; canManage: boolean; publication: Record<string, unknown>; limits: Record<string, unknown>; permissions: string[] };
type FileFacts = { mimeType: string; fileSize: number; dimensions?: { width: number; height: number }; previewUrl?: string };
type PublicFile = UploadSessionFile & { public_upload?: FileFacts };
type PublicSession = UploadSession & { manga_id: string; chapter_id?: string; draft?: ChapterDraft | null; page_order: string[]; can_mutate: boolean; can_cancel: boolean; can_retry_commit: boolean; is_expired: boolean; expires_at: string; state: string; publication_status?: string; target_status: string; receipt?: Chapter; committed_result?: Chapter; public_upload: { groupIds: string[]; receipt: unknown; receiptStatus: 'none' | 'available' | 'unavailable'; commitState?: string } };
type Fence = { sessionId: string; state: 'uncertain' | 'accepted-unavailable' };
export interface UploadFenceStore { get(key: string): string | null; set(key: string, value: string): void; delete(key: string): void }
export interface PublicUploadOwner {
  readonly id: string; readonly actor: string;
  assert(): void; alive(): boolean; dispose(): void;
  commitState(): Fence['state'] | undefined;
  pendingCommitSession(): string | undefined;
  request<T = unknown>(route: string, method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE', payload?: unknown): Promise<Response<T>>;
}
export interface PublicUploadDependencies {
  capabilities(): Promise<Policy>;
  proveService(origin: string, key: string): Promise<{ origin: string; serviceKey: string }>;
  selectedSigner(): UploadRequestIdentity;
  publicOrigin(): string;
  createRequest(context: UploadRequestContext): (request: PublicUploadRequest) => Promise<PublicUploadResponse>;
  catalogueRead(operation: string, options: { id: string }): Promise<Response>;
  multipart: typeof serializeUploadMultipart;
  digest(bytes: Uint8Array): Promise<string>;
  decodeImage(blob: Blob): Promise<{ width: number; height: number }>;
  destroy(callback: () => void): void;
  fences: UploadFenceStore;
}
const defaultFences: UploadFenceStore = {
  get: key => window.sessionStorage.getItem(key),
  set: (key, value) => window.sessionStorage.setItem(key, value),
  delete: key => window.sessionStorage.removeItem(key),
};
const defaults: PublicUploadDependencies = {
  capabilities, proveService, selectedSigner, publicOrigin: () => window.location.origin,
  createRequest: context => createNostrUploadRequest(context),
  catalogueRead: (operation, options) => publicCatalogueRead(operation, options),
  multipart: serializeUploadMultipart,
  digest: async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new Uint8Array(bytes)))].map(byte => byte.toString(16).padStart(2, '0')).join(''),
  decodeImage: async blob => { const image = await createImageBitmap(blob); try { return { width: image.width, height: image.height }; } finally { image.close(); } },
  destroy: onDestroy, fences: defaultFences,
};
const encoder = new TextEncoder();
const unknownPage = 'Outcome unknown. This image may already be staged. An exact owner receipt is required before removing or replacing it. No image POST was replayed.';
const unknownCommit = 'The publication outcome is unresolved. This exact upload is read-only until its saved receipt is explicitly reconciled. No commit was replayed.';
const mimeExtensions: Record<string, string> = { 'image/png': '.png', 'image/jpeg': '.jpg', 'image/gif': '.gif', 'image/webp': '.webp' };
const fail = (message: string, status = 502): never => { throw new PublicApiContractError(message, status); };
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
export function createPublicUploadAdapter(injected: Partial<PublicUploadDependencies> = {}) {
  const deps = { ...defaults, ...injected };
  type Account = { user: User; actor: string; scope: Scope; identity: UploadRequestIdentity; facts: UploadAccountFacts; assertCurrent: () => void; key: string };
  type Owned = { account: Account; sessionId?: string; fence?: Fence; latest?: PublicSession; groups: Map<string, ScanlationGroup>; request: ReturnType<PublicUploadDependencies['createRequest']>; assert: () => void };
  const accounts = new WeakMap<object, Account>(), owners = new WeakMap<PublicUploadOwner, Owned>();
  function scope(policy: Policy): Scope {
    if (!policy.available || !policy.schema_ready || !policy.origin || !policy.service_key || !policy.catalog_namespace || !/^[0-9a-f]{64}$/.test(policy.service_key)) fail('The configured publication service is unavailable', 503);
    return { origin: canonicalOrigin(deps.publicOrigin()), proofOrigin: canonicalOrigin(policy.origin), serviceKey: policy.service_key, catalogNamespace: publicUuid(policy.catalog_namespace) };
  }
  function equalScope(a: Scope, b: Scope) { return a.origin === b.origin && a.proofOrigin === b.proofOrigin && a.serviceKey === b.serviceKey && a.catalogNamespace === b.catalogNamespace; }
  async function refresh(account: Account) { account.identity.check(); account.assertCurrent(); if (!equalScope(account.scope, scope(await deps.capabilities()))) fail('Configured upload scope changed. Review the original target again.', 409); account.identity.check(); account.assertCurrent(); }
  function transport(account: Account, assertCurrent: () => void) {
    return deps.createRequest({ scope: account.scope, identity: account.identity, assertCurrent, beforeSend: async signal => { if (signal.aborted) fail('Upload cancelled', 409); assertCurrent(); await refresh(account); assertCurrent(); } });
  }
  function readFence(account: Account): Fence | undefined {
    const stored = deps.fences.get(account.key); if (stored === null) return;
    let row: Record<string, unknown>; try { row = record(JSON.parse(stored)); } catch { return fail('Saved upload recovery state is unreadable. No mutation was sent.', 409); }
    const sessionId = publicUuid(row.sessionId); if (!['uncertain', 'accepted-unavailable'].includes(String(row.state))) fail('Saved upload recovery state is invalid', 409);
    return { sessionId, state: row.state as Fence['state'] };
  }
  function fence(owned: Owned, value?: Fence) {
    // Persist before dispatch. A failed store write prevents the new commit.
    if (value) deps.fences.set(owned.account.key, JSON.stringify(value)); else deps.fences.delete(owned.account.key);
    owned.fence = value;
    if (owned.latest && value) { owned.latest.can_mutate = false; owned.latest.can_cancel = false; owned.latest.can_retry_commit = false; owned.latest.receipt = undefined; owned.latest.committed_result = undefined; owned.latest.public_upload.commitState = value.state; }
  }
  const ownedBy = (owner?: PublicUploadOwner): Owned => { if (!owner || !owners.has(owner)) fail('An explicit signed upload owner is required', 409); owner.assert(); return owners.get(owner)!; };
  function assertMutable(owned: Owned) { if (owned.fence || readFence(owned.account)) throw new VisibleError('Reconcile publication first', unknownCommit); }
  async function raw(owned: Owned, path: string, method: PublicUploadRequest['method'], payload?: unknown, responseType: 'json' | 'image' = 'json') {
    owned.assert(); const response = await owned.request({ url: owned.account.scope.origin + '/api/v1' + path, method, responseType, ...(payload === undefined ? {} : { body: encoder.encode(JSON.stringify(payload)), contentType: 'application/json' }) }); owned.assert(); return response;
  }
  function basicSession(input: unknown): string { const row = record(input, 'upload session resource'); if (row.type !== 'upload_session') fail('Invalid upload session resource'); const attrs = record(row.attributes); integer(attrs.version, 1); boolean(attrs.isCommitted); boolean(attrs.isDeleted); boolean(attrs.isProcessed); return publicUuid(row.id); }
  function normalizeFile(input: unknown, sessionId: string, detailed: boolean): PublicFile {
    const source = record(input, 'upload file'), row = detailed ? source : record(source.attributes, 'upload file attributes');
    const id = publicUuid(source.id); if (!detailed && source.type !== 'upload_session_file') fail('Wrong upload file resource kind');
    const name = text(row.originalFileName, 'original filename'), hash = text(row.fileHash, 'original digest'), mime = text(row.mimeType, 'image MIME');
    if (!/^[0-9a-f]{64}$/.test(hash) || !mimeExtensions[mime]) fail('Invalid original image receipt');
    const size = integer(row.fileSize, 1); if (size > 25 * 1024 * 1024) fail('Upload receipt exceeds image budget');
    const facts: FileFacts = { mimeType: mime, fileSize: size };
    if (detailed) { const dims = record(row.dimensions, 'original image dimensions'); facts.dimensions = { width: integer(dims.width, 1), height: integer(dims.height, 1) }; const expected = `/api/v1/nostr/uploads/${sessionId}/files/${id}/preview`; if (row.previewUrl !== expected) fail('Preview URL escaped its exact upload/file scope'); facts.previewUrl = expected; }
    return { id, original_file_name: name, file_hash: hash, file_name: name, file_ext: mimeExtensions[mime], file_size: String(size), public_upload: facts };
  }
  function normalizeSession(input: unknown, expected: string): PublicSession {
    const envelope = record(input, 'upload snapshot'); if (envelope.result !== 'ok') fail('Invalid upload snapshot envelope'); const row = record(envelope.session, 'upload snapshot session');
    const id = publicUuid(row.id); if (id !== expected) fail('Upload snapshot changed the requested session', 409);
    const mangaId = publicUuid(row.mangaId), chapterId = row.chapterId === null ? undefined : publicUuid(row.chapterId), groupIds = ids(row.groupIds, 10), order = ids(row.pageOrder);
    if (!Array.isArray(row.files) || row.files.length > 500) fail('Invalid staged image list'); const files = row.files.map(file => normalizeFile(file, id, true));
    if (files.length !== order.length || new Set(files.map(file => file.id)).size !== files.length || order.some(file => !files.some(row => row.id === file))) fail('Staged order does not cover the exact current image set');
    const isCommitted = boolean(row.isCommitted), canMutate = boolean(row.canMutate), canCancel = boolean(row.canCancel), canRetry = boolean(row.canRetryCommit), isExpired = boolean(row.isExpired), state = text(row.state, 'upload state');
    if (!['open', 'committed', 'deleted'].includes(state) || isCommitted !== (state === 'committed') || (isCommitted || state === 'deleted' || isExpired) && (canMutate || canCancel) || canMutate && !canRetry) fail('Inconsistent upload state');
    const receipt = row.receipt === null ? null : resource(row.receipt, 'chapter'); if (receipt && (!isCommitted || !receipt.relationships.some(relation => relation.type === 'manga' && publicUuid(relation.id) === mangaId) || chapterId && publicUuid(receipt.id) !== chapterId)) fail('Upload receipt escaped its exact target');
    const receiptStatus = row.receiptStatus; if (!['none', 'available', 'unavailable'].includes(String(receiptStatus)) || !!receipt !== (receiptStatus === 'available') || isCommitted && receiptStatus === 'none' || !isCommitted && receiptStatus !== 'none') fail('Invalid exact receipt availability');
    const publication = row.publicationStatus === null ? undefined : text(row.publicationStatus, 'publication status');
    return { id, version: integer(row.version, 1), is_committed: isCommitted, manga_id: mangaId, ...(chapterId ? { chapter_id: chapterId } : {}), draft: row.draft === null ? null : draft(row.draft), page_order: order, can_mutate: canMutate, can_cancel: canCancel, can_retry_commit: canRetry, is_expired: isExpired, expires_at: text(row.expiresAt, 'upload expiry'), state, publication_status: publication, target_status: text(row.targetStatus, 'upload target status'), relationships: { files: order.map(id => files.find(file => file.id === id)!) }, public_upload: { groupIds, receipt, receiptStatus } } as PublicSession;
  }
  async function exact(owned: Owned, sessionId: string): Promise<PublicSession> {
    owned.fence = owned.fence ?? readFence(owned.account);
    const id = publicUuid(sessionId); if (owned.sessionId && owned.sessionId !== id || owned.fence && owned.fence.sessionId !== id) fail('The owned upload session changed', 409);
    const response = await raw(owned, `/nostr/uploads/${id}`, 'GET'); if (response.status !== 200) throw resumeError(response.status, response.body);
    const session = normalizeSession(response.body, id); owned.sessionId = id; owned.latest = session;
    for (const groupId of session.public_upload.groupIds) if (!owned.groups.has(groupId)) {
      owned.assert(); await refresh(owned.account); owned.assert();
      const groupResponse = await deps.catalogueRead('group.get', { id: groupId }); owned.assert(); await refresh(owned.account); owned.assert();
      if (groupResponse.status !== 200) throw resumeError(groupResponse.status, groupResponse.body);
      const group = record(groupResponse.body, 'actual upload group'), actual = resource(record(group.public_api, 'actual group provenance').resource, 'scanlation_group');
      if (publicUuid(group.id) !== groupId || publicUuid(actual.id) !== groupId || text(group.name, 'group name') !== actual.attributes.name) fail('Upload group presentation changed its exact UUID or name');
      owned.groups.set(groupId, groupResponse.body as ScanlationGroup);
    }
    session.relationships.groups = session.public_upload.groupIds.map(groupId => owned.groups.get(groupId)!);
    if (session.is_committed && !owned.fence) fence(owned, { sessionId: id, state: 'accepted-unavailable' });
    const pending = owned.fence ?? readFence(owned.account); if (pending) { owned.fence = pending; session.can_mutate = false; session.can_cancel = false; session.can_retry_commit = false; session.public_upload.commitState = pending.state; }
    return session;
  }
  async function discover(owned: Owned): Promise<Response<PublicSession>> {
    owned.fence = owned.fence ?? readFence(owned.account);
    // A remembered exact session is never replaced by the ambiguous current-session route.
    if (owned.fence || owned.sessionId) return { status: 200, body: await exact(owned, (owned.fence?.sessionId ?? owned.sessionId)!) };
    const response = await raw(owned, '/upload', 'GET'); if (response.status !== 200) return response as Response<PublicSession>;
    const id = basicSession(response.body); owned.sessionId = id; return { ...response, body: await exact(owned, id) };
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
  async function loadUploadAccount(assertCurrent = () => {}): Promise<User> {
    assertCurrent(); const identity = deps.selectedSigner(); identity.check(); const captured = scope(await deps.capabilities()); assertCurrent();
    const proof = await deps.proveService(captured.proofOrigin, captured.serviceKey); identity.check(); assertCurrent(); if (proof.origin !== captured.proofOrigin || proof.serviceKey !== captured.serviceKey) fail('Upload service proof differs from configured service');
    const account: Account = { user: undefined as unknown as User, actor: '', scope: captured, identity, facts: undefined as unknown as UploadAccountFacts, assertCurrent, key: `weebdex.public-upload-recovery.v1:${captured.origin}:${captured.serviceKey}:${captured.catalogNamespace}:${identity.owner}` };
    await refresh(account); const request = transport(account, () => { identity.check(); assertCurrent(); });
    const response = await request({ url: captured.origin + '/api/v1/nostr/account-context', method: 'GET' }); identity.check(); assertCurrent(); if (response.status !== 200) throw resumeError(response.status, response.body);
    const result = record(response.body, 'bound upload account'), user = record(result.user, 'bound user'), nostr = record(result.nostr, 'bound Nostr identity'), upload = record(result.upload, 'upload grants');
    if (result.result !== 'ok' || nostr.pubkey !== identity.owner || nostr.serviceKey !== captured.serviceKey || publicUuid(nostr.catalogNamespace) !== captured.catalogNamespace || nostr.authentication !== 'NIP-98' || nostr.requestScoped !== true || nostr.nativeSessionAuthority !== false || nostr.catalogueScope !== 'instance-local' || upload.conditional !== true) fail('Upload account context does not match the selected signer/service/catalogue');
    const permissions = strings(user.permissions), roles = strings(user.roles), publication = record(upload.publication), limits = record(upload.limits);
    const canBegin = boolean(upload.canBegin), canManage = boolean(upload.canManage); if (canBegin !== permissions.includes('chapter.create') || canManage !== permissions.includes('self.chapter.update') || publication.requiresOperatorOrAuthorizedGroup !== true) fail('Upload eligibility was not backed by actual native grants');
    const view = { id: publicUuid(user.id), name: text(user.username, 'bound username'), roles, permissions } as User;
    account.actor = view.id; account.user = view; account.facts = { canBegin, canManage, permissions, publication, limits }; accounts.set(view, account); return view;
  }
  function publicUploadOwner(target: () => string, user: User): PublicUploadOwner {
    const account = accounts.get(user); if (!account) fail('Load the selected signer’s bound upload account first', 409);
    const id = publicUuid(target()); let disposed = false;
    const assert = () => { if (disposed || publicUuid(target()) !== id || user.id !== account.actor) fail('The upload editor target changed. No request was replayed.', 409); account.identity.check(); account.assertCurrent(); if (canonicalOrigin(deps.publicOrigin()) !== account.scope.origin) fail('Public upload origin changed', 409); };
    const initialFence = readFence(account);
    const owned: Owned = { account, assert, groups: new Map(), request: transport(account, assert), sessionId: initialFence?.sessionId, fence: initialFence };
    const owner: PublicUploadOwner = { id, actor: account.actor, assert, alive: () => { try { assert(); return true; } catch { return false; } }, dispose: () => { disposed = true; }, commitState: () => (owned.fence ?? readFence(account))?.state, pendingCommitSession: () => (owned.fence ?? readFence(account))?.sessionId, request: async <T>(route: string, method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE', payload?: unknown): Promise<Response<T>> => {
      assert(); if (!route.startsWith('/upload') || /[?#\\]/.test(route)) fail('Unsupported signed upload route', 422);
      if (route === '/upload' && method === 'GET') return await discover(owned) as Response<T>;
      assertMutable(owned);
      if (route === '/upload/check-approval-required' && method === 'POST') { const value = record(payload); const locale = text(value.language ?? value.locale, 'upload language'); const manga = owned.latest?.manga_id ?? id; const response = await raw(owned, route, method, { manga, locale }); if (response.status === 200 && (record(response.body).result !== 'ok' || typeof record(response.body).requiresApproval !== 'boolean')) fail('Invalid actual approval decision'); return response as Response<T>; }
      if ((route === '/upload/begin' || route.startsWith('/upload/begin/')) && method === 'POST') {
        const value = record(payload); let body: unknown;
        if (route === '/upload/begin') { if (!account.facts.canBegin || !account.facts.canManage || publicUuid(value.manga) !== id) fail('The bound native account cannot begin this target upload', 403); body = { manga: id, groups: ids(value.groups, 10) }; }
        else { if (value.replace === true) fail('Replace-all has no public upload command. No native request was sent.', 422); if (publicUuid(route.slice('/upload/begin/'.length)) !== id || !account.facts.canManage) fail('The bound native account cannot edit this upload target', 403); body = { version: integer(value.version, 1) }; }
        if (owned.sessionId) fail('An owned upload already exists. Resume or explicitly cancel it first.', 409);
        const response = await raw(owned, route, method, body); if (response.status !== 200) return response as Response<T>;
        owned.sessionId = basicSession(response.body); return { ...response, body: await exact(owned, owned.sessionId) } as Response<T>;
      }
      const match = /^\/upload\/([^/]+)(?:\/([^/]+))?$/.exec(route); if (!match) fail('Unsupported signed upload route', 422);
      const sessionId = publicUuid(match[1]), action = match[2]; if (owned.sessionId !== sessionId) fail('The upload request escaped its owned session', 409);
      if (action === 'commit' && method === 'POST') {
        const value = record(payload); if (value.termsAccepted !== true) fail('Explicit upload terms acceptance is required', 422);
        const candidate = { draft: draft(value.draft), page_order: ids(value.page_order) }; if (!candidate.page_order.length || !candidate.draft.language) fail('A language and staged page order are required', 422);
        const current = await exact(owned, sessionId); if (!current.can_mutate || !current.can_retry_commit || !sameDraft(current.draft, candidate.draft) || JSON.stringify(current.page_order) !== JSON.stringify(candidate.page_order)) fail('Commit must match the acknowledged private draft and exact page order', 409);
        const body = { chapterDraft: { volume: candidate.draft.volume || null, chapter: candidate.draft.chapter || null, title: candidate.draft.title || null, translatedLanguage: candidate.draft.language }, pageOrder: candidate.page_order, termsAccepted: true };
        fence(owned, { sessionId, state: 'uncertain' });
        try {
          const response = await raw(owned, route, 'POST', body);
          if (response.status !== 200) { if (response.status >= 400 && response.status < 500 && record(response.body).result === 'error' && Array.isArray(record(response.body).errors)) { fence(owned); return response as Response<T>; } throw resumeError(response.status, response.body); }
          const receipt = resource(response.body, 'chapter'); if (!receipt.relationships.some(relation => relation.type === 'manga' && publicUuid(relation.id) === current.manga_id)) fail('Accepted receipt changed its target');
          fence(owned, { sessionId, state: 'accepted-unavailable' });
          const result = await presentation(owned, response.body, current.manga_id);
          if (!result.body) fence(owned, { sessionId, state: 'accepted-unavailable' }); else { fence(owned); owned.sessionId = undefined; owned.latest = undefined; }
          return { ...response, ...result, accepted: true, sessionId } as Response<T>;
        } catch (error) { if (owned.fence?.state === 'uncertain' && error && typeof error === 'object' && 'outcome' in error && error.outcome === 'unsent') fence(owned); else if (owned.fence) fence(owned, owned.fence); throw error; }
      }
      if (method === 'PATCH' && action === undefined) { const value = record(payload); const current = await exact(owned, sessionId); if (!current.can_mutate || !account.facts.canManage) fail('Upload is read-only', 409); const body = { version: integer(value.version, 1), draft: draft(value.draft), pageOrder: ids(value.page_order ?? value.pageOrder) }; const response = await raw(owned, `/nostr/uploads/${sessionId}`, 'PATCH', body); return response.status === 200 ? { ...response, body: (owned.latest = normalizeSession(response.body, sessionId)) } as Response<T> : response as Response<T>; }
      if (method === 'DELETE' && (action === undefined || action === 'batch' || /^[0-9a-f-]{36}$/i.test(action))) {
        const current = await exact(owned, sessionId); if (!(action ? current.can_mutate : current.can_cancel) || !account.facts.canManage) fail('Upload is read-only', 409);
        const known = new Set(current.page_order); let body: string[] | undefined; if (action === 'batch') body = ids(payload); else if (action) body = [publicUuid(action)]; if (body && (!body.length || body.some(file => !known.has(file)))) fail('Deletion must name exact current staging files', 409);
        const response = await raw(owned, route, 'DELETE', action === 'batch' ? body : undefined); if (response.status === 200) { if (record(response.body).result !== 'ok') fail('Deletion was not confirmed'); if (!action) { owned.sessionId = undefined; owned.latest = undefined; } } return response as Response<T>;
      }
      fail('Unsupported public upload operation. No native mutation was sent.', 422);
    } };
    owners.set(owner, owned); deps.destroy(owner.dispose); return owner;
  }
  async function currentUpload(owner?: PublicUploadOwner): Promise<PublicSession | undefined> { const owned = ownedBy(owner), response = await discover(owned); if (response.status === 404 && !owned.sessionId && !owned.fence) return; if (response.status !== 200 || !response.body) throw resumeError(response.status, response.body); return response.body; }
  async function reconcileUploadReceipt(owner: PublicUploadOwner, sessionId: string): Promise<PublicSession> {
    const owned = ownedBy(owner), id = publicUuid(sessionId), pending = owned.fence ?? readFence(owned.account); if (!pending || pending.sessionId !== id) fail('No exact unresolved publication matches this recovery request', 409);
    const session = await exact(owned, id); if (!session.is_committed || !session.public_upload.receipt) throw new VisibleError('Publication is still unresolved', unknownCommit);
    const shown = await presentation(owned, session.public_upload.receipt, session.manga_id); if (!shown.body) { fence(owned, { sessionId: id, state: 'accepted-unavailable' }); return session; }
    session.receipt = shown.body; session.committed_result = shown.body; session.can_mutate = false; session.can_cancel = false; session.can_retry_commit = false; fence(owned); owned.sessionId = undefined; return session;
  }
  async function saveResume(session: UploadSession, value: UploadSnapshot, owner?: PublicUploadOwner): Promise<PublicSession> {
    const owned = ownedBy(owner); assertMutable(owned); const previous = session as PublicSession, fresh = await exact(owned, publicUuid(session.id));
    if (!fresh.can_mutate || !sameDraft(fresh.draft, previous.draft) && !sameDraft(fresh.draft, value.draft)) throw resumeError(409);
    const known = new Set(previous.page_order ?? []), prior = (previous.page_order ?? []).filter(id => fresh.page_order.includes(id)), remote = fresh.page_order.filter(id => known.has(id));
    if (JSON.stringify(prior) !== JSON.stringify(remote) && JSON.stringify(fresh.page_order) !== JSON.stringify(value.page_order)) throw resumeError(409);
    if (value.page_order.length !== fresh.page_order.length || new Set(value.page_order).size !== value.page_order.length || value.page_order.some(id => !fresh.page_order.includes(publicUuid(id)))) throw resumeError(409);
    const response = await owner!.request<PublicSession>(`/upload/${session.id}`, 'PATCH', { version: fresh.version, ...value }); if (response.status !== 200 || !response.body) throw resumeError(response.status, response.body); return response.body;
  }
  async function cancelResume(session: UploadSession, owner?: PublicUploadOwner) { const response = await owner!.request(`/upload/${publicUuid(session.id)}`, 'DELETE'); if (response.status !== 200) throw resumeError(response.status, response.body); }
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
  async function uploadPreview(owner: PublicUploadOwner, session: UploadSession | string, input: UploadSessionFile, signal?: AbortSignal): Promise<Blob> {
    const owned = ownedBy(owner), sessionId = publicUuid(typeof session === 'string' ? session : session.id), fileId = publicUuid(input.id), fresh = await exact(owned, sessionId), file = (fresh.relationships.files as PublicFile[]).find(file => file.id === fileId);
    if (!file || file.file_hash !== input.file_hash || file.file_size !== input.file_size || !file.public_upload?.dimensions || !file.public_upload.previewUrl) fail('Preview no longer matches the exact staged original', 409);
    const facts = file.public_upload, response = await owned.request({ url: owned.account.scope.origin + facts.previewUrl, method: 'GET', responseType: 'image', signal, responseLimit: facts.fileSize }); owned.assert();
    const mime = response.headers['content-type']?.split(';', 1)[0].trim().toLowerCase(); if (response.status !== 200 || !(response.body instanceof Uint8Array) || response.body.byteLength !== facts.fileSize || mime !== facts.mimeType || await deps.digest(response.body) !== file.file_hash) fail('Signed preview did not match the original image receipt');
    owned.assert(); const blob = new Blob([new Uint8Array(response.body)], { type: facts.mimeType }), dimensions = await deps.decodeImage(blob); owned.assert(); if (dimensions.width !== facts.dimensions!.width || dimensions.height !== facts.dimensions!.height) fail('Decoded preview dimensions differ from the original receipt'); return blob;
  }
  function uploadAccountFacts(user: User) { const account = accounts.get(user); if (!account) fail('No bound upload account context', 409); return structuredClone(account.facts); }
  return { loadUploadAccount, publicUploadOwner, uploadAccountFacts, currentUpload, saveResume, cancelResume, reconcileUploadReceipt, runUploadBatch, resolveUploadPage, uploadPreview };
}

export function markUnsentUploadPages(pages: UploadSessionFile[], session: string) { for (const page of pages) if (page.file && page.upload_recovery?.state !== 'unknown') { page.error = 'Not sent because this batch has not reached this image. Remove or replace this local placeholder explicitly.'; page.upload_recovery = { state: 'unsent', session, before_ids: [] }; } }
export function isRemoteUploadFile(value: UploadSessionFile): boolean { try { publicUuid(value.id); return !value.file && !value.error && !value.upload_recovery && /^[0-9a-f]{64}$/.test(value.file_hash) && /^\d+$/.test(value.file_size) && Number.isSafeInteger(Number(value.file_size)) && Number(value.file_size) > 0; } catch { return false; } }
export function prepareResume(session: UploadSession): PublicSession { const value = session as PublicSession, order = new Map(value.page_order.map((id, index) => [id, index])); value.relationships.files = [...value.relationships.files ?? []].sort((a, b) => order.get(a.id)! - order.get(b.id)!); return value; }
export function acknowledge(session: UploadSession, saved: UploadSession) { const current = session as PublicSession, next = saved as PublicSession; for (const key of ['version', 'draft', 'page_order', 'can_mutate', 'can_cancel', 'can_retry_commit', 'publication_status', 'state', 'is_expired', 'expires_at', 'public_upload'] as const) Object.assign(current, { [key]: next[key] }); }
export function resumeNotice(session: UploadSession) { const value = session as PublicSession; if (value.public_upload?.commitState) return unknownCommit; if (value.is_committed) return 'The exact saved chapter receipt was restored. Reader navigation requires its current readable catalogue presentation.'; if (!value.can_mutate) return 'This saved upload is read-only with its current state or authorization. Its staged work has been kept.'; return 'Your original images, metadata and page order are staged privately on this instance.'; }
export function immutableCommit(session: UploadSession, value: UploadSnapshot): UploadSnapshot { const current = session as PublicSession; if (!current.can_mutate || !current.can_retry_commit || current.is_committed || current.public_upload?.commitState) throw new VisibleError('Reconcile publication first', resumeNotice(session)); return { draft: draft(value.draft), page_order: ids(value.page_order) }; }
const production = createPublicUploadAdapter();
export const { loadUploadAccount, publicUploadOwner, uploadAccountFacts, currentUpload, saveResume, cancelResume, reconcileUploadReceipt, runUploadBatch, resolveUploadPage, uploadPreview } = production;
