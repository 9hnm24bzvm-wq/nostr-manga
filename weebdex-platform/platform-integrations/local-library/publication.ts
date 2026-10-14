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
import type { NewJob } from './protocol/types.d.ts';
export type Relay = { origin: string; serviceKey: string; selected: boolean };
export type Capabilities = { catalog_namespace: string | null; available: boolean; schema_ready: boolean; origin: string | null; service_key: string | null; connect_origins: string[]; external_signer_required: true; account_admission_required: true };
export type SourceMapping = { kind: 'manga' | 'chapter'; id: string; event_id: string; compatibility_id: string; publisher?: string; source_kind?: number };
export type NIP07Signer = { getPublicKey(): Promise<string>; signEvent(event: unknown): Promise<unknown> };
export const signerIdentity = writable<{ owner: string | null; generation: number }>({ owner: null, generation: 0 });
let signer: NIP07Signer | null = null;
export async function connectSigner(assertCurrent = () => {}) {
  assertCurrent();
  const candidate = (window as Window & { nostr?: NIP07Signer }).nostr;
  if (!candidate?.getPublicKey || !candidate.signEvent) throw Error('A compatible NIP-07 browser signer is required for publication. Device saving works without one.');
  const generation = get(signerIdentity).generation + 1;
  signerIdentity.set({ owner: null, generation }); signer = null;
  const owner = await signerRequest(() => { assertCurrent(); return candidate.getPublicKey(); });
  assertCurrent();
  if (!/^[0-9a-f]{64}$/.test(owner)) throw Error('Signer returned an invalid public key');
  if (get(signerIdentity).generation !== generation) throw Error('Signer selection changed');
  signer = candidate; signerIdentity.set({ owner, generation });
}
export function disconnectSigner() { signer = null; signerIdentity.update(state => ({ owner: null, generation: state.generation + 1 })); }
export function selectedSigner() {
  const identity = get(signerIdentity), selected = signer;
  if (!selected || !identity.owner) throw Error('Connect the intended NIP-07 signing identity first');
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
  const { protocol } = await stores(), preferences = await protocol.relayPreferences();
  return { ...preferences, relays: preferences.relays.map(row => ({ ...row, selected: row.selected === true })) };
}
export async function rememberRelay(input: string, key: string, expectedRevision: number) {
  const policy = await capabilities(), canonical = assertAllowedRelay(input, policy);
  if (key && !/^[0-9a-f]{64}$/.test(key)) throw Error('Invalid expected service public key');
  const verified = await proveService(canonical, key || undefined), { protocol } = await stores();
  const current = await protocol.relayPreferences();
  if (current.revision !== expectedRevision) throw Error('Instance selection changed. Review it again.');
  const next = [...current.relays.filter(row => row.origin !== canonical), { ...verified, selected: false }];
  await protocol.setRelays(next, expectedRevision);
  return verified;
}
export async function selectRelays(relays: Relay[], expectedRevision: number) {
  const policy = await capabilities();
  for (const relay of relays) assertAllowedRelay(relay.origin, policy);
  const { protocol } = await stores(); return protocol.setRelays(relays, expectedRevision);
}
export async function forgetRelay(relayOrigin: string, expectedRevision: number) {
  const { protocol } = await stores(), current = await protocol.relayPreferences();
  if (current.revision !== expectedRevision) throw Error('Instance selection changed. Review again.');
  return protocol.setRelays(current.relays.filter(row => row.origin !== relayOrigin), expectedRevision);
}
export async function admitAccount(relay: Relay, assertCurrent = () => {}) {
  assertCurrent();
  const policy = await capabilities(); assertAllowedRelay(relay.origin, policy);
  const identity = selectedSigner(); await proveService(relay.origin, relay.serviceKey); identity.check();
  assertCurrent();
  const result = await signedRequest(relay, activeSigner(identity.signer, assertCurrent), identity.owner, '/enrollment');
  if (![200, 201].includes(result.status)) throw Error(`Instance refused public-key account admission: HTTP ${result.status}`);
  return result;
}
async function resourceMappings(snapshot: DeviceSnapshot) {
  if (snapshot.migration_reason) throw Error(snapshot.migration_reason);
  const requests = new Map<string, { kind: 'manga' | 'chapter'; id: string }>();
  for (const row of snapshot.records) {
    if (row.family === 'series') requests.set('manga:' + row.id, { kind: 'manga', id: row.id! });
    if (row.family === 'chapters') requests.set('chapter:' + row.id, { kind: 'chapter', id: row.id! });
    if (row.family === 'members') requests.set('manga:' + row.series, { kind: 'manga', id: row.series! });
  }
  const policy = await capabilities();
  if (!policy.available || !policy.origin || !policy.service_key) throw Error('This instance has no available verified protocol bridge');
  if (!snapshot.scope.catalogNamespace || snapshot.scope.catalogNamespace !== policy.catalog_namespace) throw Error('This device catalogue namespace is unadopted or changed. Review the source instance before publishing.');
  if (!snapshot.scope.serviceKey) throw Error('This origin-only device library needs explicit service-identity adoption before protocol publication');
  if (snapshot.scope.serviceKey !== policy.service_key) throw Error('Device catalogue scope no longer matches the configured protocol service. Migration is required.');
  const maps = new Map<string, SourceMapping>(), resources = [...requests.values()];
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
  const identity = selectedSigner(), snapshot = await currentDeviceSnapshot(), { protocol } = await stores();
  const preferences = await protocol.relayPreferences(), recipients = preferences.relays.filter(row => row.selected === true).map(row => ({ ...row, selected: true }));
  if (!recipients.length) throw Error('Choose at least one verified publication instance');
  const { maps, policy } = await resourceMappings(snapshot);
  for (const relay of recipients) assertAllowedRelay(relay.origin, policy);
  const prepared = await graphWorker({ protocolDatabase: protocol.db.name, deviceDatabase: (await stores()).device.db.name, mappings: [...maps.values()] });
  identity.check();
  if ((await currentDeviceSnapshot()).revision !== snapshot.revision) throw Error('Device library changed during preview. Review again.');
  return { deviceRevision: snapshot.revision, protocolRevision: prepared.protocolRevision, mappingProof: prepared.mappingProof, relayRevision: preferences.revision, owner: identity.owner, generation: identity.generation, recipients, graph: prepared.graph, excluded: prepared.excluded, sourceOrigin: policy.origin!, serviceKey: policy.service_key! };
}
export async function publishReviewed(review: PublicationReview, publicConsent: boolean, assertCurrent = () => {}) {
  assertCurrent();
  if (publicConsent !== true) throw Error('Explicit public-publication consent is required');
  if (!navigator.locks) throw Error('This browser needs Web Locks for safe publication');
  const identity = selectedSigner(), { device, protocol } = await stores();
  if (review.owner !== identity.owner || review.generation !== identity.generation) throw Error('Signer changed since preview');
  const current = await currentDeviceSnapshot(), fresh = await resourceMappings(current);
  if (fresh.policy.origin !== review.sourceOrigin || fresh.policy.service_key !== review.serviceKey) throw Error('Source instance changed since preview');
  // Recheck current source mappings in a worker without building a second huge graph.
  const fingerprint = await graphWorker({ mappings: [...fresh.maps.values()], fingerprintOnly: true });
  const preferences = await protocol.relayPreferences(), canonical = await protocol.counts();
  for (const relay of review.recipients) assertAllowedRelay(relay.origin, fresh.policy);
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
  await deliverJob(protocol, job.id, activeSigner(identity.signer, assertCurrent), identity.owner, undefined, assertCurrent);
  return job.id;
}
export async function publicationJobs() { const { protocol } = await stores(); return protocol.jobs(); }
export async function retryPublication(id: string, assertCurrent = () => {}) {
  assertCurrent();
  const { protocol } = await stores(), identity = selectedSigner(), job = await protocol.job(id), policy = await capabilities();
  if (!job || job.event.pubkey !== identity.owner) throw Error('Connect the identity that signed this snapshot');
  for (const relay of job.recipients) assertAllowedRelay(relay.origin, policy);
  assertCurrent();
  await deliverJob(protocol, id, activeSigner(identity.signer, assertCurrent), identity.owner, undefined, assertCurrent);
}
export async function cancelPublication(id: string) { const { protocol } = await stores(); return cancelJob(protocol, id); }
export async function removePublication(id: string) {
  const { protocol } = await stores(), job = await protocol.job(id);
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
  const manga = await scopedCatalogCandidates<Manga>('/manga', 'id', [...mangaIds], query), chapters = await scopedCatalogCandidates<Chapter>('/chapter/feed', 'id', [...chapterIds], query);
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
  const identity = selectedSigner(), { protocol } = await stores(), revision = (await protocol.counts()).revision, preferences = await protocol.relayPreferences();
  const result = await recoverPublic(relay, activeSigner(identity.signer, assertCurrent), identity.owner); identity.check(); assertCurrent();
  return { ...result, relay, protocolRevision: revision, relayRevision: preferences.revision, owner: identity.owner, generation: identity.generation };
}
export async function applyRecovery(review: RecoveryReview) {
  const identity = selectedSigner(), { protocol } = await stores();
  if (identity.owner !== review.owner || identity.generation !== review.generation) throw Error('Identity changed since recovery preview');
  return deviceWriteLock(() => protocol.replace(review.snapshot, review.protocolRevision, { owner: review.event.pubkey, id: review.event.id, created_at: review.event.created_at }, review.relayRevision));
}
export async function exportCanonicalBackup() { const { protocol } = await stores(); return backupBlob(protocol); }
export async function canonicalRecords(family: string, after: string | null = null) { const { protocol } = await stores(); return { counts: await protocol.counts(), records: await protocol.list(family, after, 50) }; }
export async function removeCanonicalRecord(family: string, key: string) { const { protocol } = await stores(); await protocol.remove(family, key); }
export type CanonicalImportReview = { file: File; expectedRevision: number; counts: Record<string, number>; records: number };
export async function previewCanonicalImport(file: File): Promise<CanonicalImportReview> {
  const { protocol } = await stores(), expectedRevision = (await protocol.counts()).revision;
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
  return { file, expectedRevision, counts, records };
}
export async function applyCanonicalImport(review: CanonicalImportReview) {
  const { protocol } = await stores(), { importBackup } = await import('./protocol/backup.js');
  // Original staged import and original revision CAS remain authoritative.
  await importBackup(protocol, review.file, review.expectedRevision);
}
