// Only explicit UI actions may invoke signer/publication operations.
import { writable, get } from 'svelte/store';
import { PUBLIC_API_URL } from '$env/static/public';
import { stores, currentDeviceSnapshot, apiRead, refreshLibrary, verifyCatalogueScope, message } from './library';
import { scopedCatalogCandidates } from './catalog';
import { deviceWriteLock, type DeviceSnapshot } from './device-store';
import { buildGraph, normalize, recordKey, families } from './protocol/codec.js';
import { proveService, origin, signerRequest, signedRequest, signExact, verifyEvent } from './protocol/nostr.js';
import { queueSnapshot, deliverJob, cancelJob, recoverPublic } from './protocol/publishing.js';
import { backupBlob } from './protocol/backup.js';
import { verifyLibraryRelay } from './library-relay-support';
import { LocalLibrary } from './protocol/store.js';
import { createScopedProtocolAccess } from './protocol-scope';
import { publicResourceMappings } from '../public-api/resource-mappings';
import { publicSourceReferences, eligiblePublicMappings } from './public-device-projection';
import type { NewJob } from './protocol/types.d.ts';
export type Relay = { origin: string; serviceKey: string; selected: boolean };
export type Capabilities = { catalog_namespace: string | null; available: boolean; schema_ready: boolean; origin: string | null; service_key: string | null; connect_origins: string[]; external_signer_required: boolean; account_admission_required: true };
export type SourceMapping = { kind: 'manga' | 'chapter'; id: string; event_id: string; compatibility_id: string; publisher?: string; source_kind?: number; canonical_id?: string; parent?: { id: string; event_id: string } | null };
export type NIP07Signer = { getPublicKey(): Promise<string>; signEvent(event: unknown): Promise<unknown> };
export type LocalSessionSigner = NIP07Signer & { clear(): void };
export const signerIdentity = writable<{ owner: string | null; generation: number; source: 'local' | 'external' | 'remote' | null }>({ owner: null, generation: 0, source: null });
let signer: NIP07Signer | LocalSessionSigner | null = null;
let clearLocalSigner: (() => void) | null = null;
const protocolScopes = createScopedProtocolAccess({
  selection: () => ({ ...get(signerIdentity), origin: new URL(PUBLIC_API_URL || '/api', window.location.origin).origin }),
  assertCurrent: selected => { const current = get(signerIdentity); if (current.owner !== selected.owner || current.generation !== selected.generation) throw Error('Canonical library identity changed. Review this action again.'); },
  open: name => LocalLibrary.open(name)
});
async function publicationStores(requireOwner = true) {
  const selected = await protocolScopes(requireOwner), { device } = await stores(); selected.check(); return { ...selected, device };
}

function clearSelectedSigner() {
  const clear = clearLocalSigner; signer = null; clearLocalSigner = null;
  clear?.();
}
// The draft owns its key until the UI explicitly activates it after backup and
// public-key review. Only this public identity enters the existing Svelte store.
export function installLocalSigner(owner: string, candidate: LocalSessionSigner, assertCurrent: () => void) {
  assertCurrent();
  if (!/^[0-9a-f]{64}$/.test(owner)) throw Error('Invalid local signing public key');
  clearSelectedSigner();
  const generation = get(signerIdentity).generation + 1;
  signer = candidate; clearLocalSigner = () => candidate.clear();
  signerIdentity.set({ owner, generation, source: 'local' });
}
export type RemoteSessionSigner = LocalSessionSigner & { onClose(listener: () => void): () => void };
export function installRemoteSigner(owner: string, candidate: RemoteSessionSigner, assertCurrent: () => void) {
  assertCurrent(); if (!/^[0-9a-f]{64}$/.test(owner)) throw Error('Invalid remote signing public key');
  clearSelectedSigner(); const generation = get(signerIdentity).generation + 1;
  signer = candidate; clearLocalSigner = () => candidate.clear();
  signerIdentity.set({ owner, generation, source: 'remote' });
  candidate.onClose(() => { if (signer === candidate) disconnectSigner(); });
}
export async function connectSigner(assertCurrent = () => {}, expectedOwner?: string) {
  assertCurrent();
  const candidate = (window as Window & { nostr?: NIP07Signer }).nostr;
  if (!candidate?.getPublicKey || !candidate.signEvent) throw Error('No compatible NIP-07 browser signer was found. Create or import a key in Public identity settings, or install a signing extension.');
  const generation = get(signerIdentity).generation + 1;
  clearSelectedSigner(); signerIdentity.set({ owner: null, generation, source: null });
  const owner = await signerRequest(() => { assertCurrent(); return candidate.getPublicKey(); });
  assertCurrent();
  if (!/^[0-9a-f]{64}$/.test(owner)) throw Error('Signer returned an invalid public key');
  if (expectedOwner && owner !== expectedOwner) throw Error('Your signer selected a different identity. Choose the identity connected to this account in your signer and try again. Nothing was linked or published.');
  if (get(signerIdentity).generation !== generation) throw Error('Signer selection changed');
  signer = candidate; signerIdentity.set({ owner, generation, source: 'external' });
}
export function disconnectSigner() { clearSelectedSigner(); signerIdentity.update(state => ({ owner: null, generation: state.generation + 1, source: null })); }
export function selectedSigner() {
  const identity = get(signerIdentity), selected = signer;
  if (!selected || !identity.owner) throw Error('Create, import or connect the intended signing identity first');
  const check = () => {
    const current = get(signerIdentity);
    if (signer !== selected || current.owner !== identity.owner || current.generation !== identity.generation) throw Error('Signer identity changed. Review this action again.');
  };
  return { owner: identity.owner, generation: identity.generation, signer: { signEvent: async (template: unknown) => { check(); const result = await selected.signEvent(template); check(); return result; } }, check };
}
function activeSigner(selected: { signEvent(event: unknown): Promise<unknown> }, assertCurrent: () => void) {
  return { signEvent: async (template: unknown) => { assertCurrent(); const result = await selected.signEvent(template); assertCurrent(); return result; } };
}
export async function capabilities(): Promise<Capabilities> {
  const result = await apiRead<Capabilities>('/protocol/capabilities');
  if (!Array.isArray(result.connect_origins)) throw Error('Instance publication policy is unavailable');
  return result;
}
export function assertAllowedRelay(relayOrigin: string, policy: Capabilities) {
  const canonical = origin(relayOrigin);
  if (canonical !== window.location.origin && !policy.connect_origins.includes(canonical)) throw Error('This origin is not allowed by the instance relay/CSP policy. Ask the instance operator to allow it before publishing.');
  return canonical;
}
export async function relayPreferences() {
  const { protocol } = await publicationStores(false), preferences = await protocol.relayPreferences();
  return { ...preferences, relays: preferences.relays.map(row => ({ ...row, selected: row.selected === true })) };
}
export async function rememberRelay(input: string, key: string, expectedRevision: number, assertCurrent = () => {}) {
  assertCurrent();
  const { protocol, check } = await publicationStores(false);
  const policy = await capabilities(), canonical = assertAllowedRelay(input, policy);
  if (key && !/^[0-9a-f]{64}$/.test(key)) throw Error('Invalid expected service public key');
  const active = () => { check(); assertCurrent(); };
  const verified = await proveService(canonical, key || undefined); active(); await verifyLibraryRelay(verified, active); active();
  const current = await protocol.relayPreferences(); active();
  if (current.revision !== expectedRevision) throw Error('Instance selection changed. Review it again.');
  const next = [...current.relays.filter(row => row.origin !== canonical), { ...verified, selected: false }];
  active(); await protocol.setRelays(next, expectedRevision); active();
  return verified;
}
export async function selectRelays(relays: Relay[], expectedRevision: number) {
  const { protocol, check } = await publicationStores(false);
  const policy = await capabilities();
  for (const relay of relays) { assertAllowedRelay(relay.origin, policy); if (relay.selected) { await proveService(relay.origin, relay.serviceKey); check(); await verifyLibraryRelay(relay, check); } }
  return protocol.setRelays(relays, expectedRevision);
}
export async function forgetRelay(relayOrigin: string, expectedRevision: number) {
  const { protocol } = await publicationStores(false), current = await protocol.relayPreferences();
  if (current.revision !== expectedRevision) throw Error('Instance selection changed. Review again.');
  return protocol.setRelays(current.relays.filter(row => row.origin !== relayOrigin), expectedRevision);
}
export async function admitAccount(relay: Relay, assertCurrent = () => {}) {
  assertCurrent();
  const policy = await capabilities(); assertAllowedRelay(relay.origin, policy);
  const identity = selectedSigner(); await proveService(relay.origin, relay.serviceKey); identity.check();
  assertCurrent();
  const result = await signedRequest(relay, activeSigner(identity.signer, assertCurrent), identity.owner, '/enrollment', 'POST', {}, 65536, async () => { identity.check(); assertCurrent(); });
  if (![200, 201].includes(result.status)) throw Error(`Instance refused public-key account admission: HTTP ${result.status}`);
  return result;
}
async function resourceMappings(snapshot: DeviceSnapshot) {
  if (snapshot.migration_reason) throw Error(snapshot.migration_reason);
  const requests = new Map<string, { kind: 'manga' | 'chapter'; id: string }>();
  if (snapshot.version === 2) for (const ref of publicSourceReferences(snapshot.records)) requests.set(ref.kind + ':' + ref.id, ref);
  else {
  for (const row of snapshot.records) {
    if (row.family === 'series') requests.set('manga:' + row.id, { kind: 'manga', id: row.id! });
    if (row.family === 'chapters') requests.set('chapter:' + row.id, { kind: 'chapter', id: row.id! });
    if (row.family === 'members') requests.set('manga:' + row.series, { kind: 'manga', id: row.series! });
  }
  }
  const policy = await capabilities();
  if (!policy.available || !policy.origin || !policy.service_key) throw Error('This instance has no available verified protocol bridge');
  if (!snapshot.scope.catalogNamespace || snapshot.scope.catalogNamespace !== policy.catalog_namespace) throw Error('This device catalogue namespace is unadopted or changed. Review the source instance before publishing.');
  if (!snapshot.scope.serviceKey) throw Error('This origin-only device library needs explicit service-identity adoption before protocol publication');
  if (snapshot.scope.serviceKey !== policy.service_key) throw Error('Device catalogue scope no longer matches the configured protocol service. Migration is required.');
  const maps = new Map<string, SourceMapping>(), resources = [...requests.values()];
  if (snapshot.version === 2) {
    const publicOrigin = new URL(PUBLIC_API_URL || '/api', window.location.origin).origin;
    if (publicOrigin !== snapshot.scope.origin) throw Error('Public UUID mapping belongs to a different saved instance origin');
    const scope = { publicOrigin, sourceOrigin: policy.origin, serviceKey: policy.service_key, catalogNamespace: snapshot.scope.catalogNamespace };
    const mapped = [];
    for (let start = 0; start < resources.length; start += 256) mapped.push(...(await publicResourceMappings(scope, resources.slice(start, start + 256))).mappings);
    for (const row of eligiblePublicMappings(snapshot.records, mapped)) maps.set(row.kind + ':' + row.id, row);
    return { maps, policy };
  }
  for (let start = 0; start < resources.length; start += 256) {
    // Native resource IDs are sent only to their own scoped API.
    const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + '/protocol/resources', { method: 'POST', credentials: 'omit', redirect: 'error', cache: 'no-store', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ resources: resources.slice(start, start + 256) }), signal: AbortSignal.timeout(30000) });
    if (!response.ok) throw Error(`Canonical mapping failed: HTTP ${response.status}`);
    const result = await response.json();
    if (result.catalog_namespace !== policy.catalog_namespace || result.origin !== policy.origin || result.service_key !== policy.service_key || !Array.isArray(result.data)) throw Error('Canonical mapping came from a different protocol service');
    for (const mapping of result.data) {
      const key = mapping.kind + ':' + mapping.id;
      if (!requests.has(key) || !/^[0-9a-f]{64}$/.test(mapping.event_id)) throw Error('Invalid canonical source mapping');
      if (maps.has(key)) throw Error('Duplicate canonical source mapping');
      maps.set(key, mapping);
    }
  }
  return { maps, policy };
}
function graphWorker(data: unknown): Promise<any> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL('./projection-worker.ts', import.meta.url), { type: 'module' });
    const timer = setTimeout(() => { worker.terminate(); reject(Error('Snapshot preparation timed out. Nothing was signed.')); }, 120000);
    worker.onmessage = ({ data }) => { clearTimeout(timer); worker.terminate(); if (data.error) reject(Error(data.error)); else resolve(data); };
    worker.onerror = event => { clearTimeout(timer); worker.terminate(); reject(Error(event.message)); };
    worker.postMessage(data);
  });
}
export type PublicationReview = { mappingProof: string; deviceRevision: number; protocolRevision: number; relayRevision: number; owner: string; generation: number; recipients: Relay[]; graph: ReturnType<typeof buildGraph>; excluded: string[]; sourceOrigin: string; serviceKey: string };
export async function previewPublication(): Promise<PublicationReview> {
  const identity = selectedSigner(), snapshot = await currentDeviceSnapshot(), { protocol, device } = await publicationStores();
  const deviceState = await device.publicationState(); if (deviceState.migration_reason) throw Error(deviceState.migration_reason);
  const preferences = await protocol.relayPreferences(), recipients = preferences.relays.filter(row => row.selected === true).map(row => ({ ...row, selected: true }));
  if (!recipients.length) throw Error('Choose at least one verified publication instance');
  const { maps, policy } = await resourceMappings(snapshot);
  for (const relay of recipients) { assertAllowedRelay(relay.origin, policy); await proveService(relay.origin, relay.serviceKey); identity.check(); await verifyLibraryRelay(relay, identity.check); }
  const prepared = await graphWorker({ protocolDatabase: protocol.db.name, deviceDatabase: device.db.name, mappings: [...maps.values()], deviceRevision: snapshot.revision, deviceScope: snapshot.scope, deviceOwner: snapshot.version === 2 ? snapshot.ownerId : null, deviceVersion: snapshot.version });
  identity.check();
  if (prepared.deviceRevision !== snapshot.revision || (await currentDeviceSnapshot()).revision !== snapshot.revision) throw Error('Device library changed during preview. Review again.');
  return { deviceRevision: snapshot.revision, protocolRevision: prepared.protocolRevision, mappingProof: prepared.mappingProof, relayRevision: preferences.revision, owner: identity.owner, generation: identity.generation, recipients, graph: prepared.graph, excluded: prepared.excluded, sourceOrigin: policy.origin!, serviceKey: policy.service_key! };
}
export async function publishReviewed(review: PublicationReview, publicConsent: boolean, assertCurrent = () => {}) {
  assertCurrent();
  if (publicConsent !== true) throw Error('Explicit public-publication consent is required');
  if (!navigator.locks) throw Error('This browser needs Web Locks for safe publication');
  const identity = selectedSigner(), { device, protocol, completionStore } = await publicationStores(); identity.check();
  if (review.owner !== identity.owner || review.generation !== identity.generation) throw Error('Signer changed since preview');
  const deviceState = await device.publicationState(); if (deviceState.migration_reason) throw Error(deviceState.migration_reason);
  const current = await currentDeviceSnapshot(), fresh = await resourceMappings(current);
  if (fresh.policy.origin !== review.sourceOrigin || fresh.policy.service_key !== review.serviceKey) throw Error('Source instance changed since preview');
  // Recheck current source mappings in a worker without building a second huge graph.
  const fingerprint = await graphWorker({ mappings: [...fresh.maps.values()], fingerprintOnly: true });
  const preferences = await protocol.relayPreferences(), canonical = await protocol.counts();
  for (const relay of review.recipients) { assertAllowedRelay(relay.origin, fresh.policy); await proveService(relay.origin, relay.serviceKey); identity.check(); assertCurrent(); await verifyLibraryRelay(relay, () => { identity.check(); assertCurrent(); }); }
  if (current.revision !== review.deviceRevision || canonical.revision !== review.protocolRevision || preferences.revision !== review.relayRevision || fingerprint.mappingProof !== review.mappingProof) throw Error('Library, canonical mapping or recipient selection changed. Review again.');
  // Keep device projections out of the canonical recovery database. Otherwise a
  // later native unfollow could accidentally survive as a stale canonical record.
  // The original codec builds the reviewed graph; original outbox owns its bytes.
  const guarded: typeof protocol = Object.create(protocol);
  guarded.createJob = (job: NewJob) => deviceWriteLock(async () => {
    identity.check();
    const latest = await device.publicationState();
    if (latest.migration_reason) throw Error(latest.migration_reason);
    if (latest.revision !== review.deviceRevision) throw Error('Device library changed while signing. Nothing was queued; review again.');
    return protocol.createJob({ ...job, deviceRevision: review.deviceRevision, sourceScope: current.scope });
  });
  // A dismissed settings view may not start a new signature. If dismissal
  // happens inside the extension prompt, retain the exact signed/CAS-checked
  // job before checking UI lifetime again. No newly approved bytes are lost.
  const snapshotSigner = { signEvent: (template: unknown) => { assertCurrent(); return identity.signer.signEvent(template); } };
  const job = await queueSnapshot(guarded, snapshotSigner, identity.owner, { revision: review.protocolRevision, graph: review.graph }, review.recipients, true, review.relayRevision);
  assertCurrent();
  await deliverJob(protocol, job.id, activeSigner(identity.signer, assertCurrent), identity.owner, undefined, () => { identity.check(); assertCurrent(); }, completionStore(job.id));
  identity.check(); assertCurrent(); return job.id;
}
export async function publicationJobs() { if (!get(signerIdentity).owner) return []; const { protocol } = await publicationStores(); return protocol.jobs(); }
export async function retryPublication(id: string, assertCurrent = () => {}) {
  assertCurrent();
  const { protocol, completionStore } = await publicationStores(), identity = selectedSigner(), job = await protocol.job(id), policy = await capabilities();
  if (!job || job.event.pubkey !== identity.owner) throw Error('Connect the identity that signed this snapshot');
  for (const relay of job.recipients) { assertAllowedRelay(relay.origin, policy); await proveService(relay.origin, relay.serviceKey); identity.check(); assertCurrent(); await verifyLibraryRelay(relay, () => { identity.check(); assertCurrent(); }); }
  assertCurrent();
  await deliverJob(protocol, id, activeSigner(identity.signer, assertCurrent), identity.owner, undefined, () => { identity.check(); assertCurrent(); }, completionStore(id));
  identity.check(); assertCurrent();
}
export async function cancelPublication(id: string) { const { protocol } = await publicationStores(); return cancelJob(protocol, id); }
export async function removePublication(id: string) {
  const { protocol } = await publicationStores(), job = await protocol.job(id);
  if (job && !job.cancelled && job.recipients.some((relay: { state: string }) => !['accepted', 'superseded'].includes(relay.state))) throw Error('Cancel unfinished sending before removing this retry record');
  return protocol.deleteJob(id);
}
export type ServiceAdoptionReview = { expectedRevision: number; origin: string; serviceOrigin: string; serviceKey: string; catalogNamespace: string; existingRecords: number; identities: { kind: string; id: string; title: string; missing: boolean }[] };
export async function previewServiceAdoption(): Promise<ServiceAdoptionReview> {
  const snapshot = await currentDeviceSnapshot(), policy = await capabilities();
  if (!policy.origin || !policy.service_key || !policy.catalog_namespace) throw Error('This instance has no configured original source identity to adopt');
  if ((snapshot.scope.catalogNamespace && snapshot.scope.catalogNamespace !== policy.catalog_namespace) || (snapshot.scope.serviceKey && snapshot.scope.serviceKey !== policy.service_key)) throw Error('Known scope changed. This needs reviewed migration, not adoption.');
  assertAllowedRelay(policy.origin, policy); await proveService(policy.origin, policy.service_key);
  const mangaIds = new Set<string>(), chapterIds = new Set<string>();
  for (const row of snapshot.records) { if (row.family === 'series') mangaIds.add(row.id!); if (row.family === 'members') mangaIds.add(row.series!); if (row.family === 'chapters') { chapterIds.add(row.id!); mangaIds.add(row.mangaId!); } }
  const query = new URLSearchParams(); for (const value of ['safe', 'suggestive', 'erotica', 'pornographic']) query.append('contentRating', value);
  const publicRefs = snapshot.version === 2 ? publicSourceReferences(snapshot.records) : undefined;
  const manga = await scopedCatalogCandidates<Manga>('/manga', 'id', publicRefs ? publicRefs.filter(row => row.kind === 'manga').map(row => row.id) : [...mangaIds], query), chapters = await scopedCatalogCandidates<Chapter>('/chapter/feed', 'id', publicRefs ? publicRefs.filter(row => row.kind === 'chapter').map(row => row.id) : [...chapterIds], query);
  const titles = new Map(manga.map(row => [row.id, row.title])), headlines = new Map(chapters.map(row => [row.id, `${row.relationships.manga.title}: ${row.title ?? 'chapter ' + (row.chapter ?? row.id)}`]));
  const current = await capabilities();
  if (current.catalog_namespace !== policy.catalog_namespace || current.service_key !== policy.service_key || current.origin !== policy.origin || (await currentDeviceSnapshot()).revision !== snapshot.revision) throw Error('Device records or source scope changed during adoption preview. Review again.');
  return { expectedRevision: snapshot.revision, origin: snapshot.scope.origin, serviceOrigin: policy.origin, serviceKey: policy.service_key, catalogNamespace: policy.catalog_namespace, existingRecords: snapshot.records.length, identities: [...[...mangaIds].map(id => ({ kind: 'manga', id, title: titles.get(id) ?? 'not visible in the current catalogue', missing: !titles.has(id) })), ...[...chapterIds].map(id => ({ kind: 'chapter', id, title: headlines.get(id) ?? 'not visible in the current catalogue', missing: !headlines.has(id) }))] };
}
export async function adoptServiceIdentity(review: ServiceAdoptionReview) {
  const { device } = await stores(), policy = await capabilities();
  if (policy.origin !== review.serviceOrigin || policy.service_key !== review.serviceKey || policy.catalog_namespace !== review.catalogNamespace) throw Error('Current source identity changed since adoption preview');
  assertAllowedRelay(review.serviceOrigin, policy); await proveService(review.serviceOrigin, review.serviceKey);
  await device.pinServiceIdentity(review.serviceKey, review.catalogNamespace, review.expectedRevision); await refreshLibrary(); await verifyCatalogueScope();
}
export type RecoveryReview = { snapshot: unknown; event: { id: string; pubkey: string; created_at: number }; relay: Relay; protocolRevision: number; relayRevision: number; owner: string; generation: number };
export async function previewRecovery(relay: Relay, assertCurrent = () => {}): Promise<RecoveryReview> {
  assertCurrent();
  const policy = await capabilities(); assertAllowedRelay(relay.origin, policy);
  const identity = selectedSigner(), { protocol } = await publicationStores(), revision = (await protocol.counts()).revision, preferences = await protocol.relayPreferences();
  await proveService(relay.origin, relay.serviceKey); identity.check(); assertCurrent(); await verifyLibraryRelay(relay, () => { identity.check(); assertCurrent(); });
  const result = await recoverPublic(relay, activeSigner(identity.signer, assertCurrent), identity.owner); identity.check(); assertCurrent();
  return { ...result, relay, protocolRevision: revision, relayRevision: preferences.revision, owner: identity.owner, generation: identity.generation };
}
export async function applyRecovery(review: RecoveryReview) {
  const identity = selectedSigner(), { protocol } = await publicationStores(); identity.check();
  if (review.event.pubkey !== identity.owner) throw Error('Recovered event belongs to a different canonical key');
  if (identity.owner !== review.owner || identity.generation !== review.generation) throw Error('Identity changed since recovery preview');
  return deviceWriteLock(() => { identity.check(); return protocol.replace(review.snapshot, review.protocolRevision, { owner: review.event.pubkey, id: review.event.id, created_at: review.event.created_at }, review.relayRevision); });
}
// Legacy data is available only through an explicitly labelled local export.
export async function exportLegacyCanonicalBackup() { const { protocol } = await stores(); return backupBlob(protocol); }
export async function exportCanonicalBackup() { const { protocol, check } = await publicationStores(); const blob = await backupBlob(protocol); check(); return blob; }
export type CanonicalSelection = { owner: string; generation: number };
export async function canonicalRecords(family: string, after: string | null = null) { const { protocol, owner, generation } = await publicationStores(); return { owner: owner!, generation, counts: await protocol.counts(), records: await protocol.list(family, after, 50) }; }
export async function removeCanonicalRecord(family: string, key: string, selected: CanonicalSelection) { const identity = selectedSigner(); if (selected.owner !== identity.owner || selected.generation !== identity.generation) throw Error('Canonical record belongs to a different reviewed identity'); const { protocol } = await publicationStores(); identity.check(); await protocol.remove(family, key); }
export type CanonicalImportReview = { owner: string; generation: number; file: File; expectedRevision: number; counts: Record<string, number>; records: number };
export async function previewCanonicalImport(file: File): Promise<CanonicalImportReview> {
  const { protocol, owner, generation, check } = await publicationStores(), expectedRevision = (await protocol.counts()).revision;
  if (file.size > 4 * 1024 ** 3) throw Error('Canonical backup exceeds the original 4 GiB safety limit');
  const counts: Record<string, number> = Object.fromEntries(families.map((family: string) => [family, 0]));
  const seen = new Set<string>(); let records = 0;
  const add = (family: string, input: unknown) => {
    const row = normalize(family, input), key = family + ':' + recordKey(family, row);
    if (seen.has(key)) throw Error('Duplicate canonical backup record');
    seen.add(key); counts[family]++; records++;
    if (records > 2000000) throw Error('Canonical backup record limit');
  };
  const beginning = await file.slice(0, 4096).text();
  let first: any; try { first = JSON.parse(beginning.split('\n')[0]); } catch {}
  if (first?.format !== 'manga-local-library' || first.version !== 2) {
    if (file.size > 256 * 1024 * 1024) throw Error('Legacy JSON backup exceeds preview memory limit; use the original streaming JSONL export');
    const snapshot = JSON.parse(await file.text());
    if (snapshot.format !== 'manga-local-library' || snapshot.version !== 1 || Object.keys(snapshot.records ?? {}).sort().join('|') !== [...families].sort().join('|')) throw Error('Unsupported canonical backup');
    for (const family of families) { if (!Array.isArray(snapshot.records[family])) throw Error('Invalid canonical backup family'); for (const row of snapshot.records[family]) add(family, row); }
  } else {
    let lineBuffer = '', header = false, ended = false;
    const decoder = new TextDecoder('utf-8', { fatal: true });
    for (let offset = 0; offset < file.size; offset += 65536) {
      lineBuffer += decoder.decode(await file.slice(offset, offset + 65536).arrayBuffer(), { stream: offset + 65536 < file.size });
      let newline: number;
      while ((newline = lineBuffer.indexOf('\n')) !== -1) {
        const line = lineBuffer.slice(0, newline); lineBuffer = lineBuffer.slice(newline + 1);
        if (new TextEncoder().encode(line).length + 1 > 65536) throw Error('Canonical backup line limit');
        const input = JSON.parse(line);
        if (!header) { if (input.format !== 'manga-local-library' || input.version !== 2) throw Error('Invalid canonical backup header'); header = true; }
        else if (ended) throw Error('Canonical backup has data after its footer');
        else if (Array.isArray(input) && input.length === 2) add(input[0], input[1]);
        else if (input.end === records) ended = true;
        else throw Error('Canonical backup count mismatch');
      }
      if (new TextEncoder().encode(lineBuffer).length >= 65536) throw Error('Canonical backup line limit');
    }
    if (lineBuffer.length || !header || !ended) throw Error('Truncated canonical backup');
  }
  check(); return { owner: owner!, generation, file, expectedRevision, counts, records };
}
export async function applyCanonicalImport(review: CanonicalImportReview) {
  const identity = selectedSigner(); if (identity.owner !== review.owner || identity.generation !== review.generation) throw Error('Canonical import identity changed since review');
  const { protocol } = await publicationStores(), { importBackup } = await import('./protocol/backup.js');
  // Original staged import and original revision CAS remain authoritative.
  identity.check(); await importBackup(protocol, review.file, review.expectedRevision); identity.check();
}
