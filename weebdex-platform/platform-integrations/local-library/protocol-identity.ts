// Public original account/social transport. Password accounts never sign here.
import { PUBLIC_API_URL, PUBLIC_MANGADEX_CATALOGUE } from '$env/static/public';
import {publicResourceMappings} from '../public-api/resource-mappings';
import {publicUuid} from '../public-api/validation';
import {sameScope} from './device-store';
import { request, transaction } from './protocol/indexeddb.js';
import { stores, currentDeviceSnapshot } from './library';
import { capabilities, assertAllowedRelay, selectedSigner, type Relay, type SourceMapping } from './publication';
import { origin, proveService, bytesResponse, signExact, signedRequest, verifyEvent, now } from './protocol/nostr.js';
import { isPublicActionReceipt } from './protocol-receipt';
export type SignedEvent = { id: string; pubkey: string; kind: number; created_at: number; tags: string[][]; content: string; sig: string };
export type PublicReview = { relay: Relay; owner: string; generation: number; kind: number; content: string; tags: string[][]; purpose: string; source?: SourceMapping; parent?: SignedEvent; metadataBase?: MetadataBase; changes?: MetadataChanges };
export async function sourceResource(kind: 'manga' | 'chapter', id: string, assertContext: () => void = () => {}): Promise<SourceMapping> {
  assertContext();
  if (PUBLIC_MANGADEX_CATALOGUE === '1') id = publicUuid(id);
  else if (!/^[A-Za-z0-9]{10}$/.test(id)) throw Error('Choose a native catalogue resource from this instance');
  const scope = (await currentDeviceSnapshot()).scope; assertContext();
  const policy = await capabilities(); assertContext();
  if (!scope.catalogNamespace || scope.catalogNamespace !== policy.catalog_namespace || (scope.serviceKey && scope.serviceKey !== policy.service_key)) throw Error('Catalogue scope changed. Review the instance before using this native ID.');
  if (PUBLIC_MANGADEX_CATALOGUE === '1') {
    if (typeof policy.origin !== 'string' || typeof policy.service_key !== 'string' || !/^[0-9a-f]{64}$/.test(policy.service_key)) throw Error('This instance has no verified signed-source service.');
    const result = await publicResourceMappings({publicOrigin:scope.origin,sourceOrigin:policy.origin,serviceKey:policy.service_key,catalogNamespace:scope.catalogNamespace},[{kind,id}],assertContext);
    assertContext();
    const current = (await currentDeviceSnapshot()).scope; assertContext();
    if (!sameScope(scope,current)) throw Error('Catalogue scope changed while resolving this source. Review it again.');
    const source = result.mappings.find(row=>row.kind===kind&&row.id===id);
    if (!source || result.unresolved.length) throw Error('This resource has no visible signed source. Use the instance community if available.');
    return source;
  }
  const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + '/protocol/resources', { method: 'POST', credentials: 'omit', cache: 'no-store', redirect: 'error', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ resources: [{ kind, id }] }), signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw Error(`Source mapping failed: HTTP ${response.status}`);
  const result = await response.json();
  if (result.origin !== policy.origin || result.service_key !== policy.service_key || result.catalog_namespace !== policy.catalog_namespace) throw Error('Source mapping came from a different scoped instance');
  const source = result.data?.find((row: SourceMapping) => row.kind === kind && row.id === id);
  if (!source || !/^[0-9a-f]{64}$/.test(source.event_id) || !/^[0-9a-f]{64}$/.test(source.publisher ?? '') || source.source_kind !== (kind === 'manga' ? 1002 : 1001)) throw Error('This resource has no visible verified canonical source. Native-only catalogue comments use the instance account community.');
  return source;
}
export async function publicJson(relay: Relay, path: string, limit = 1024 * 1024) {
  const policy = await capabilities(); assertAllowedRelay(relay.origin, policy);
  if (!path.startsWith('/') || path.startsWith('//')) throw Error('Invalid original API path');
  const response = await fetch(origin(relay.origin) + path, { credentials: 'omit', redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(15000) });
  if (!response.ok) { const error = Error(`Original public read failed: HTTP ${response.status}`) as Error & { status: number; metadataHeadGuard?: boolean }; error.status = response.status; if (response.status === 404 && /^\/accounts\/[0-9a-f]{64}\/metadata\/(0|3|10002)$/.test(path)) { try { const value = JSON.parse(new TextDecoder().decode(await bytesResponse(response, 4096))); error.metadataHeadGuard = value.expectedHeadSupported === true; } catch { /* Unknown/malformed response does not prove safe absent-head updates. */ } } throw error; }
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await bytesResponse(response, limit)));
}
export async function readMetadata(relay: Relay, owner: string, kind: 0 | 3 | 10002) {
  if (!/^[0-9a-f]{64}$/.test(owner)) throw Error('Invalid public identity');
  await proveService(relay.origin, relay.serviceKey);
  const head = await publicJson(relay, `/accounts/${owner}/metadata/${kind}`);
  if (head.publisher !== owner || head.kind !== kind || !/^[0-9a-f]{64}$/.test(head.id) || typeof head.projected !== 'boolean') throw Error('Invalid metadata head receipt');
  if (!head.projected) return { head, event: null as SignedEvent | null };
  const event = await publicJson(relay, `/events/${head.id}`, 16384);
  if (!verifyEvent(event) || event.id !== head.id || event.pubkey !== owner || event.kind !== kind || event.created_at !== head.created_at) throw Error('Metadata signature or current head mismatch');
  return { head, event: event as SignedEvent };
}
export async function readSocial(relay: Relay, source: SourceMapping, offset = 0): Promise<SignedEvent[]> {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > 10000) throw Error('Social query offset is outside original bounds');
  await proveService(relay.origin, relay.serviceKey);
  const root = await publicJson(relay, `/events/${source.event_id}`, 16384);
  if (!verifyEvent(root) || root.id !== source.event_id || root.pubkey !== source.publisher || root.kind !== source.source_kind) throw Error('Selected instance does not have the verified source publication');
  const result = await publicJson(relay, `/social?target=${source.event_id}&limit=50&offset=${offset}`);
  if (result.public !== true || !Array.isArray(result.events) || result.events.length > 50) throw Error('Invalid public social response');
  for (const event of result.events) {
    if (!verifyEvent(event) || ![7, 1111].includes(event.kind)) throw Error('Unverified public social event');
    if (event.kind === 1111 && event.tags.filter((tag: string[]) => tag[0] === 'E' && tag[1] === source.event_id).length !== 1) throw Error('Comment belongs to a different source thread');
    if (event.kind === 7 && event.tags.filter((tag: string[]) => tag[0] === 'e').at(-1)?.[1] !== source.event_id) throw Error('Reaction belongs to a different source');
  }
  return result.events;
}
function checkedReview(review: PublicReview): PublicReview {
  const candidate = { ...review, pubkey: review.owner, created_at: now(), id: '0'.repeat(64), sig: '0'.repeat(128) };
  delete (candidate as Partial<PublicReview>).relay; delete (candidate as Partial<PublicReview>).source; delete (candidate as Partial<PublicReview>).parent; delete (candidate as Partial<PublicReview>).metadataBase; delete (candidate as Partial<PublicReview>).changes;
  if (new TextEncoder().encode(JSON.stringify(candidate)).length > 16384) throw Error('This template exceeds the original browser signed-event budget. Reduce its content or tags before reviewing.');
  return review;
}
export type MetadataBase = { headGuard: boolean; relay: Relay; owner: string; kind: 0 | 3 | 10002; head: { id: string; created_at: number; projected: boolean } | null; event: SignedEvent | null };
export type MetadataChanges = { added: string[]; changed: string[]; removed: string[] };
export async function metadataForEditing(relay: Relay, owner: string, kind: 0 | 3 | 10002): Promise<MetadataBase> {
  try { const result = await readMetadata(relay, owner, kind); return structuredClone({ relay, owner, kind, headGuard: result.head.expectedHeadSupported === true, ...result }); }
  catch (error) { if ((error as Error & { status?: number }).status !== 404) throw error; return structuredClone({ relay, owner, kind, headGuard: (error as Error & { metadataHeadGuard?: boolean }).metadataHeadGuard === true, head: null, event: null }); }
}
function checkedBase(relay: Relay, kind: 0 | 3 | 10002, base?: MetadataBase) {
  const identity = selectedSigner();
  if (!base || base.owner !== identity.owner || base.kind !== kind || base.relay.origin !== relay.origin || base.relay.serviceKey !== relay.serviceKey) throw Error('Load the current public information before editing it.');
  if (!base.headGuard) throw Error('This destination does not support safe conflict-checked profile updates. Ask its operator to upgrade; nothing was signed.');
  if (base.head && (!base.event || !verifyEvent(base.event) || base.event.id !== base.head.id || base.event.pubkey !== identity.owner || base.event.kind !== kind || base.event.created_at !== base.head.created_at)) throw Error('The current public information is still updating or cannot be verified. Reload before editing; nothing was signed.');
  if (!base.head && base.event) throw Error('Invalid public metadata baseline');
  return { identity, base: structuredClone(base) };
}
export function profileFields(base: MetadataBase): Record<string, unknown> {
  if (base.event && new TextEncoder().encode(base.event.content).length > 8000) throw Error('This profile exceeds the safe editor size limit. Existing information was not changed; use a compatible larger-profile editor.');
  const value = base.event ? JSON.parse(base.event.content) : {};
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('The current public profile is not an editable object.');
  return structuredClone(value);
}
export function reviewProfile(relay: Relay, patch: Record<string, string>, baseline?: MetadataBase, remove: string[] = []): PublicReview {
  const { identity, base } = checkedBase(relay, 0, baseline), before = profileFields(base), profile = { ...before, ...patch };
  if (remove.some(key => !Object.hasOwn(before, key)) || new Set(remove).size !== remove.length) throw Error('Choose existing profile fields to remove.');
  for (const key of remove) delete profile[key];
  if (new TextEncoder().encode(JSON.stringify(profile)).length > 8000) throw Error('Profile exceeds the signed-event budget');
  const changes = { added: Object.keys(profile).filter(key => !Object.hasOwn(before, key)), changed: Object.keys(profile).filter(key => Object.hasOwn(before, key) && JSON.stringify(before[key]) !== JSON.stringify(profile[key])), removed: Object.keys(before).filter(key => !Object.hasOwn(profile, key)) };
  return checkedReview({ relay: structuredClone(relay), owner: identity.owner, generation: identity.generation, kind: 0, tags: base.event?.tags ?? [], content: JSON.stringify(profile), purpose: 'Update your public profile', metadataBase: base, changes });
}
export function reviewFollows(relay: Relay, keys: string[], baseline?: MetadataBase): PublicReview {
  if (keys.length > 256 || keys.some(key => !/^[0-9a-f]{64}$/.test(key)) || new Set(keys).size !== keys.length) throw Error('Choose at most 256 distinct public identities');
  const { identity, base } = checkedBase(relay, 3, baseline), previous = base.event?.tags ?? [], oldKeys = previous.filter(tag => tag[0] === 'p').map(tag => tag[1]);
  // Preserve relay/pet-name extensions on retained contacts and all unrelated tags/content.
  const tags = [...previous.filter(tag => tag[0] !== 'p'), ...keys.flatMap(key => { const saved = previous.filter(tag => tag[0] === 'p' && tag[1] === key); return saved.length ? saved : [['p', key]]; })];
  const changes = { added: keys.filter(key => !oldKeys.includes(key)), changed: [], removed: oldKeys.filter(key => !keys.includes(key)) };
  return checkedReview({ relay: structuredClone(relay), owner: identity.owner, generation: identity.generation, kind: 3, tags, content: base.event?.content ?? '', purpose: 'Update the people you publicly follow', metadataBase: base, changes });
}
export function relayLines(base: MetadataBase): string[] { return (base.event?.tags ?? []).filter(tag => tag[0] === 'r').map(tag => tag[1] + (tag[2] ? ' ' + tag[2] : '')); }
export function reviewRelayList(relay: Relay, routes: string[], baseline?: MetadataBase): PublicReview {
  if (routes.length > 32 || routes.some(route => new TextEncoder().encode(route).length > 1024)) throw Error('Choose at most 32 relay URLs');
  const { identity, base } = checkedBase(relay, 10002, baseline), previous = base.event?.tags ?? [];
  const tags = routes.map(route => {
    const [input, mode, ...extra] = route.trim().split(/\s+/), url = new URL(input), local = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
    if (extra.length || (mode && !['read', 'write'].includes(mode)) || (url.protocol !== 'wss:' && !(local && url.protocol === 'ws:')) || url.username || url.password || url.hash) throw Error('Use one WSS relay URL per line, optionally followed by read or write.');
    const normalized = url.toString();
    return previous.find(tag => tag[0] === 'r' && new URL(tag[1]).toString() === normalized && (tag[2] ?? '') === (mode ?? '')) ?? (mode ? ['r', normalized, mode] : ['r', normalized]);
  });
  if (new Set(tags.map(tag => tag[1])).size !== tags.length) throw Error('Each relay URL can appear only once.');
  const old = previous.filter(tag => tag[0] === 'r'), names = (list: string[][]) => list.map(tag => tag.slice(1).join(' '));
  const before = names(old), after = names(tags), changes = { added: after.filter(value => !before.includes(value)), changed: [], removed: before.filter(value => !after.includes(value)) };
  return checkedReview({ relay: structuredClone(relay), owner: identity.owner, generation: identity.generation, kind: 10002, tags: [...previous.filter(tag => tag[0] !== 'r'), ...tags], content: base.event?.content ?? '', purpose: 'Update your public posting destinations', metadataBase: base, changes });
}
export async function assertMetadataUnchanged(review: PublicReview) {
  if (![0, 3, 10002].includes(review.kind)) return;
  const { base } = checkedBase(review.relay, review.kind as 0 | 3 | 10002, review.metadataBase);
  const current = await metadataForEditing(review.relay, review.owner, review.kind as 0 | 3 | 10002);
  if (!current.headGuard) throw Error('This destination no longer supports safe conflict-checked updates. Nothing was signed. Ask its operator to restore support before reviewing again.');
  if (current.head?.id !== base.head?.id || current.head?.created_at !== base.head?.created_at) throw Error('Your public information changed since it was loaded. Reload it and review your changes again; the newer information was not overwritten.');
  return current;
}
export function reviewComment(relay: Relay, source: SourceMapping, content: string, parent?: SignedEvent): PublicReview {
  if (!content.trim() || new TextEncoder().encode(content).length > 8000) throw Error('Enter a comment within the 8000-byte signed-event budget');
  if (parent && (!verifyEvent(parent) || parent.kind !== 1111 || parent.tags.filter(tag => tag[0] === 'E' && tag[1] === source.event_id).length !== 1)) throw Error('Reply parent is not a verified comment in this source thread');
  const identity = selectedSigner(), immediate = parent ?? { id: source.event_id, pubkey: source.publisher!, kind: source.source_kind! };
  return checkedReview({ relay: structuredClone(relay), owner: identity.owner, generation: identity.generation, kind: 1111, content, tags: [['E', source.event_id], ['K', String(source.source_kind)], ['P', source.publisher!], ['e', immediate.id], ['k', String(immediate.kind)], ['p', immediate.pubkey]], purpose: parent ? 'Publish this public reply' : 'Publish this public source comment', source: structuredClone(source), parent });
}
export function reviewReaction(relay: Relay, source: SourceMapping, reaction: string): PublicReview {
  if (!reaction || new TextEncoder().encode(reaction).length > 128) throw Error('Enter a reaction within 128 bytes');
  const identity = selectedSigner();
  return checkedReview({ relay: structuredClone(relay), owner: identity.owner, generation: identity.generation, kind: 7, content: reaction, tags: [['e', source.event_id], ['p', source.publisher!], ['k', String(source.source_kind)]], purpose: 'Publish this public source reaction', source: structuredClone(source) });
}
export function reviewWithdrawal(relay: Relay, event: SignedEvent): PublicReview {
  const identity = selectedSigner();
  if (!verifyEvent(event) || event.pubkey !== identity.owner || ![0, 3, 7, 1111, 10002].includes(event.kind)) throw Error('Only verified profile/follow/relay/social events by this signing identity can use this withdrawal control');
  return checkedReview({ relay: structuredClone(relay), owner: identity.owner, generation: identity.generation, kind: 5, content: '', tags: [['e', event.id]], purpose: 'Request withdrawal of this exact authored public event' });
}
export async function sendReviewed(review: PublicReview, publicConsent: boolean, assertContext: () => void = () => {}) {
  if (publicConsent !== true) throw Error('Review and approve the public action first');
  if (!navigator.locks) throw Error('Web Locks are required for safe cross-tab public delivery. No event was signed.');
  assertContext();
  const identity = selectedSigner(), policy = await capabilities();
  assertContext();
  if (identity.owner !== review.owner || identity.generation !== review.generation) throw Error('Signing identity changed since review');
  assertAllowedRelay(review.relay.origin, policy); await proveService(review.relay.origin, review.relay.serviceKey);
  assertContext();
  if (review.source) {
    const current = await sourceResource(review.source.kind, review.source.id, assertContext);
    assertContext();
    if (current.event_id !== review.source.event_id || current.publisher !== review.source.publisher || current.source_kind !== review.source.source_kind) throw Error('Source mapping changed since review');
    const root = await publicJson(review.relay, `/events/${current.event_id}`, 16384);
    assertContext();
    if (!verifyEvent(root) || root.pubkey !== current.publisher || root.kind !== current.source_kind) throw Error('Source is unavailable at the selected recipient');
    if (review.parent) { const parent = await publicJson(review.relay, `/events/${review.parent.id}`, 16384); assertContext(); if (!verifyEvent(parent) || parent.id !== review.parent.id || parent.pubkey !== review.parent.pubkey || parent.kind !== 1111) throw Error('Reply parent is no longer available'); }
  }
  let timestamp = now();
  if ([0, 3, 10002].includes(review.kind)) {
    const current = await assertMetadataUnchanged(review);
    if (current?.head) timestamp = Math.max(timestamp, current.head.created_at + 1);
    assertContext();
    if (timestamp > now() + 60) throw Error('Metadata publication clock is ahead. Wait before signing.');
  }
  assertContext();
  // signExact invokes the external signer on a deferred turn. Check the view at
  // that actual invocation, but retain a returned exact signed intent below.
  const actionSigner = { signEvent: async (template: unknown) => { assertContext(); return identity.signer.signEvent(template); } };
  const event = await signExact(actionSigner, identity.owner, { kind: review.kind, created_at: timestamp, tags: review.tags, content: review.content });
  identity.check();
  const path = review.kind === 5 ? '/account-deletions' : [0, 3, 10002].includes(review.kind) ? '/account-metadata' : '/social';
  const record = await rememberPublicAction({ event, ...([0, 3, 10002].includes(review.kind) ? { expectedHead: review.metadataBase!.head?.id ?? null } : {}), relay: review.relay, path, purpose: review.purpose, state: 'prepared', message: 'Signed and saved locally; not sent yet', cancelled: false, createdAt: Date.now() });
  // After a signer returns, preserve the exact signed intent before checking the
  // UI context again. A changed target stops transport, never rebinds its body.
  try { assertContext(); }
  catch (error) { await updatePublicAction(record.event.id, { message: 'Exact signed intent retained at its original recipient; the selected UI context changed before delivery. No transport was started. Reconcile, cancel or explicitly retry this saved body.' }); throw error; }
  return deliverPublicAction(record.event.id, assertContext);
}
export type PublicActionRecord = { event: SignedEvent; expectedHead?: string | null; relay: Relay; path: string; purpose: string; state: 'prepared' | 'sending' | 'accepted' | 'superseded' | 'outcome_unknown' | 'refused'; message: string; cancelled: boolean; createdAt: number; receipt?: { status: number; data: any } };
const ACTIONS_KEY = 'weebdexSignedPublicActions-v1';
export async function publicActionRecords(): Promise<PublicActionRecord[]> {
  const { protocol } = await stores();
  const records = await protocol.getMeta(ACTIONS_KEY) ?? [];
  if (!Array.isArray(records) || records.length > 20 || records.some(row => !verifyEvent(row.event) || !['/account-metadata', '/social', '/account-deletions'].includes(row.path))) throw Error('Invalid saved signed public-action outbox');
  return records;
}
async function rememberPublicAction(record: PublicActionRecord) {
  const { protocol } = await stores();
  return transaction(protocol.db, 'meta', 'readwrite', async (tx: IDBTransaction) => {
    const meta = tx.objectStore('meta'), records = await request(meta.get(ACTIONS_KEY)) ?? [];
    if (records.length >= 20) throw Error('Public-action outbox is full. Remove completed records before signing another action.');
    if (records.some((row: PublicActionRecord) => row.event.id === record.event.id)) throw Error('This exact signed action is already saved. Reconcile or retry its existing record.');
    meta.put([...records, structuredClone(record)], ACTIONS_KEY); return record;
  });
}
async function updatePublicAction(id: string, update: Partial<Pick<PublicActionRecord, 'state' | 'message' | 'cancelled' | 'receipt'>>) {
  const { protocol } = await stores();
  return transaction(protocol.db, 'meta', 'readwrite', async (tx: IDBTransaction) => {
    const meta = tx.objectStore('meta'), records: PublicActionRecord[] = await request(meta.get(ACTIONS_KEY)) ?? [], index = records.findIndex(row => row.event.id === id);
    if (index < 0) throw Error('Saved public action is missing');
    const previous = records[index];
    // A cancelled action stays cancelled; signed body and recipient never change.
    records[index] = { ...previous, ...update, cancelled: previous.cancelled || !!update.cancelled };
    meta.put(records, ACTIONS_KEY); return records[index];
  });
}
export async function reconcilePublicAction(id: string) {
  const record = (await publicActionRecords()).find(row => row.event.id === id);
  if (!record) throw Error('Saved public action is missing');
  await proveService(record.relay.origin, record.relay.serviceKey);
  try {
    const event = await publicJson(record.relay, record.event.kind === 5 ? `/account-deletions/${id}` : `/events/${id}`, 16384);
    if (!verifyEvent(event) || event.id !== id || event.pubkey !== record.event.pubkey || event.kind !== record.event.kind) throw Error('Public counterpart does not match the exact saved signed action');
    return updatePublicAction(id, { state: 'accepted', message: 'Exact signed public counterpart verified' });
  } catch (error) {
    if ((error as Error & { status?: number }).status !== 404) throw error;
    // Missing projection is not proof that the request was refused. Retain body.
    return updatePublicAction(id, { message: 'No visible exact counterpart yet. Acceptance/projection may be pending; retry only this saved signed body.' });
  }
}
export async function deliverPublicAction(id: string, assertContext: () => void = () => {}) {
  if (!navigator.locks) throw Error('Web Locks are required to safely deliver/retry a signed public action across tabs. The exact action is saved locally.');
  return navigator.locks.request('weebdex-public-action:' + id, { ifAvailable: true }, async lock => {
    if (!lock) throw Error('This exact public action is being delivered in another tab');
    assertContext();
    let record = (await publicActionRecords()).find(row => row.event.id === id);
    if (!record || record.cancelled) throw Error('Saved public action is cancelled or missing');
    const identity = selectedSigner(), policy = await capabilities();
    assertContext();
    if (identity.owner !== record.event.pubkey) throw Error('Connect the signing identity that owns this saved public action');
    assertAllowedRelay(record.relay.origin, policy);
    if (record.state === 'accepted' || record.state === 'superseded') return { event: record.event, receipt: record.receipt ?? { status: 200, data: { id, verified: true } } };
    await proveService(record.relay.origin, record.relay.serviceKey);
    assertContext();
    // Reconcile a lost response first; exact IDs make explicit retry idempotent.
    if (record.state !== 'prepared') {
      const reconciled = await reconcilePublicAction(id);
      assertContext();
      if (reconciled.state === 'accepted') return { event: record.event, receipt: reconciled.receipt ?? { status: 200, data: { id, verified: true } } };
    }
    if (record.path === '/account-metadata') {
      if (!Object.hasOwn(record, 'expectedHead')) { await updatePublicAction(id, { state: 'refused', message: 'This older saved profile action has no conflict guard. Cancel it, load current information and review again.' }); throw Error('This older saved profile action has no conflict guard. Cancel it and review current information again.'); }
      const current = await metadataForEditing(record.relay, record.event.pubkey, record.event.kind as 0 | 3 | 10002); assertContext();
      if (!current.headGuard) throw Error('This destination does not support safe conflict-checked delivery. No HTTP proof or update was sent.');
      if ((current.head?.id ?? null) !== record.expectedHead && current.head?.id !== record.event.id) { await updatePublicAction(id, { state: 'refused', message: 'The current public information changed. Cancel this action, reload and review again; nothing was overwritten.' }); throw Error('The current public information changed. Reload and review again; nothing was overwritten.'); }
    }
    const active = async () => { const current = (await publicActionRecords()).find(row => row.event.id === id); identity.check(); assertContext(); if (!current || current.cancelled) throw Error('Further sending cancelled; an already sent request cannot be recalled'); };
    await active(); await updatePublicAction(id, { state: 'sending', message: 'Delivering this exact saved signed action' });
    try {
      // A fresh HTTP proof is disposable; closing its view must prevent another
      // extension prompt or transport after a pending proof returns. The saved
      // original action and final IDB/beforeSend cancellation check stay intact.
      const proofSigner = { signEvent: async (template: unknown) => { assertContext(); const signed = await identity.signer.signEvent(template); assertContext(); return signed; } };
      const body = record.path === '/account-metadata' && Object.hasOwn(record, 'expectedHead') ? { ...record.event, expectedHead: record.expectedHead } : record.event;
      const receipt = await signedRequest(record.relay, proofSigner, identity.owner, record.path, 'POST', body, 65536, active);
      const conflict = receipt.data && typeof receipt.data === 'object' && !Array.isArray(receipt.data) ? receipt.data as Record<string, unknown> : undefined;
      if (receipt.status === 409 && conflict?.id === id && conflict.headChanged === true) { await updatePublicAction(id, { state: 'refused', message: 'Your public information changed after review. Nothing was overwritten. Cancel this saved action, reload the current information and review again.', receipt }); throw Error('Your public information changed after review. Reload and review again; the newer information was not overwritten.'); }
      const accepted = isPublicActionReceipt(receipt.data) ? receipt.data : undefined;
      if (!accepted || accepted.id !== id || ![200, 202, 409].includes(receipt.status)) {
        if ([400, 401, 403, 404, 422].includes(receipt.status)) { await updatePublicAction(id, { state: 'refused', message: `Original service refused this exact action: HTTP ${receipt.status}`, receipt }); throw Error(`Original service refused this action: HTTP ${receipt.status}`); }
        throw Error(`A reliable matching event receipt was not received: HTTP ${receipt.status}`);
      }
      if (receipt.status === 409 && accepted.superseded !== true) throw Error('Unexpected publication conflict');
      await updatePublicAction(id, { state: receipt.status === 409 ? 'superseded' : 'accepted', message: receipt.status === 409 ? 'Superseded by a newer signed public event' : accepted.projected === true ? 'Accepted and projected' : 'Accepted; projection/withdrawal may still be pending', receipt });
      return { event: record.event, receipt };
    } catch (error) {
      record = (await publicActionRecords()).find(row => row.event.id === id)!;
      if (record.state !== 'refused') await updatePublicAction(id, { state: 'outcome_unknown', message: `Outcome unknown: ${error instanceof Error ? error.message : String(error)}. The exact signed event is retained; reconcile before an explicit retry.` });
      throw error;
    }
  });
}
export async function cancelPublicAction(id: string) { return updatePublicAction(id, { cancelled: true, message: 'Further delivery cancelled. Already sent requests cannot be recalled.' }); }
export async function removePublicAction(id: string) {
  const { protocol } = await stores();
  return transaction(protocol.db, 'meta', 'readwrite', async (tx: IDBTransaction) => {
    const meta = tx.objectStore('meta'), records: PublicActionRecord[] = await request(meta.get(ACTIONS_KEY)) ?? [], record = records.find(row => row.event.id === id);
    if (!record) return;
    if (!record.cancelled && !['accepted', 'superseded'].includes(record.state)) throw Error('Cancel further delivery before removing an unresolved exact-action record');
    meta.put(records.filter(row => row.event.id !== id), ACTIONS_KEY);
  });
}
