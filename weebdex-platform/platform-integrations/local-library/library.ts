import {cataloguePreviewLibraryUnavailable, CATALOGUE_PREVIEW_LIBRARY_FENCE, assertCataloguePreviewLibraryReady, publicUuidLibraryEnabled, publicAccountLibraryUnavailable, assertAccountLibraryReady} from '../public-api/library-staging';
import { get, writable, derived } from 'svelte/store';
import Session from '$lib/stores/Session';
import { PUBLIC_API_URL } from '$env/static/public';
import { HTTPError } from '$lib/HTTP';
import { DeviceStore, PUBLIC_DEVICE_FORMAT_FENCE, type DeviceRecord, type DeviceSnapshot, type Family, catalogueId, normalizeDevice, validateSnapshot, sameScope, type InstanceScope } from './device-store';
import { request } from './protocol/indexeddb.js';
import {uuidPattern} from '../public-api/validation';
import { LocalLibrary } from './protocol/store.js';
import type { MigrationReview } from '../public-api/device-id-migration';
import type { DeviceListNameReview } from './device-list-name';
export type { DeviceListNameReview } from './device-list-name';
import { captureNativeAuthority, sameNativeReadAuthority } from '$platform/mutation-authority';
export type Mode = 'device' | 'account';
export type ScopeStatus = 'pending' | 'verified' | 'offline' | 'origin_only' | 'migration_required';
export type LibraryState = { scopeStatus: ScopeStatus; scopeReason?: string; cachedScopeReady?: boolean; ready: boolean; mode: Mode; revision: number; accountRevision: number; records: DeviceRecord[]; error?: string };
export const localLibraryState = writable<LibraryState>({ scopeStatus: 'pending', ready: false, mode: 'device', revision: 0, accountRevision: 0, records: [] });
// Unresolved legacy rows remain in backups/recovery, never in UUID catalogue reads.
export const isCurrentCatalogueId=(id:unknown)=>typeof id==='string'&&(!publicUuidLibraryEnabled||uuidPattern.test(id));
export const libraryIdentity = derived([localLibraryState, Session], ([local, account]) => ({
  ready: local.ready, mode: local.mode, accountId: account?.identity.id ?? null, canUse: !cataloguePreviewLibraryUnavailable && local.ready && (local.mode === 'account' ? !publicAccountLibraryUnavailable && !!account : local.scopeStatus === 'verified'),
  // This is library availability, never an account Session or permission identity.
  follows: cataloguePreviewLibraryUnavailable ? {} : local.mode === 'device' ? (local.scopeStatus !== 'verified' ? {} : Object.fromEntries(local.records.filter(row => row.family === 'series' && isCurrentCatalogueId(row.id)).map(row => [row.id!, [row.status!, row.notification ? 1 : 0]]))) : (publicAccountLibraryUnavailable ? {} : account?.follows ?? {})
}));
export const libraryViewKey = derived([localLibraryState, libraryIdentity], ([local, identity]) => identity.mode === 'device' ? `device:${local.revision}:${local.scopeStatus}` : `account:${identity.accountId ?? 'none'}:${local.accountRevision}`);
export const captureLibraryView = () => get(libraryViewKey);
export function assertLibraryView(expected: string) {
  if (get(libraryViewKey) !== expected) throw new HTTPError(409, 'The selected library changed while loading. Reload this view.', 'Library changed');
}
// Every Session write invalidates an in-flight account reconciliation, including
// same-ID permission/follow refreshes and A -> B -> A switches. This is not a
// login authority; it only prevents old responses restoring newer store state.
let sessionWriteGeneration = 0;
Session.subscribe(() => { sessionWriteGeneration++; });
let importContextGeneration = 0, importContextKey = '';
derived([localLibraryState, Session], ([local, account]) => JSON.stringify([local.mode, local.scopeStatus, account?.identity.id ?? null])).subscribe(key => {
  if (key !== importContextKey) { importContextKey = key; importContextGeneration++; }
});
export function captureLibraryContext() { return { generation: importContextGeneration, key: importContextKey, mode: get(localLibraryState).mode, accountId: get(Session)?.identity.id ?? null }; }
export function assertLibraryContext(context: ReturnType<typeof captureLibraryContext>) { if (context.generation !== importContextGeneration || context.key !== importContextKey) throw new HTTPError(409, 'The selected library authority changed while saving. The original destination may have changed; refresh before another action.', 'Library context changed'); }
// Explicit selections are captured before any storage/network wait. The first
// persisted mode discovery is permitted once, never a subsequent user selection.
let selectionEpoch = 0;
const readerSelectionState = writable({ epoch: 0, pending: false });
// Read-only notification starts before useMode waits for storage or Session.
// A reader cannot admit an old-mode action while a new choice is pending.
export const readerLibrarySelection = { subscribe: readerSelectionState.subscribe };
export function captureLibraryAdmission() {
  return { authority: captureNativeAuthority(), selectionEpoch, context: get(localLibraryState).ready ? captureLibraryContext() : undefined };
}
export function assertLibraryAdmission(admission: ReturnType<typeof captureLibraryAdmission>) {
  if (admission.selectionEpoch !== selectionEpoch || !sameNativeReadAuthority(admission.authority)) throw new HTTPError(409, 'The reader account or selected library changed while opening. No old reader action was admitted to the new destination.', 'Library admission changed');
  if (admission.context) assertLibraryContext(admission.context);
}
export type LibraryImportContext = { readonly mode: Mode; readonly accountId: string | null; readonly scope: InstanceScope; readonly generation: number };
export async function captureImportContext(): Promise<LibraryImportContext> {
  await assertCatalogueScope();
  const local = get(localLibraryState), accountId = get(Session)?.identity.id ?? null;
  if (local.mode === 'account') { if (!accountId) throw new HTTPError(401, 'Sign into the selected account before importing'); await authoritativeAccount(accountId); }
  const snapshot = await currentDeviceSnapshot();
  const context = Object.freeze({ mode: local.mode, accountId, scope: Object.freeze({ ...snapshot.scope }), generation: importContextGeneration });
  assertImportContext(context, snapshot.scope); return context;
}
export function assertImportContext(context: LibraryImportContext, scope: InstanceScope = context.scope) {
  if (context.generation !== importContextGeneration || context.mode !== get(localLibraryState).mode || context.accountId !== (get(Session)?.identity.id ?? null) || !sameScope(context.scope, scope)) throw new HTTPError(409, 'The selected library mode, catalogue scope or account changed. Import stopped; earlier acknowledged batches remain in their original destination. Review the file again before continuing.', 'Import context changed');
  assertCurrentCatalogueScopeValue(scope);
}
let device: DeviceStore, protocol: LocalLibrary, opening: Promise<void> | undefined, refreshGeneration = 0;
export function openLibrary(): Promise<void> {
  return opening ??= (async () => {
    try {
      const apiOrigin = new URL(PUBLIC_API_URL || '/api', window.location.href).origin;
      // Opening local records never waits for the network. Saved scope is read
      // atomically; online identity discovery is an independent bounded check.
      device = await DeviceStore.open({ origin: apiOrigin, serviceKey: null, catalogNamespace: null }, () => { void refreshLibrary().catch(() => {}); }, () => localLibraryState.update(state => ({ ...state, ready: false, error: 'Local database changed in another tab. Reload before saving.' })), {idFormat:publicUuidLibraryEnabled?'public_uuid-v2':'native-v1'});
      protocol = await LocalLibrary.open('weebdex-protocol-library-v1');
      const preference = await protocol.getMeta('weebdexLibraryMode');
      localLibraryState.update(state => ({ ...state, mode: preference === 'account' ? 'account' : 'device' }));
      await refreshLibrary();
      void verifyCatalogueScope();
      window.addEventListener('online', () => { void verifyCatalogueScope(); });
    } catch (error) {
      localLibraryState.update(state => ({ ...state, ready: false, error: `Cannot open durable local storage: ${message(error)}. No server fallback was used.` }));
      throw error;
    }
  })();
}
let scopeVerification: Promise<void> | undefined;
let verifiedCatalogueScope: InstanceScope | undefined;
let readerObservedCatalogue: { url: string; scope: InstanceScope; generation: number } | undefined;
export function verifyCatalogueScope(): Promise<void> {
  if (scopeVerification) return scopeVerification;
  localLibraryState.update(state => state.scopeStatus === 'migration_required' ? state : ({ ...state, scopeStatus: 'pending', scopeReason: undefined }));
  scopeVerification = (async () => {
    try {
      const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + '/protocol/capabilities', { credentials: 'omit', cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(8000) });
      if (!response.ok) throw Error(`Scope discovery unavailable: HTTP ${response.status}`);
      const { bytesResponse } = await import('./protocol/nostr.js');
      const policy = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await bytesResponse(response, 65536)));
      if (typeof policy.catalog_namespace !== 'string' || !/^[0-9a-f-]{32,36}$/.test(policy.catalog_namespace) || (policy.service_key !== null && policy.service_key !== undefined && !/^[0-9a-f]{64}$/.test(policy.service_key))) throw Error('Instance returned an invalid catalogue scope');
      const liveScope: InstanceScope = { origin: new URL(PUBLIC_API_URL || '/api', window.location.href).origin, catalogNamespace: policy.catalog_namespace, serviceKey: policy.service_key ?? null };
      readerObservedCatalogue = { url: new URL(PUBLIC_API_URL.replace(/\/$/, '') + '/protocol/capabilities', window.location.href).href, scope: { ...liveScope }, generation: (readerObservedCatalogue?.generation ?? 0) + 1 };
      const result = await device.verifyLiveScope(liveScope);
      if (result.status === 'verified') { const saved = await device.snapshot(); if (publicUuidLibraryEnabled ? saved.scope.origin !== liveScope.origin || saved.scope.catalogNamespace !== liveScope.catalogNamespace || !!saved.scope.serviceKey && !!liveScope.serviceKey && saved.scope.serviceKey !== liveScope.serviceKey : !sameScope(saved.scope, liveScope)) throw Error('Current service identity does not confirm the saved catalogue scope'); verifiedCatalogueScope = saved.scope; } else verifiedCatalogueScope = undefined;
      localLibraryState.update(state => ({ ...state, scopeStatus: cataloguePreviewLibraryUnavailable ? 'migration_required' : result.status, scopeReason: cataloguePreviewLibraryUnavailable ? CATALOGUE_PREVIEW_LIBRARY_FENCE : result.reason, cachedScopeReady:result.status==='verified' }));
    } catch (error) {
      localLibraryState.update(state => state.scopeStatus === 'migration_required' ? ({ ...state, scopeReason: `${state.scopeReason ?? 'A catalogue scope mismatch was detected; native mutations are fenced.'} Scope recheck failed: ${message(error)}. Inspection and backups remain available.` }) : ({ ...state, scopeStatus: 'offline', scopeReason: `Catalogue scope could not be verified: ${message(error)}. Saved local records, edits and backups remain available; current catalogue hydration/publication needs verification.` }));
    } finally { scopeVerification = undefined; }
  })();
  return scopeVerification;
}
export async function assertCatalogueScope() {
  assertCataloguePreviewLibraryReady();
  await openLibrary();
  if (get(localLibraryState).scopeStatus === 'pending') await scopeVerification;
  const current = get(localLibraryState);
  if (current.scopeStatus !== 'verified') throw new HTTPError(409, current.scopeReason ?? 'Verify this catalogue scope before hydrating native IDs', 'Catalogue scope unverified');
}
// Hydration and new catalogue actions require live verification. Previously
// verified same-tab UUID reader progress may save offline to its exact scope.
export function canSaveLocalProgress(state: LibraryState) { return state.ready && state.mode === 'device' && (state.scopeStatus === 'verified' || publicUuidLibraryEnabled && state.cachedScopeReady === true && (state.scopeStatus === 'offline' || state.scopeStatus === 'pending')); }
export function assertCurrentCatalogueScopeValue(scope: InstanceScope, allowCachedProgress = false) {
  assertCataloguePreviewLibraryReady();
  if (!(get(localLibraryState).scopeStatus === 'verified' || allowCachedProgress && canSaveLocalProgress(get(localLibraryState))) || !verifiedCatalogueScope || !sameScope(scope, verifiedCatalogueScope)) throw new HTTPError(409, 'Current catalogue identity is not verified. Saved-device inspection, backups and explicit saved-record edits remain available; no current title ID was admitted.', 'Catalogue scope unverified');
}
async function assertCurrentCatalogueTransaction(meta: IDBObjectStore, allowCachedProgress = false) { assertCurrentCatalogueScopeValue(await request(meta.get('instanceScope')),allowCachedProgress); }
export const message = (error: unknown) => error instanceof HTTPError ? error.error : error instanceof Error ? error.message : String(error);
export async function refreshLibrary() {
  const generation = ++refreshGeneration;
  const snapshot = await device.snapshot();
  if (generation === refreshGeneration) localLibraryState.update(state => ({ ...state, ready: true, revision: snapshot.revision, records: snapshot.records, error: undefined, ...(cataloguePreviewLibraryUnavailable ? { scopeStatus: 'migration_required' as const, scopeReason: CATALOGUE_PREVIEW_LIBRARY_FENCE } : {}), ...(snapshot.version === 2 && !publicUuidLibraryEnabled || snapshot.migration_reason ? { scopeStatus: 'migration_required' as const, scopeReason: snapshot.migration_reason ?? PUBLIC_DEVICE_FORMAT_FENCE, cachedScopeReady:false } : {}) }));
}
export async function stores() { await openLibrary(); return { device, protocol }; }
export async function useMode(mode: Mode) {
  if (!['device', 'account'].includes(mode)) throw Error('Unknown library mode');
  if (mode === 'account') assertAccountLibraryReady();
  const selection = ++selectionEpoch;
  readerSelectionState.set({ epoch: selection, pending: true });
  try {
    await openLibrary();
    if (mode === 'account' && !(await Session.isValid())) throw Error('Sign into a WeebDex account before choosing account-library mode');
    if (selection !== selectionEpoch) throw new HTTPError(409, 'A newer library selection superseded this action');
    const { transaction } = await import('./protocol/indexeddb.js');
    if (selection !== selectionEpoch) throw new HTTPError(409, 'A newer library selection superseded this action before storage dispatch');
    await transaction(protocol.db, 'meta', 'readwrite', (tx: IDBTransaction) => { tx.objectStore('meta').put(mode, 'weebdexLibraryMode'); });
    if (selection !== selectionEpoch) return;
    localLibraryState.update(state => ({ ...state, mode }));
  } finally {
    // A superseded selection must not clear the newer selection's pending fence.
    if (selection === selectionEpoch) readerSelectionState.set({ epoch: selection, pending: false });
  }
}
export async function localAction<T>(operation: () => Promise<T>): Promise<T> {
  assertCataloguePreviewLibraryReady();
  await openLibrary();
  try { const result = await operation(); await refreshLibrary(); return result; }
  catch (error) {
    const text = `Device library could not be saved: ${message(error)}. Nothing was published.`;
    localLibraryState.update(state => ({ ...state, error: text }));
    throw error instanceof HTTPError ? error : new HTTPError(507, text, 'Local storage failed');
  }
}
export async function currentDeviceSnapshot() { await openLibrary(); return device.snapshot(); }
export async function saveSeries(id: string, status: string, notification = true, assertContext: () => void = () => {}) { assertCataloguePreviewLibraryReady();
  catalogueId(id);
  return localAction(async () => {
    await device.mutate(async (rows, meta) => {
      assertContext(); await assertCurrentCatalogueTransaction(meta); assertContext();
      const old = await request(rows.get(['series', id]));
      rows.put(normalizeDevice({ family: 'series', key: id, id, status, notification, followedAt: old?.followedAt ?? new Date().toISOString(), updated_at: Math.floor(Date.now() / 1000) }));
    });
  });
}
export async function removeSeries(id: string, assertContext: () => void = () => {}) { assertCataloguePreviewLibraryReady(); catalogueId(id); return localAction(() => device.mutate(async (rows, meta) => { assertContext(); await assertCurrentCatalogueTransaction(meta); assertContext(); rows.delete(['series', id]); for (const row of await request(rows.getAll())) if (row.family === 'members' && row.series === id) rows.delete([row.family, row.key]); })); }
export async function saveSavedSeries(id: string, status: string, notification: boolean) {
  catalogueId(id); return localAction(() => device.mutate(async rows => { const old = await request(rows.get(['series', id])); if (!old) throw Error('Choose an existing saved-device series; current catalogue IDs need verification'); rows.put(normalizeDevice({ ...old, status, notification, updated_at: Math.floor(Date.now() / 1000) })); }));
}
export async function updateChapters(mangaId: string, read: string[] = [], unread: string[] = [], assertContext: () => void = () => {}) { assertCataloguePreviewLibraryReady();
  catalogueId(mangaId);
  if (read.length + unread.length > 1000) throw Error('Update at most 1000 chapters at once');
  if (read.some(id => unread.includes(id))) throw Error('A chapter cannot be marked read and unread in the same change');
  const readIds = new Set(read.map(catalogueId)), unreadIds = new Set(unread.map(catalogueId));
  await localAction(() => device.mutate(async (rows, meta) => {
    assertContext(); await assertCurrentCatalogueTransaction(meta); assertContext();
    for (const id of [...readIds, ...unreadIds]) {
      const old = await request(rows.get(['chapters', id]));
      if (old && old.mangaId !== mangaId) throw Error('Chapter belongs to a different saved manga');
      rows.put(normalizeDevice({ family: 'chapters', key: id, id, mangaId, completed: readIds.has(id), page: old?.page ?? null, readAt: readIds.has(id) ? old?.readAt ?? new Date().toISOString() : undefined, updated_at: Math.floor(Date.now() / 1000) }));
    }
  }));
  return { read_updated: readIds.size, unread_updated: unreadIds.size };
}
export async function saveProgress(mangaId: string, chapterId: string, page: number, completed: boolean, assertContext: () => void = () => {}) { assertCataloguePreviewLibraryReady();
  catalogueId(mangaId); catalogueId(chapterId);
  assertContext();
  await localAction(() => device.mutate(async (rows, meta) => {
    assertContext(); await assertCurrentCatalogueTransaction(meta,true); assertContext();
    const old = await request(rows.get(['chapters', chapterId]));
    assertContext();
    if (old && old.mangaId !== mangaId) throw Error('Chapter belongs to a different saved manga');
    const done = completed || !!old?.completed;
    rows.put(normalizeDevice({ family: 'chapters', key: chapterId, id: chapterId, mangaId, page, completed: done, readAt: done ? old?.readAt ?? new Date().toISOString() : undefined, updated_at: Math.floor(Date.now() / 1000) }));
  }));
}
export async function createList(name: string) {
  return localAction(async () => {
    const id = device.newListId();
    await device.writeBatch([{ family: 'lists', key: id, id, name, updated_at: Math.floor(Date.now() / 1000) }]); return id;
  });
}
export async function previewListName(id: string, assertCurrent = () => {}) { assertCurrent(); await openLibrary(); assertCurrent(); const { previewDeviceListName } = await import('./device-list-name'); assertCurrent(); return previewDeviceListName(device, id, assertCurrent); }
export async function renameList(review: DeviceListNameReview, name: string, assertCurrent = () => {}) { return localAction(async () => { const { renameDeviceList } = await import('./device-list-name'); assertCurrent(); return renameDeviceList(device, review, name, assertCurrent); }); }
export async function saveListMember(list: string, series: string, position: number) {
  const row = normalizeDevice({ family: 'members', key: list + ':' + series, list, series, position, updated_at: Math.floor(Date.now() / 1000) });
  return localAction(() => device.mutate(async rows => {
    if (!(await request(rows.get(['lists', list])))) throw Error('Choose an existing local list');
    if (!(await request(rows.get(['series', series])))) throw Error('Choose an existing scope-bound saved series');
    rows.put(row);
  }));
}
export async function removeLocalRecord(family: Family, key: string) { return localAction(() => device.remove(family, key)); }
export const encodeParams = (input: URLSearchParams) => [...input.entries()].map(([key, value]) => encodeURIComponent(key) + '=' + encodeURIComponent(value)).join('&');
export const params = (input: URLSearchParams) => new URLSearchParams(input.toString().replace(/^\?/, ''));
export async function apiRead<T>(path: string, query?: URLSearchParams, account = false, expectedAccount?: string): Promise<T> {
  if (/^\/(?:manga|chapter|cover|author|group|tag)(?:\/|$)|^\/user\/(?:history|follows)(?:\/|$)/.test(path)) assertCataloguePreviewLibraryReady();
  if (account) assertAccountLibraryReady();
  if (publicUuidLibraryEnabled && /^\/(?:manga|chapter|cover|author|group|tag)(?:\/|$)|^\/user\/(?:history|follows)(?:\/|$)/.test(path)) throw new HTTPError(501,'This catalogue read requires its UUID adapter. No native request was sent.');
  const encoded = query ? encodeParams(params(query)) : '';
  const suffix = encoded ? '?' + encoded : '';
  const expectedId = expectedAccount ?? (account ? get(Session)?.identity.id : undefined);
  if (account && !expectedId) throw new HTTPError(401, 'No selected account identity');
  const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + path + suffix, { headers: expectedId ? { 'X-WeebDex-Expected-Account': expectedId } : {}, credentials: account ? 'include' : 'omit', cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new HTTPError(response.status, `Catalogue request failed: HTTP ${response.status}`);
  return response.json();
}
export async function accountRequest<T>(path: string, method: string, payload?: unknown, expectedAccountId?: string, assertContext: () => void = () => {}): Promise<{ status: number; body?: T }> {
  assertCataloguePreviewLibraryReady();
  assertAccountLibraryReady();
  const expectedId = expectedAccountId ?? get(Session)?.identity.id;
  const context = captureLibraryContext(), sessionGeneration = sessionWriteGeneration;
  const assertCurrentAccount = () => {
    assertLibraryContext(context); assertContext();
    if (!expectedId || get(Session)?.identity.id !== expectedId || sessionGeneration !== sessionWriteGeneration) throw new HTTPError(409, 'The selected account or newer Session state changed. An old account response was discarded; reload the current account before another action.', 'Account reconciliation context changed');
  };
  if (!(await Session.isValid())) throw new HTTPError(401, 'Sign into the selected account library');
  if (!expectedId) throw new HTTPError(401, 'No selected account identity');
  assertCurrentAccount();
  try {
    const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + path, { method, credentials: 'include', redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(30000), headers: payload === undefined ? { 'X-WeebDex-Expected-Account': expectedId } : { 'Content-Type': 'application/json', 'X-WeebDex-Expected-Account': expectedId }, body: payload === undefined ? undefined : JSON.stringify(payload) });
    if (!response.ok) { let body: T | undefined; try { body = await response.json(); } catch {} assertCurrentAccount(); return { status: response.status, body }; }
    const body = response.status === 204 ? undefined : await response.json();
    assertCurrentAccount();
    if (method !== 'GET') {
      const confirmed = await authoritativeAccount(expectedId);
      // No await between the final generation/account fence and Session write.
      // Caller-only guards are too late: this helper owns the store side effect.
      assertCurrentAccount();
      Session.set(confirmed);
      localLibraryState.update(state => ({ ...state, accountRevision: state.accountRevision + 1 }));
    }
    return { status: response.status, body };
  } catch (error) {
    if (method === 'GET') throw error;
    throw new HTTPError(0, `The account request may have succeeded, but its outcome could not be reconciled: ${message(error)}. Reload and check the selected account before submitting again. This request was not replayed.`, 'Request outcome unknown');
  }
}
export async function authoritativeAccount(expectedId: string): Promise<CheckSessionResponse> {
  const current = await apiRead<CheckSessionResponse>('/auth/check', undefined, true, expectedId);
  if (current.identity?.id !== expectedId) throw new HTTPError(409, 'The authenticated browser account changed. Review the account source again.', 'Account changed');
  return current;
}
export async function isAccountMode() { await openLibrary(); return get(localLibraryState).mode === 'account'; }
export type ImportReview = { snapshot: DeviceSnapshot; expectedRevision: number; source: string; collisions?: number; accountId?: string; foreignOwner?: boolean };
export type DeviceBackupRestoreReview = Readonly<{ mode: 'merge' | 'replace'; summary: MigrationReview['summary']; backup: MigrationReview['backup'] }>;
const deviceBackupPreviews = new WeakMap<ImportReview, { device: DeviceStore; ownerId: string | null }>();
const deviceBackupRestores = new WeakMap<DeviceBackupRestoreReview, { device: DeviceStore; review: MigrationReview; assertCurrent: () => void }>();
export async function previewImport(file: File): Promise<ImportReview> {
  if (file.size > 256 * 1024 * 1024) throw Error('Backup exceeds the 256 MiB import safety limit');
  const current = await currentDeviceSnapshot();
  const snapshot = validateSnapshot(JSON.parse(await file.text()));
  const keys = new Set(current.records.map(row => row.family + ':' + row.key));
  const result: ImportReview = { snapshot, expectedRevision: current.revision, source: file.name, collisions: snapshot.records.filter(row => keys.has(row.family + ':' + row.key)).length };
  if (snapshot.version === 2) {
    const { freezeLibrary } = await import('../public-api/device-id-state');
    result.foreignOwner = current.version !== 2 || snapshot.ownerId !== current.ownerId;
    freezeLibrary(result);
    deviceBackupPreviews.set(result, { device, ownerId: current.version === 2 ? current.ownerId : null });
  }
  return result;
}
// Choosing a restore mode prepares a frozen exact-state review. A separate
// confirmation commits only that review; no signer, relay or account is involved.
export async function prepareDeviceBackupImport(review: ImportReview, mode: 'merge' | 'replace', admitForeignOwner: boolean, assertCurrent: () => void): Promise<DeviceBackupRestoreReview> {
  const preview = deviceBackupPreviews.get(review);
  if (!preview || preview.device !== device || review.snapshot.version !== 2) throw Error('Preview this device backup before reviewing a restore.');
  if (!preview.ownerId) throw Error('Verify this browser’s catalogue scope, then preview the backup again.');
  const current = () => { assertCurrent(); if (preview.device !== device) throw Error('The device library changed. Preview the backup again.'); };
  current();
  const prepared = await device.prepareRestore(review.snapshot, { expectedRevision: review.expectedRevision, ownerId: preview.ownerId, mode, admitForeignOwner, assertCurrent: current });
  current();
  const result = Object.freeze({ mode, summary: prepared.summary, backup: prepared.backup });
  deviceBackupRestores.set(result, { device, review: prepared, assertCurrent: current }); return result;
}
export async function applyDeviceBackupImport(review: DeviceBackupRestoreReview, assertCurrent: () => void, watchCurrent?: (changed: () => void) => () => void) {
  const prepared = deviceBackupRestores.get(review);
  if (!prepared || prepared.device !== device) throw Error('Prepare and confirm a fresh device-backup restore review.');
  deviceBackupRestores.delete(review);
  return localAction(() => prepared.device.restore(prepared.review, () => { prepared.assertCurrent(); assertCurrent(); }, watchCurrent));
}
export async function applyImport(review: ImportReview, mode: 'merge' | 'replace', assertContext?: (scope: InstanceScope) => void) {
  if (review.accountId) {
    if (get(Session)?.identity.id !== review.accountId) throw Error('Account identity changed. Review the account import again.');
    await authoritativeAccount(review.accountId);
  }
  return localAction(() => device.import(review.snapshot, review.expectedRevision, mode, review.accountId || review.source === 'Reviewed catalogue import' || assertContext ? scope => { if (review.accountId || review.source === 'Reviewed catalogue import') assertCurrentCatalogueScopeValue(scope); assertContext?.(scope); } : undefined));
}
// Reviewed CSV/imported rows merge into the existing owner-bound library;
// they are not fabricated v1/v2 backups and cannot replace migration sidecars.
export async function applyCatalogueRecords(records:DeviceRecord[],expectedRevision:number,scope:InstanceScope,assertCurrent:()=>void){
  const checked=records.map(normalizeDevice);
  return localAction(()=>device.mutate(async(rows,meta)=>{
    assertCurrent();const current=await request(meta.get('instanceScope'));assertCurrent();
    if(!sameScope(scope,current))throw new HTTPError(409,'Catalogue scope changed. Review the import again.');
    assertCurrentCatalogueScopeValue(current);
    for(const row of checked){const old=await request(rows.get([row.family,row.key]));assertCurrent();if(!old||row.updated_at>old.updated_at)rows.put(row);}
    assertCurrent();
  },expectedRevision));
}
export async function exportBackup() {
  const snapshot = await currentDeviceSnapshot();
  return new Blob([JSON.stringify(snapshot)], { type: 'application/json' });
}
export async function exportDeviceCsv() {
  const snapshot = await currentDeviceSnapshot(), quote = (value: unknown) => '"' + String(value ?? '').replace(/"/g, '""') + '"';
  const lines = [['weebdex_id', 'title', 'reading_status', 'catalogue_origin', 'catalogue_namespace', 'service_key'], ...snapshot.records.filter(row => row.family === 'series').map(row => [row.id, row.id, row.status, snapshot.scope.origin, snapshot.scope.catalogNamespace ?? '', snapshot.scope.serviceKey ?? ''])];
  return new Blob([lines.map(row => row.map(quote).join(',')).join('\r\n') + '\r\n'], { type: 'text/csv;charset=utf-8' });
}
export async function parseDeviceCsv(csv: string, parser: { parse: (text: string, options: { complete: (results: { data: string[][]; errors?: unknown[] }) => void; error: (error: unknown) => void }) => void }) {
  if (csv.length > 16 * 1024 * 1024) throw Error('CSV exceeds the 16 MiB import limit; split it into reviewed batches');
  const current = await currentDeviceSnapshot();
  return new Promise<{ id: string; status: string }[]>((resolve, reject) => parser.parse(csv, { error: reject, complete: results => { try {
    if (results.errors?.length) throw Error('Invalid CSV'); const rows = results.data, header = rows[0] ?? [];
    const columns = ['catalogue_origin', 'catalogue_namespace', 'service_key'].map(name => header.indexOf(name));
    if (columns.some(index => index >= 0)) {
      if (columns.some(index => index < 0)) throw Error('Incomplete CSV catalogue scope');
      for (const row of rows.slice(1)) if (row.some(value => value?.trim())) {
        const scope = { origin: row[columns[0]], catalogNamespace: row[columns[1]] || null, serviceKey: row[columns[2]] || null };
        if (!sameScope(scope, current.scope)) throw Error('This scoped CSV belongs to a different catalogue. Native IDs will not be reinterpreted. Restore on its source instance.');
      }
    }
    const records = rows.slice(1).filter(row => row.some(value => value?.trim())).map(row => ({ id: catalogueId(row[0]), status: row[2] }));
    if (records.some(row => !['reading','on_hold','plan_to_read','dropped','re_reading','completed'].includes(row.status))) throw Error('Invalid reading status in CSV');
    resolve(records);
  } catch (error) { reject(error); } } }));
}
export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob), anchor = document.createElement('a');
  anchor.href = url; anchor.download = filename; anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 10000);
}
export async function previewAccountImport(): Promise<ImportReview> {
  await assertCatalogueScope();
  if (!(await Session.isValid())) throw Error('Sign into the account you want to import');
  const accountId = get(Session)?.identity.id;
  if (!accountId) throw Error('No authenticated account');
  const checkIdentity = async () => { if (get(Session)?.identity.id !== accountId) throw Error('Account identity changed during import. Review again.'); await authoritativeAccount(accountId); };
  await checkIdentity();
  const current = await currentDeviceSnapshot(), records: DeviceRecord[] = [];
  const maxRecords = 2000000;
  let page = 1, expectedFollowTotal: number | undefined, expectedHistoryTotal: number | undefined;
  for (;;) {
    await checkIdentity();
    const result = await apiRead<MangaFollowFeedResponse>('/user/follows/manga', new URLSearchParams({ page: String(page), limit: '100' }), true, accountId);
    await checkIdentity();
    if (expectedFollowTotal !== undefined && result.total !== expectedFollowTotal) throw Error('Account follows changed during import. Review again.'); expectedFollowTotal = result.total;
    if (!Number.isSafeInteger(result.total) || result.total < 0 || result.total > maxRecords || !Array.isArray(result.data) || result.data.length > 100) throw Error('Invalid or over-budget account import page');
    if (records.length + (result.data?.length ?? 0) > maxRecords) throw Error('Account import record limit');
    for (const row of result.data ?? []) records.push(normalizeDevice({ family: 'series', key: row.id, id: row.id, status: row.reading_status, notification: false, followedAt: row.followed_at, updated_at: Math.floor(Date.parse(row.followed_at) / 1000) }));
    if (page * 100 >= result.total) break;
    if (!(result.data?.length)) throw Error('Account import returned an incomplete page');
    page++;
  }
  page = 1;
  for (;;) {
    await checkIdentity();
    const result = await apiRead<MappedChapterHistoryFeedResponse>('/user/history/manga/feed', new URLSearchParams({ page: String(page), limit: '100' }), true, accountId);
    await checkIdentity();
    if (expectedHistoryTotal !== undefined && result.total !== expectedHistoryTotal) throw Error('Account history changed during import. Review again.'); expectedHistoryTotal = result.total;
    if (!Number.isSafeInteger(result.total) || result.total < 0 || result.total > maxRecords || !Array.isArray(result.data) || result.data.length > 100) throw Error('Invalid or over-budget account import page');
    if (records.length + (result.data?.length ?? 0) > maxRecords) throw Error('Account import record limit');
    for (const row of result.data ?? []) records.push(normalizeDevice({ family: 'chapters', key: row.id, id: row.id, mangaId: row.relationships.manga.id, completed: true, page: null, readAt: row.read_at, updated_at: Math.floor(Date.parse(row.read_at) / 1000) }));
    if (page * 100 >= result.total) break;
    if (!(result.data?.length)) throw Error('Account history import returned an incomplete page');
    page++;
  }
  await checkIdentity();
  const existing = new Set(current.records.map(row => row.family + ':' + row.key));
  return { accountId, snapshot: validateSnapshot({ scope: current.scope, format: 'weebdex-device-library', version: 1, revision: 0, exported_at: new Date().toISOString(), records }), expectedRevision: current.revision, collisions: records.filter(row => existing.has(row.family + ':' + row.key)).length, source: 'Explicit account-library import' };
}

export function applyAccountFollow(context: ReturnType<typeof captureLibraryContext>, id: string, value: [string, number] | undefined) {
  assertLibraryContext(context);
  if (context.mode !== 'account') return;
  const account = get(Session); if (!account || account.identity.id !== context.accountId) throw new HTTPError(409, 'The account changed; no follow flag was applied to another account', 'Account changed');
  const follows = { ...(account.follows ?? {}) };
  if (value) follows[id] = value as typeof follows[string]; else delete follows[id];
  Session.set({ ...account, follows });
}

// Bounded reader seam: no snapshot(), getAll(), copied history, or write authority.
// The caller must recheck this identity before and after its owned readonly transaction.
export function readerLibrarySource() {
  const state = get(localLibraryState);
  return { context: captureLibraryContext(), admission: captureLibraryAdmission(), selectionPending: get(readerSelectionState).pending, ready: state.ready, mode: state.mode, revision: state.revision, accountRevision: state.accountRevision,
    scopeStatus: state.scopeStatus, reason: state.error ?? state.scopeReason,
    observedCatalogue: readerObservedCatalogue && { ...readerObservedCatalogue, scope: { ...readerObservedCatalogue.scope } },
    scope: verifiedCatalogueScope && { ...verifiedCatalogueScope }, db: state.ready && device ? device.db : undefined };
}
