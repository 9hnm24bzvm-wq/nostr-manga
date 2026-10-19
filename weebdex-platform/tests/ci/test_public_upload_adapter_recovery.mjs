// Finite fake-transport adapter seams. This is not browser, HTTP, SQL or publication evidence.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const context = vm.createContext({ TextEncoder, TextDecoder, Uint8Array, ArrayBuffer, Blob, AbortController, URL, structuredClone });
const modules = new Map();
const forbidden = () => { throw Error('Unexpected production primitive in fake adapter seam'); };
function synthetic(id, exports) { if (!modules.has(id)) modules.set(id, new vm.SyntheticModule(Object.keys(exports), function () { for (const [key, value] of Object.entries(exports)) this.setExport(key, value); }, { context, identifier: id })); return modules.get(id); }
async function load(file) {
  if (modules.has(file)) return modules.get(file);
  const module = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform', sourceUrl: file }), { context, identifier: file }); modules.set(file, module);
  await module.link(specifier => {
    if (specifier === 'svelte') return synthetic(specifier, { onDestroy: forbidden, createContext: () => { let value; return [() => value, next => value = next]; } });
    if (specifier === '$lib/constants/Error') return load(path.join(root, 'frontend/src/lib/constants/Error.ts'));
    if (specifier === '../local-library/publication') return synthetic(specifier, { capabilities: forbidden, selectedSigner: forbidden });
    if (specifier === '../local-library/protocol/nostr.js') return synthetic(specifier, { proveService: forbidden, origin: value => { const url = new URL(value); if (url.origin !== value || !['http:', 'https:'].includes(url.protocol)) throw Error('Invalid exact origin'); return value; } });
    if (specifier === './nostr-upload-request') return synthetic(specifier, { createNostrUploadRequest: forbidden });
    if (specifier === './site-client') return synthetic(specifier, { publicCatalogueRead: forbidden });
    if (!['./validation', './upload-multipart'].includes(specifier)) throw Error('Unexpected adapter dependency ' + specifier);
    return load(path.resolve(path.dirname(file), specifier + '.ts'));
  });
  return module;
}
const module = await load(path.join(root, 'platform-integrations/public-api/upload-client.ts')); await module.evaluate();
const adapter = module.namespace;
const uuid = digit => `${digit.repeat(8)}-${digit.repeat(4)}-8${digit.repeat(3)}-8${digit.repeat(3)}-${digit.repeat(12)}`;
const MANGA = uuid('1'), SESSION = uuid('2'), USER = uuid('3'), FILE = uuid('4'), CHAPTER = uuid('5'), NAMESPACE = uuid('6'), SECOND_FILE = uuid('7'), SECOND_SESSION = uuid('8');
const PUBLIC = 'http://127.0.0.1:61411', RELAY = 'http://127.0.0.1:61412', SERVICE = 'a'.repeat(64), ACTOR = 'b'.repeat(64);
const PNG = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3, 4]);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const clone = value => structuredClone(value);
const response = (body, status = 200, mime = 'application/json') => ({ status, body, bodyAvailable: true, headers: { 'content-type': mime, 'retry-after': null, 'cache-control': 'private, no-store', vary: 'Authorization', 'x-content-type-options': 'nosniff' } });
const refusal = (status, detail = 'Explicit bound-account authorization refused') => response({ result: 'error', errors: [{ status, title: 'Refused', detail }] }, status);
const image = (name = 'page.png') => ({ name, type: 'image/png', size: PNG.length, arrayBuffer: async () => PNG.slice().buffer });
const localPage = (name = 'page.png') => ({ id: 'local-page', file: image(name), original_file_name: name, file_hash: '', file_name: '', file_ext: '.png', file_size: String(PNG.length), thumbnail: 'blob:explicit-local-fixture' });
function chapterResource(version = 2, pending = false) { return { id: CHAPTER, type: 'chapter', attributes: { version, title: null, volume: null, chapter: '1', translatedLanguage: 'en', pages: 1, isUnavailable: pending, externalUrl: null, nostr: { publicationScope: 'original-protocol', publicationStatus: pending ? 'accepted_pending_projection' : 'materialized', eventId: 'c'.repeat(64) } }, relationships: [{ id: MANGA, type: 'manga' }] }; }
function chapterView(receipt = chapterResource()) { return { id: receipt.id, version: receipt.attributes.version, title: receipt.attributes.title, chapter: receipt.attributes.chapter, language: receipt.attributes.translatedLanguage, public_api: { resource: receipt }, relationships: { manga: { id: MANGA } }, is_unavailable: receipt.attributes.isUnavailable, external_url: null, data: [{ name: 'original.png', public_url: '/data/exact/original.png', dimensions: [2, 3] }] }; }
function session(id = SESSION) { return { id, createdAt: '2026-10-17T01:02:03Z', updatedAt: '2026-10-17T02:03:04Z', mangaId: MANGA, chapterId: null, groupIds: [], version: 1, draft: {}, pageOrder: [], files: [], canMutate: true, canCancel: true, canRetryCommit: true, isCommitted: false, isProcessed: false, isDeleted: false, isExpired: false, expiresAt: '2026-10-18T00:00:00Z', state: 'open', publicationStatus: 'unprepared', targetStatus: 'available', receiptStatus: 'none', receipt: null }; }
function staged(id = FILE, name = 'page.png', sessionId = SESSION) { return { id, originalFileName: name, fileHash: digest(PNG), fileSize: PNG.length, mimeType: 'image/png', dimensions: { width: 2, height: 3 }, previewUrl: `/api/v1/nostr/uploads/${sessionId}/files/${id}/preview` }; }
function fixture(options = {}) {
  const storage = options.storage ?? new Map(), requests = [], proofs = [], destroys = [];
  const state = { policy: { available: true, schema_ready: true, origin: RELAY, service_key: SERVICE, catalog_namespace: NAMESPACE, connect_origins: [], external_signer_required: true, account_admission_required: true }, publicOrigin: PUBLIC, generation: 1, active: true, sessions: new Map(), current: undefined, beginCount: 0, attachCount: 0, catalogue: chapterView(), ...options.state };
  const account = { result: 'ok', user: { id: USER, username: 'actual-bound-user', version: 7, roles: [], permissions: ['chapter.create', 'self.chapter.update'] }, nostr: { pubkey: ACTOR, serviceKey: SERVICE, catalogNamespace: NAMESPACE, authentication: 'NIP-98', requestScoped: true, nativeSessionAuthority: false, catalogueScope: 'instance-local' }, upload: { canBegin: true, canManage: true, conditional: true, publication: { configured: true, requiresOperatorOrAuthorizedGroup: true, scope: 'original-protocol' }, limits: { maxFiles: 500, maxFileBytes: 25 * 1024 * 1024 } } };
  options.account?.(account);
  const defaultRequest = async request => {
    const url = new URL(request.url), route = url.pathname.slice('/api/v1'.length), body = request.body && request.contentType === 'application/json' ? JSON.parse(new TextDecoder().decode(request.body)) : undefined;
    if (route === '/nostr/account-context') return response(clone(account));
    if (route === '/upload' && request.method === 'GET') { const current = state.sessions.get(state.current); return current ? response({ id: current.id, type: 'upload_session', attributes: { version: current.version, isCommitted: current.isCommitted, isProcessed: current.isProcessed, isDeleted: current.isDeleted } }) : refusal(404); }
    if (route === '/upload/begin') { const id = state.beginCount++ ? SECOND_SESSION : SESSION, current = session(id); state.sessions.set(id, current); state.current = id; return response({ id, type: 'upload_session', attributes: { version: current.version, isCommitted: current.isCommitted, isProcessed: current.isProcessed, isDeleted: current.isDeleted } }); }
    if (route === '/upload/check-approval-required') return response({ result: 'ok', requiresApproval: false });
    const exact = /^\/nostr\/uploads\/([^/]+)$/.exec(route);
    if (exact) { const current = state.sessions.get(exact[1]); if (!current) return refusal(404); if (request.method === 'PATCH') { if (body.version !== current.version) return refusal(409); current.draft = clone(body.draft); current.pageOrder = clone(body.pageOrder); current.version++; } return response({ result: 'ok', session: clone(current) }); }
    const replace = /^\/nostr\/uploads\/([^/]+)\/files\/([^/]+)\/replace$/.exec(route);
    if (replace && request.method === 'POST') {
      const current = state.sessions.get(replace[1]), previous = current?.files.find(file => file.id === replace[2]);
      if (!current || !previous) return refusal(404);
      const wire = new TextDecoder().decode(request.body), version = Number(/name="expectedVersion"\r\n\r\n([1-9][0-9]*)\r\n/.exec(wire)?.[1]), name = /filename="([^"]+)"/.exec(wire)?.[1], mime = /name="file"; filename="[^"]+"\r\nContent-Type: ([^\r]+)\r\n/.exec(wire)?.[1];
      if (version !== current.version) return refusal(409, 'Stale replacement version');
      const offset = new TextEncoder().encode(wire.slice(0, wire.indexOf('\r\n\r\n', wire.indexOf('name="file"')) + 4)).length;
      const tail = new TextEncoder().encode('\r\n--' + request.contentType.split('boundary=')[1] + '--\r\n').length, bytes = request.body.slice(offset, request.body.length - tail), hash = digest(bytes);
      if (previous.fileHash !== hash || previous.originalFileName !== name) current.version++;
      Object.assign(previous, { originalFileName: name, fileHash: hash, fileSize: bytes.length, mimeType: mime }); state.replaceCount = (state.replaceCount ?? 0) + 1;
      return response({ result: 'ok', file: { id: previous.id, type: 'upload_session_file', attributes: { originalFileName: name, fileHash: hash, fileSize: bytes.length, mimeType: mime, source: 'local', version: 1 } }, sessionVersion: current.version });
    }
    if (/\/preview$/.test(route)) return response(PNG.slice(), 200, 'image/png');
    const upload = /^\/upload\/([^/]+)(?:\/([^/]+))?$/.exec(route); if (!upload) throw Error('Unexpected fake route ' + route);
    const current = state.sessions.get(upload[1]); if (!current) return refusal(404);
    if (request.method === 'POST' && !upload[2]) { const id = state.attachCount++ ? SECOND_FILE : FILE, bytes = new TextDecoder().decode(request.body), name = /filename="([^"]+)"/.exec(bytes)?.[1]; const file = staged(id, name, current.id); current.files.push(file); current.pageOrder.push(id); current.version++; return response({ result: 'ok', errors: [], data: [{ id, type: 'upload_session_file', attributes: { originalFileName: name, fileHash: file.fileHash, fileSize: file.fileSize, mimeType: file.mimeType, source: 'local', version: 1 } }] }); }
    if (request.method === 'POST' && upload[2] === 'commit') { const receipt = chapterResource(); Object.assign(current, { isCommitted: true, chapterId: CHAPTER, state: 'committed', canMutate: false, canCancel: false, canRetryCommit: false, publicationStatus: 'materialized', receiptStatus: 'available', receipt }); return response(receipt); }
    if (request.method === 'DELETE') { if (!upload[2]) { current.state = 'deleted'; current.isDeleted = true; current.canMutate = false; current.canCancel = false; current.canRetryCommit = false; state.current = undefined; } else { const removed = upload[2] === 'batch' ? body : [upload[2]]; current.files = current.files.filter(file => !removed.includes(file.id)); current.pageOrder = current.pageOrder.filter(id => !removed.includes(id)); current.version++; } return response({ result: 'ok' }); }
    throw Error('Unexpected fake method ' + request.method + ' ' + route);
  };
  const api = adapter.createPublicUploadAdapter({
    capabilities: async () => { await options.capabilities?.(state, storage); return clone(state.policy); }, publicOrigin: () => state.publicOrigin,
    proveService: async (origin, serviceKey) => { proofs.push({ origin, serviceKey }); return { origin, serviceKey }; },
    selectedSigner: () => { const generation = state.generation; return { owner: ACTOR, generation, signer: { signEvent: forbidden }, check: () => { if (!state.active || state.generation !== generation) throw Error('Selected signer changed'); } }; },
    createRequest: captured => async request => { state.preparing = request; try { captured.assertCurrent(); await captured.beforeSend?.(new AbortController().signal); captured.assertCurrent(); } catch (error) { throw Object.assign(error, { outcome: 'unsent' }); } finally { state.preparing = undefined; } requests.push(request); const result = await (options.request ? options.request(request, defaultRequest, state) : defaultRequest(request)); captured.assertCurrent(); options.afterTransport?.(request, state); return result; },
    catalogueRead: async (operation, { id }) => { assert.equal(operation, 'chapter.get'); assert.equal(id, CHAPTER); if (options.catalogue) return options.catalogue(state); return response(clone(state.catalogue)); },
    multipart: async file => { const source = await fs.promises.readFile(path.join(root, 'platform-integrations/public-api/upload-multipart.ts'), 'utf8'); assert.match(source, /serializeUploadMultipart/); const multipart = (await load(path.join(root, 'platform-integrations/public-api/upload-multipart.ts'))).namespace; return multipart.serializeUploadMultipart(file, { boundary: 'explicit-fake-boundary-123456' }); },
    digest: async bytes => digest(bytes), decodeImage: async () => ({ width: 2, height: 3 }), destroy: callback => destroys.push(callback), newAttempt: () => 'explicit-fake-attempt-' + (state.attempts = (state.attempts ?? 0) + 1),
    fences: { get: key => storage.get(key) ?? null, set: (key, value) => storage.set(key, value), delete: key => storage.delete(key) },
    replacementLock: async (_key, action) => action(),
    replacementFences: { get: key => storage.get(key) ?? null, set: (key, value) => storage.set(key, value), delete: key => storage.delete(key) },
    replacementMultipart: async (file, version) => (await load(path.join(root, 'platform-integrations/public-api/upload-multipart.ts'))).namespace.serializeUploadReplacementMultipart(file, version, { boundary: 'explicit-fake-boundary-123456' }),
    ...options.dependencies,
  });
  return { api, state, requests, proofs, destroys, storage, account, async owner() { const user = await api.loadUploadAccount(); return { user, owner: api.publicUploadOwner(() => MANGA, user) }; }, async begin(owner) { const result = await owner.request('/upload/begin', 'POST', { manga: MANGA, groups: [] }); assert.equal(result.status, 200); return result.body; } };
}
async function ready(options = {}) { const fixtureState = fixture(options), { owner, user } = await fixtureState.owner(), current = await fixtureState.begin(owner); return { ...fixtureState, owner, user, current }; }
async function attach(f, name = 'page.png') { const page = localPage(name); await f.api.runUploadBatch(f.current, [page], f.owner); return page; }
async function stagedCommit(f) { const page = await attach(f), saved = await f.api.saveResume(f.current, { draft: { chapter: '1', language: 'en' }, page_order: [page.id] }, f.owner); return { page, saved, payload: { draft: saved.draft, page_order: saved.page_order, termsAccepted: true } }; }
const mutations = f => f.requests.filter(row => row.method !== 'GET');

test('account binds actual UUID/version/grants and proves distinct protocol/public origins', async () => { const f = fixture(), { owner, user } = await f.owner(); assert.equal(user.id, USER); assert.equal(user.version, 7); assert.deepEqual([...user.roles], []); assert.deepEqual([...user.permissions], ['chapter.create', 'self.chapter.update']); assert.equal(f.proofs[0].origin, RELAY); assert.equal(new URL(f.requests[0].url).origin, PUBLIC); assert.equal(f.api.uploadAccountFacts(user).canBegin, true); assert.equal(owner.actor, USER); f.destroys[0](); assert.equal(owner.alive(), false); });
test('missing or invalid bound account version fails without a fabricated default', async () => { for (const version of [undefined, null, 0, -1, 1.5, '7']) { const f = fixture({ account: value => { value.user.version = version; } }); await assert.rejects(() => f.api.loadUploadAccount(), /Invalid upload integer/); assert.equal(mutations(f).length, 0); } });
test('session maps exact native timestamps and processing/deletion flags', async () => { const f = await ready(), native = f.state.sessions.get(SESSION); assert.equal(f.current.created_at, native.createdAt); assert.equal(f.current.updated_at, native.updatedAt); assert.equal(f.current.is_processed, false); assert.equal(f.current.is_deleted, false); Object.assign(native, { updatedAt: '2026-10-17T03:04:05Z', isProcessed: true, isDeleted: true, state: 'deleted', canMutate: false, canCancel: false, canRetryCommit: false }); const restored = await f.api.currentUpload(f.owner); assert.equal(restored.created_at, native.createdAt); assert.equal(restored.updated_at, native.updatedAt); assert.equal(restored.is_processed, true); assert.equal(restored.is_deleted, true); });
test('missing or invalid native session facts fail without fabricated timestamps or flags', async () => { for (const [key, value] of [['createdAt', undefined], ['createdAt', null], ['createdAt', ''], ['createdAt', 1], ['updatedAt', undefined], ['updatedAt', null], ['updatedAt', ''], ['updatedAt', 1], ['isProcessed', undefined], ['isProcessed', null], ['isProcessed', 'false'], ['isProcessed', 0], ['isDeleted', undefined], ['isDeleted', null], ['isDeleted', 'false'], ['isDeleted', 0]]) { const f = await ready(); f.state.sessions.get(SESSION)[key] = value; const before = mutations(f).length; await assert.rejects(() => f.api.currentUpload(f.owner), /Invalid upload/); assert.equal(mutations(f).length, before); } });
test('native deletion flag must agree with the exact open, committed or deleted state', async () => { for (const change of [{ isDeleted: true }, { state: 'committed', isCommitted: true, isDeleted: true, canMutate: false, canCancel: false, canRetryCommit: false, receiptStatus: 'unavailable' }, { state: 'deleted', isDeleted: false, canMutate: false, canCancel: false, canRetryCommit: false }]) { const f = await ready(); Object.assign(f.state.sessions.get(SESSION), change); const before = mutations(f).length; await assert.rejects(() => f.api.currentUpload(f.owner), /Inconsistent upload state/); assert.equal(mutations(f).length, before); } });
test('admission, empty roles and absent native grants never become upload eligibility', async () => { const f = fixture({ account: value => { value.user.permissions = []; value.upload.canBegin = false; value.upload.canManage = false; } }), { owner } = await f.owner(); await assert.rejects(() => f.begin(owner), /bound native account/); assert.equal(mutations(f).length, 0); });
test('mismatched actor/service/catalogue and fabricated grant flags are refused', async () => { for (const change of [value => value.nostr.pubkey = 'c'.repeat(64), value => value.nostr.serviceKey = 'c'.repeat(64), value => value.nostr.catalogNamespace = MANGA, value => value.upload.canBegin = false, value => value.nostr.nativeSessionAuthority = true]) { const f = fixture({ account: change }); await assert.rejects(() => f.api.loadUploadAccount()); } });
test('identity generation, view disposal and configured scope fence later mutations', async () => { for (const change of [f => f.state.generation++, f => f.owner.dispose(), f => f.state.policy.service_key = 'c'.repeat(64), f => f.state.policy.origin = PUBLIC, f => f.state.publicOrigin = RELAY]) { const f = await ready(); const before = mutations(f).length; change(f); await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE')); assert.equal(mutations(f).length, before); } });
test('only exact owned UUID upload routes are accepted; structural owner copies fail', async () => { const f = await ready(); for (const route of ['/chapter/' + CHAPTER, '//example/upload', '/upload/native-id', '/upload/' + SESSION + '?x=1', '/upload/' + SESSION + '/files/' + FILE + '/replace', '/upload/' + SECOND_SESSION]) await assert.rejects(() => f.owner.request(route, route.includes('chapter') ? 'PUT' : 'DELETE')); await assert.rejects(() => f.api.currentUpload({ ...f.owner }), /explicit signed upload owner/); assert.equal(mutations(f).length, 1); });
test('normal fake begin/one-image attachments/staging/reorder/preview/commit preserve real 200', async () => {
  const f = await ready(), first = await attach(f), second = await attach(f, 'second.png');
  assert.equal(adapter.isRemoteUploadFile(first), true); assert.equal(first.thumbnail, 'blob:explicit-local-fixture');
  const saved = await f.api.saveResume(f.current, { draft: { chapter: '1', language: 'en' }, page_order: [second.id, first.id] }, f.owner); assert.deepEqual([...saved.page_order], [SECOND_FILE, FILE]);
  const blob = await f.api.uploadPreview(f.owner, saved, first); assert.equal(blob.size, PNG.length); assert.equal(blob.type, 'image/png');
  const result = await f.owner.request(`/upload/${SESSION}/commit`, 'POST', { draft: saved.draft, page_order: saved.page_order, termsAccepted: true }); assert.equal(result.status, 200); assert.equal(result.accepted, true); assert.equal(result.body.id, CHAPTER); assert.equal(f.owner.commitState(), undefined);
  const request = f.requests.find(row => row.url.endsWith('/commit')), payload = JSON.parse(new TextDecoder().decode(request.body)); assert.deepEqual(payload, { chapterDraft: { volume: null, chapter: '1', title: null, translatedLanguage: 'en' }, pageOrder: [SECOND_FILE, FILE], termsAccepted: true });
  const attachments = f.requests.filter(row => row.method === 'POST' && row.url === `${PUBLIC}/api/v1/upload/${SESSION}`); assert.equal(attachments.length, 2); for (const row of attachments) { assert.equal(row.contentType, 'multipart/form-data; boundary=explicit-fake-boundary-123456'); assert.ok(row.body instanceof Uint8Array); assert.equal(new TextDecoder().decode(row.body).match(/name="file"/g).length, 1); }
  assert.equal((await f.begin(f.owner)).id, SECOND_SESSION);
});
test('explicit terms and acknowledged complete draft/order are required before dispatch', async () => { const f = await ready(), { payload } = await stagedCommit(f); for (const bad of [{ ...payload, termsAccepted: false }, { ...payload, termsAccepted: undefined }, { ...payload, draft: { language: 'fr' } }, { ...payload, page_order: [SECOND_FILE] }]) await assert.rejects(() => f.owner.request(`/upload/${SESSION}/commit`, 'POST', bad)); assert.equal(f.requests.filter(row => row.url.endsWith('/commit')).length, 0); });
test('private staging rejects stale draft/order and unknown file sets without PATCH', async () => { const f = await ready(), first = await attach(f), second = await attach(f, 'second.png'), value = { draft: { language: 'en' }, page_order: [first.id, second.id] }; const saved = await f.api.saveResume(f.current, value, f.owner); f.state.sessions.get(SESSION).draft = { language: 'fr' }; const count = f.requests.filter(row => row.method === 'PATCH').length; await assert.rejects(() => f.api.saveResume(saved, { ...value, draft: { language: 'de' } }, f.owner)); assert.equal(f.requests.filter(row => row.method === 'PATCH').length, count); await assert.rejects(() => f.api.saveResume(saved, { ...value, page_order: [FILE, MANGA] }, f.owner)); });
test('cancel and exact file/batch deletion preserve actual 200 semantics', async () => { const f = await ready(), first = await attach(f), second = await attach(f, 'second.png'); const removed = await f.owner.request(`/upload/${SESSION}/${first.id}`, 'DELETE'); assert.equal(removed.status, 200); assert.equal((await f.api.currentUpload(f.owner)).relationships.files.length, 1); assert.equal((await f.owner.request(`/upload/${SESSION}/batch`, 'DELETE', [second.id])).status, 200); await f.api.cancelResume(f.current, f.owner); assert.equal((await f.begin(f.owner)).id, SECOND_SESSION); });
test('unknown attachment is recovered only from one exact new matching original receipt', async () => { const f = await ready({ request: async (request, next) => { const result = await next(request); if (request.method === 'POST' && request.url === `${PUBLIC}/api/v1/upload/${SESSION}`) throw Object.assign(Error('Explicit fake dropped response'), { outcome: 'uncertain' }); return result; } }), page = await attach(f); assert.equal(page.id, FILE); assert.equal(page.file, undefined); assert.equal(page.upload_recovery, undefined); assert.equal(f.state.attachCount, 1); });
test('absence after unknown attachment is never refusal and cannot cause POST replay', async () => { const f = await ready({ request: async (request, next) => { if (request.method === 'POST' && request.url === `${PUBLIC}/api/v1/upload/${SESSION}`) throw Object.assign(Error('Explicit fake uncertain dispatch'), { outcome: 'uncertain' }); return next(request); } }), page = localPage(); await assert.rejects(() => f.api.runUploadBatch(f.current, [page], f.owner)); assert.equal(page.upload_recovery.state, 'unknown'); await assert.rejects(() => f.api.resolveUploadPage(f.current, page, f.owner)); const count = mutations(f).length; await assert.rejects(() => f.api.runUploadBatch(f.current, [page], f.owner)); assert.equal(mutations(f).length, count); });
test('definitive per-image rejection stays rejected and later images stay unsent', async () => { const f = await ready({ request: async (request, next) => request.method === 'POST' && request.url === `${PUBLIC}/api/v1/upload/${SESSION}` ? response({ result: 'error', data: [], errors: [{ status: 400, detail: 'Invalid image fixture', context: { originalFileName: 'page.png' } }] }) : next(request) }), pages = [localPage(), localPage('second.png')]; await assert.rejects(() => f.api.runUploadBatch(f.current, pages, f.owner)); assert.equal(pages[0].upload_recovery.state, 'rejected'); assert.equal(pages[1].upload_recovery.state, 'unsent'); assert.equal(f.requests.filter(row => row.url === `${PUBLIC}/api/v1/upload/${SESSION}` && row.method === 'POST').length, 1); });
test('an explicitly unsent attachment stays unsent and the batch guard blocks late dispatch', async () => { const f = await ready({ request: async (request, next) => { if (request.method === 'POST' && request.url === `${PUBLIC}/api/v1/upload/${SESSION}`) throw Object.assign(Error('Explicit fake signer refusal'), { outcome: 'unsent' }); return next(request); } }), page = localPage(); await assert.rejects(() => f.api.runUploadBatch(f.current, [page], f.owner)); assert.equal(page.upload_recovery.state, 'unsent'); let allowed = true; const g = await ready({ dependencies: { digest: async bytes => { allowed = false; return digest(bytes); } } }), other = localPage(); await assert.rejects(() => g.api.runUploadBatch(g.current, [other], g.owner, undefined, () => { if (!allowed) throw Error('Page view changed'); })); assert.equal(other.upload_recovery.state, 'unsent'); assert.equal(g.state.attachCount, 0); });
test('unknown commit survives a new owner and only explicit exact receipt reconciliation unlocks', async () => { const f = await ready({ request: async (request, next) => { const result = await next(request); return request.url.endsWith('/commit') ? response({ malformed: true }) : result; } }), { payload } = await stagedCommit(f); await assert.rejects(() => f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload)); assert.equal(f.owner.commitState(), 'uncertain'); assert.equal(f.owner.pendingCommitSession(), SESSION); assert.equal(f.storage.size, 1); const reopenedUser = await f.api.loadUploadAccount(), reopened = f.api.publicUploadOwner(() => MANGA, reopenedUser); assert.equal(reopened.pendingCommitSession(), SESSION); const previous = f.requests.length; const restored = await f.api.currentUpload(reopened); assert.equal(restored.is_committed, true); assert.equal(restored.can_mutate, false); assert.equal(restored.receipt, undefined); assert.equal(f.requests.slice(previous).some(row => row.url === `${PUBLIC}/api/v1/upload?expectedAccountId=${USER}`), false); await assert.rejects(() => f.begin(reopened)); const reconciled = await f.api.reconcileUploadReceipt(reopened, SESSION); assert.equal(reconciled.receipt.id, CHAPTER); assert.equal(reopened.commitState(), undefined); assert.equal(f.requests.filter(row => row.url.endsWith('/commit')).length, 1); });
test('known 202 is accepted-unavailable without inventing a readable chapter or receipt', async () => { const f = await ready({ request: async (request, next) => request.url.endsWith('/commit') ? response({ deliberatelyMalformed: true }, 202) : next(request) }), { payload } = await stagedCommit(f), result = await f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload); assert.equal(result.status, 202); assert.equal(result.accepted, true); assert.equal(result.body, undefined); assert.equal(result.receipt, undefined); assert.equal(f.owner.commitState(), 'accepted-unavailable'); f.state.sessions.delete(SESSION); await assert.rejects(() => f.api.currentUpload(f.owner)); await assert.rejects(() => f.api.reconcileUploadReceipt(f.owner, SESSION)); assert.equal(f.owner.commitState(), 'accepted-unavailable'); assert.equal(f.storage.size, 1); });
test('actual accepted pending response has no readable chapter and stays read-only', async () => { const f = await ready({ request: async (request, next, state) => { const result = await next(request); if (!request.url.endsWith('/commit')) return result; const receipt = chapterResource(2, true); Object.assign(state.sessions.get(SESSION), { receipt, publicationStatus: 'accepted_pending_projection' }); return response(receipt); } }), { payload } = await stagedCommit(f), result = await f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload); assert.equal(result.status, 200); assert.equal(result.accepted, true); assert.equal(result.body, undefined); assert.equal(f.owner.commitState(), 'accepted-unavailable'); const saved = f.state.sessions.get(SESSION); Object.assign(saved, { chapterId: null, receipt: null, receiptStatus: 'unavailable' }); const restored = await f.api.currentUpload(f.owner); assert.equal(restored.is_committed, true); assert.equal(restored.can_cancel, false); assert.equal(restored.receipt, undefined); await assert.rejects(() => f.api.reconcileUploadReceipt(f.owner, SESSION)); await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE')); });
test('failed or stale presentation cannot revive prior chapter fields after accepted 200', async () => { for (const catalogue of [() => { throw Error('Explicit fake presentation unavailable'); }, () => response(chapterView(chapterResource(1))), () => response({ ...chapterView(), id: MANGA })]) { const f = await ready({ catalogue }), { payload } = await stagedCommit(f); await assert.rejects(() => f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload)); assert.equal(f.owner.commitState(), 'accepted-unavailable'); const restored = await f.api.currentUpload(f.owner); assert.equal(restored.receipt, undefined); assert.equal(restored.committed_result, undefined); assert.equal(restored.can_retry_commit, false); assert.equal(f.requests.filter(row => row.url.endsWith('/commit')).length, 1); } });
test('explicit operator/group commit 403 is separate from admission and not accepted', async () => { const f = await ready({ request: async (request, next) => request.url.endsWith('/commit') ? refusal(403, 'The publisher is neither operator nor authorized group') : next(request) }), { payload } = await stagedCommit(f), result = await f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload); assert.equal(result.status, 403); assert.equal(result.accepted, undefined); assert.equal(f.owner.commitState(), undefined); assert.equal((await f.api.currentUpload(f.owner)).can_mutate, true); });
test('approval result maps actual false and never promotes HTTP200 to moderation state', async () => { const f = await ready(), result = await f.owner.request('/upload/check-approval-required', 'POST', { language: 'en' }); assert.equal(result.status, 200); assert.equal(result.body.requiresApproval, false); const request = f.requests.at(-1); assert.deepEqual(JSON.parse(new TextDecoder().decode(request.body)), { manga: MANGA, locale: 'en' }); });
test('signed preview validates exact MIME, original digest, size and decoded dimensions', async () => { for (const alter of [result => response(result.body, 200, 'image/jpeg'), result => response(new Uint8Array([1, 2]), 200, 'image/png'), result => response(Uint8Array.from(PNG, value => value ^ 1), 200, 'image/png')]) { const f = await ready({ request: async (request, next) => { const result = await next(request); return request.url.endsWith('/preview') ? alter(result) : result; } }), page = await attach(f); await assert.rejects(() => f.api.uploadPreview(f.owner, f.current, page)); } const f = await ready({ dependencies: { decodeImage: async () => ({ width: 20, height: 30 }) } }), page = await attach(f); await assert.rejects(() => f.api.uploadPreview(f.owner, f.current, page), /dimensions/); });
test('malformed UUID/order/preview/state snapshots fail closed with no invented view', async () => { for (const change of [value => value.id = SECOND_SESSION, value => value.pageOrder = [FILE], value => { value.files = [staged()]; value.pageOrder = [FILE]; value.files[0].previewUrl = 'https://example.invalid/leak'; }, value => value.canMutate = 'true', value => { value.state = 'committed'; value.isCommitted = true; }]) { const f = await ready(); change(f.state.sessions.get(SESSION)); await assert.rejects(() => f.api.currentUpload(f.owner)); } });
test('local placeholder snapshots remain representable but can never pass remote validation', () => { const page = localPage(), value = adapter.snapshot({ language: 'en' }, [page]); assert.equal(value.page_order[0], 'local-page'); assert.equal(adapter.isRemoteUploadFile(page), false); });
test('resume hydrates exact actual group names through bounded scoped public reads', async () => { const calls = [], groupId = SECOND_FILE, actual = { id: groupId, type: 'scanlation_group', attributes: { name: 'Actual selected group', version: 1 }, relationships: [] }; const f = await ready({ dependencies: { catalogueRead: async (operation, { id }) => { calls.push({ operation, id }); assert.equal(operation, 'group.get'); assert.equal(id, groupId); return response({ id, name: 'Actual selected group', version: 1, public_api: { resource: actual } }); } } }); f.state.sessions.get(SESSION).groupIds = [groupId]; const restored = await f.api.currentUpload(f.owner); assert.equal(restored.relationships.groups[0].id, groupId); assert.equal(restored.relationships.groups[0].name, 'Actual selected group'); await f.api.currentUpload(f.owner); assert.equal(calls.length, 1); });
test('ordinary PATCH and DELETE are fenced again after asynchronous capability refresh', async () => { for (const method of ['PATCH', 'DELETE']) { const f = await ready({ capabilities: async (state, storage) => { if (state.injectFence && state.preparing?.method === method) storage.set(`weebdex.public-upload-recovery.v1:${PUBLIC}:${SERVICE}:${NAMESPACE}:${ACTOR}`, JSON.stringify({ sessionId: SESSION, state: 'uncertain' })); } }); f.state.injectFence = true; const before = mutations(f).length; await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, method, method === 'PATCH' ? { version: 1, draft: {}, page_order: [] } : undefined)); assert.equal(mutations(f).length, before); assert.equal(f.owner.commitState(), 'uncertain'); } });
test('known 202 body-unavailable transport error retains accepted-unavailable state', async () => { const f = await ready({ request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/commit')) throw Object.assign(Error('Explicit fake unavailable body'), { outcome: 'response', status: 202, bodyAvailable: false }); return result; } }), { payload } = await stagedCommit(f); await assert.rejects(() => f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload)); assert.equal(f.owner.commitState(), 'accepted-unavailable'); assert.equal(f.owner.pendingCommitSession(), SESSION); assert.equal((await f.api.currentUpload(f.owner)).receipt, undefined); });
test('same-account discovery for a foreign target stays readable only for redirect', async () => { for (const target of ['manga', 'chapter']) { const f = await ready(); const stored = f.state.sessions.get(SESSION); if (target === 'manga') stored.mangaId = SECOND_SESSION; else stored.chapterId = CHAPTER; const visible = await f.api.currentUpload(f.owner); assert.equal(visible.id, SESSION); const before = mutations(f).length; await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE')); await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'PATCH', { version: stored.version, draft: {}, page_order: [] })); await assert.rejects(() => f.api.runUploadBatch(visible, [localPage()], f.owner)); assert.equal(mutations(f).length, before); } });
test('owner A queued DELETE cannot dispatch after owner B has an unresolved commit', { timeout: 2000 }, async () => { let entered, release; const queued = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve); const f = await ready({ capabilities: async state => { if (state.queueDelete && state.preparing?.method === 'DELETE') { entered(); await gate; } }, request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/commit')) throw Object.assign(Error('Explicit fake uncertain commit'), { outcome: 'uncertain' }); return result; } }), { payload } = await stagedCommit(f), secondUser = await f.api.loadUploadAccount(), secondOwner = f.api.publicUploadOwner(() => MANGA, secondUser); await f.api.currentUpload(secondOwner); f.state.queueDelete = true; const deletion = f.owner.request(`/upload/${SESSION}`, 'DELETE'); await queued; await assert.rejects(() => secondOwner.request(`/upload/${SESSION}/commit`, 'POST', payload)); release(); await assert.rejects(() => deletion); assert.equal(f.requests.filter(row => row.method === 'DELETE').length, 0); assert.equal(secondOwner.commitState(), 'uncertain'); });
test('simultaneous commit attempts dispatch exactly one natural commit', { timeout: 2000 }, async () => { let reads = 0, release; const gate = new Promise(resolve => release = resolve); const f = await ready({ request: async (request, next, state) => { const result = await next(request); if (state.race && request.method === 'GET' && request.url === `${PUBLIC}/api/v1/nostr/uploads/${SESSION}`) { if (++reads === 2) release(); await gate; } if (request.url.endsWith('/commit')) throw Object.assign(Error('Explicit fake uncertain commit'), { outcome: 'uncertain' }); return result; } }), { payload } = await stagedCommit(f), secondUser = await f.api.loadUploadAccount(), secondOwner = f.api.publicUploadOwner(() => MANGA, secondUser); await f.api.currentUpload(secondOwner); f.state.race = true; const results = await Promise.allSettled([f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload), secondOwner.request(`/upload/${SESSION}/commit`, 'POST', payload)]); assert.equal(results.every(result => result.status === 'rejected'), true); assert.equal(f.requests.filter(row => row.url.endsWith('/commit')).length, 1); });
test('queued ambiguous current discovery never dispatches after another owner commit fence', { timeout: 2000 }, async () => { let entered, release; const queued = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve); const f = await ready({ capabilities: async state => { if (state.queueCurrent && state.preparing?.url === `${PUBLIC}/api/v1/upload?expectedAccountId=${USER}`) { entered(); await gate; } }, request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/commit')) throw Object.assign(Error('Explicit fake uncertain commit'), { outcome: 'uncertain' }); return result; } }), { payload } = await stagedCommit(f), newUser = await f.api.loadUploadAccount(), newOwner = f.api.publicUploadOwner(() => MANGA, newUser); f.state.queueCurrent = true; const discovery = f.api.currentUpload(newOwner); await queued; await assert.rejects(() => f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload)); release(); await assert.rejects(() => discovery); assert.equal(f.requests.filter(row => row.url === `${PUBLIC}/api/v1/upload?expectedAccountId=${USER}`).length, 0); });
test('preview requires actual private no-store Authorization-vary and nosniff response headers', async () => { for (const change of [headers => { delete headers['cache-control']; }, headers => headers['cache-control'] = 'public, max-age=3600', headers => headers['cache-control'] = 'private', headers => headers.vary = 'Accept-Encoding', headers => { delete headers.vary; }, headers => headers['x-content-type-options'] = 'sniff', headers => { delete headers['x-content-type-options']; }]) { const f = await ready({ request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/preview')) change(result.headers); return result; } }), page = await attach(f); await assert.rejects(() => f.api.uploadPreview(f.owner, f.current, page), /privacy headers/); } });

// Replacement cases below use finite request/store/image-format seams only.
// They do not establish HTTP, browser, decoder, crypto, SQL or publication proof.
const replacementImage = (name = 'replacement.png', bytes = Uint8Array.from([...PNG, 9, 8, 7])) => ({ name, type: 'image/png', size: bytes.length, arrayBuffer: async () => bytes.slice().buffer });
const replacementRequests = f => f.requests.filter(request => request.url.endsWith('/replace'));
const replacementStorageKey = `weebdex.public-upload-replacement.v1:${PUBLIC}:${SERVICE}:${NAMESPACE}:${ACTOR}`;
const unknownReplacementError = () => Object.assign(Error('Explicit finite unknown replacement transport'), { outcome: 'uncertain' });
async function replacementReady(options = {}, neighbors = true) {
  const f = await ready(options), first = await attach(f);
  if (neighbors) await attach(f, 'neighbor.png');
  const current = await f.api.currentUpload(f.owner), target = current.relationships.files.find(page => page.id === first.id);
  target.thumbnail = 'blob:explicit-original-page';
  return { ...f, current, target };
}

test('replacement posts one exact version/file body then confirms stable ID, hash, version, draft and order without mutating old preview', async () => {
  const f = await replacementReady(), previous = clone(f.target), oldSession = clone(f.current), file = replacementImage();
  let reads = 0; const originalRead = file.arrayBuffer; file.arrayBuffer = () => { reads++; return originalRead(); };
  const result = await f.api.replaceUploadPage(f.current, f.target, file, f.owner);
  assert.equal(reads, 1);
  assert.equal(result.file.id, FILE); assert.equal(result.file.file_hash, digest(Uint8Array.from([...PNG, 9, 8, 7])));
  assert.equal(result.file.original_file_name, file.name); assert.equal(result.file.file_size, String(file.size)); assert.equal(result.file.public_upload.mimeType, file.type);
  assert.equal(result.session.version, oldSession.version + 1); assert.equal(result.session.can_mutate, true);
  assert.deepEqual([...result.session.page_order], [FILE, SECOND_FILE]); assert.deepEqual(clone(result.session.draft), oldSession.draft);
  assert.equal(result.session.relationships.files[1].file_hash, oldSession.relationships.files[1].file_hash);
  assert.deepEqual(clone(f.target), previous); assert.deepEqual(clone(f.current), oldSession); assert.equal(f.target.thumbnail, 'blob:explicit-original-page');
  assert.equal(f.owner.pendingReplacement(), undefined); assert.equal(f.storage.size, 0);
  const request = replacementRequests(f)[0], wire = new TextDecoder().decode(request.body), index = f.requests.indexOf(request);
  assert.equal(request.method, 'POST'); assert.equal(replacementRequests(f).length, 1); assert.equal(request.contentType, 'multipart/form-data; boundary=explicit-fake-boundary-123456');
  assert(wire.startsWith(`--explicit-fake-boundary-123456\r\nContent-Disposition: form-data; name="expectedVersion"\r\n\r\n${oldSession.version}\r\n--explicit-fake-boundary-123456\r\nContent-Disposition: form-data; name="file";`));
  assert.equal(wire.match(/name="expectedVersion"/g).length, 1); assert.equal(wire.match(/name="file"/g).length, 1);
  assert(f.requests.slice(index + 1).some(value => value.method === 'GET' && value.url === `${PUBLIC}/api/v1/nostr/uploads/${SESSION}`));
});

test('definitive identical replacement success retains actual unchanged session version', async () => {
  const f = await replacementReady(), result = await f.api.replaceUploadPage(f.current, f.target, replacementImage('page.png', PNG), f.owner);
  assert.equal(result.session.version, f.current.version); assert.equal(result.file.id, f.target.id); assert.equal(result.file.file_hash, f.target.file_hash);
  assert.equal(f.owner.pendingReplacement(), undefined); assert.equal(replacementRequests(f).length, 1);
});

test('well-formed replacement HTTP403 and stale409 preserve old image and clear only this attempt', async () => {
  for (const status of [403, 409]) {
    const f = await replacementReady({ request: (request, next) => request.url.endsWith('/replace') ? refusal(status, status === 403 ? 'Actual bound-account permission refused' : 'Actual stale replacement version') : next(request) }), old = clone(f.target);
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
    assert.deepEqual(clone(f.target), old); assert.equal(f.owner.pendingReplacement(), undefined); assert.equal(f.storage.size, 0); assert.equal(f.state.replaceCount ?? 0, 0);
    assert.equal((await f.api.currentUpload(f.owner)).can_mutate, true); assert.equal(replacementRequests(f).length, 1);
  }
});

test('admission alone never authorizes replacement without actual bound-account management grant', async () => {
  const f = fixture({ account: account => { account.user.permissions = []; account.upload.canBegin = false; account.upload.canManage = false; } });
  const { owner } = await f.owner(), saved = session(); saved.files = [staged()]; saved.pageOrder = [FILE]; f.state.sessions.set(SESSION, saved); f.state.current = SESSION;
  const current = await f.api.currentUpload(owner);
  await assert.rejects(() => f.api.replaceUploadPage(current, current.relationships.files[0], replacementImage(), owner));
  assert.equal(replacementRequests(f).length, 0); assert.equal(owner.pendingReplacement(), undefined);
});

test('malformed success, wrong stable ID/hash/name/size/MIME or session version freezes writes without adopting receipt', async () => {
  for (const change of [body => body.result = 'error', body => body.file.id = SECOND_FILE, body => body.file.attributes.fileHash = 'f'.repeat(64), body => body.file.attributes.originalFileName = 'other.png', body => body.file.attributes.fileSize++, body => body.file.attributes.mimeType = 'image/jpeg', body => body.sessionVersion++, body => { delete body.sessionVersion; }, body => body.file.type = 'chapter']) {
    const f = await replacementReady({ request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/replace')) change(result.body); return result; } }), old = clone(f.target);
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
    assert.deepEqual(clone(f.target), old); assert.equal(f.owner.pendingReplacement().fileId, FILE); assert.equal(f.owner.commitState(), undefined);
    await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE')); assert.equal(replacementRequests(f).length, 1);
  }
});

test('lost applied response keeps original page and preview until explicit exact reconciliation, with no replay', async () => {
  const f = await replacementReady({ request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/replace')) throw unknownReplacementError(); return result; } }), old = clone(f.target);
  await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
  assert.deepEqual(clone(f.target), old); assert(Object.isFrozen(f.owner.pendingReplacement()));
  assert.deepEqual({ ...f.owner.pendingReplacement() }, { state: 'uncertain', sessionId: SESSION, fileId: FILE, expectedVersion: f.current.version });
  const count = mutations(f).length;
  for (const run of [() => f.owner.request(`/upload/${SESSION}`, 'DELETE'), () => f.owner.request(`/upload/${SESSION}`, 'PATCH', { version: f.current.version, draft: {}, page_order: [FILE, SECOND_FILE] }), () => f.owner.request(`/upload/${SESSION}/commit`, 'POST', { termsAccepted: true, draft: { language: 'en' }, page_order: [FILE, SECOND_FILE] }), () => f.api.runUploadBatch(f.current, [localPage()], f.owner), () => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner), () => f.api.saveResume(f.current, adapter.snapshot({}, f.current.relationships.files), f.owner)]) await assert.rejects(run);
  assert.equal(mutations(f).length, count);
  const restored = await f.api.currentUpload(f.owner); assert.equal(restored.can_mutate, false); assert.equal(restored.can_cancel, false); assert.equal(restored.can_retry_commit, false); assert(f.owner.pendingReplacement());
  const result = await f.api.reconcileUploadReplacement(f.owner);
  assert.equal(result.file.id, FILE); assert.equal(result.file.original_file_name, 'replacement.png'); assert.equal(result.session.can_mutate, true); assert.equal(f.owner.pendingReplacement(), undefined);
  assert.deepEqual(clone(f.target), old); assert.equal(replacementRequests(f).length, 1);
});

test('unknown unchanged pre-version and lost identical no-op cannot establish delayed request completion', async () => {
  for (const noOp of [false, true]) {
    const f = await replacementReady({ request: async (request, next) => { if (request.url.endsWith('/replace')) { if (noOp) await next(request); throw unknownReplacementError(); } return next(request); } }), old = clone(f.target);
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, noOp ? replacementImage('page.png', PNG) : replacementImage(), f.owner));
    await assert.rejects(() => f.api.reconcileUploadReplacement(f.owner)); await assert.rejects(() => f.api.reconcileUploadReplacement(f.owner));
    assert(f.owner.pendingReplacement()); assert.deepEqual(clone(f.target), old); assert.equal(replacementRequests(f).length, 1);
  }
});

test('reconciliation rejects changed neighbor, order, draft, target, file evidence, version and missing session', async () => {
  for (const change of [row => row.files[1].fileHash = 'e'.repeat(64), row => row.pageOrder.reverse(), row => row.draft = { title: 'Concurrent draft' }, row => row.mangaId = SECOND_SESSION, row => row.files[0].originalFileName = 'ambiguous.png', row => row.files[0].fileHash = 'e'.repeat(64), row => row.files[0].fileSize++, row => row.files[0].mimeType = 'image/jpeg', row => row.version++, (_row, state) => state.sessions.delete(SESSION)]) {
    const f = await replacementReady({ request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/replace')) throw unknownReplacementError(); return result; } });
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
    change(f.state.sessions.get(SESSION), f.state); await assert.rejects(() => f.api.reconcileUploadReplacement(f.owner));
    assert(f.owner.pendingReplacement()); assert.equal(replacementRequests(f).length, 1);
  }
});

test('replacement fence persists across a fresh adapter and account reload, stores only bounded facts, and restores exact read-only session', async () => {
  const f = await replacementReady({ request: async (request, next) => { const result = await next(request); if (request.url.endsWith('/replace')) throw unknownReplacementError(); return result; } });
  await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
  const stored = f.storage.get(replacementStorageKey); assert(stored); assert(stored.length < 320 * 1024); assert.doesNotMatch(stored, /blob:|arrayBuffer|thumbnail|privateKey|secret|authorization/);
  const g = fixture({ storage: f.storage, state: f.state }), { owner } = await g.owner();
  assert.equal(owner.pendingReplacement().sessionId, SESSION); const current = await g.api.currentUpload(owner);
  assert.equal(current.can_mutate, false); assert.equal(g.requests.some(request => request.url === `${PUBLIC}/api/v1/upload?expectedAccountId=${USER}`), false);
  await assert.rejects(() => owner.request(`/upload/${SESSION}`, 'DELETE'));
  const result = await g.api.reconcileUploadReplacement(owner); assert.equal(result.file.id, FILE); assert.equal(result.session.can_mutate, true); assert.equal(owner.pendingReplacement(), undefined);
  assert.equal(replacementRequests(g).length, 0);
});

test('validation failures, stale input and affirmative unsent transports do not leave a replacement fence', async () => {
  for (const file of [replacementImage('bad/name.png'), { ...replacementImage(), type: 'image/jpg' }, { ...replacementImage(), size: 0 }]) {
    const f = await replacementReady(); await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, file, f.owner)); assert.equal(f.owner.pendingReplacement(), undefined); assert.equal(replacementRequests(f).length, 0);
  }
  const stale = await replacementReady(); stale.state.sessions.get(SESSION).version++;
  await assert.rejects(() => stale.api.replaceUploadPage(stale.current, stale.target, replacementImage(), stale.owner)); assert.equal(stale.owner.pendingReplacement(), undefined); assert.equal(replacementRequests(stale).length, 0);
  const unsent = await replacementReady({ request: async (request, next) => { if (request.url.endsWith('/replace')) throw Object.assign(Error('Explicit finite signer denial'), { outcome: 'unsent' }); return next(request); } });
  await assert.rejects(() => unsent.api.replaceUploadPage(unsent.current, unsent.target, replacementImage(), unsent.owner)); assert.equal(unsent.owner.pendingReplacement(), undefined); assert.equal(unsent.storage.size, 0);
});

test('replacement rechecks local target/file/session and caller identity after async validation', async () => {
  for (const change of [(f, file) => file.name = 'changed.png', (f, file) => file.type = 'image/jpeg', (f, file) => file.size++, f => f.target.file_hash = 'd'.repeat(64), f => f.current.version++, f => f.current.page_order.reverse(), f => f.state.generation++, f => f.owner.dispose()]) {
    const f = await replacementReady(), file = replacementImage(), read = file.arrayBuffer; file.arrayBuffer = async () => { const bytes = await read(); change(f, file); return bytes; };
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, file, f.owner)); assert.equal(replacementRequests(f).length, 0); assert.equal(f.storage.size, 0);
  }
  const f = await replacementReady(); let current = true; const file = replacementImage(), read = file.arrayBuffer; file.arrayBuffer = async () => { const bytes = await read(); current = false; return bytes; };
  await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, file, f.owner, () => { if (!current) throw Error('Finite page selector changed'); })); assert.equal(replacementRequests(f).length, 0); assert.equal(f.storage.size, 0);
});

test('replacement and unresolved commit fences exclude each other without altering commit recovery', async () => {
  const f = await ready({ request: async (request, next) => { if (request.url.endsWith('/commit')) throw unknownReplacementError(); return next(request); } }), { page, saved, payload } = await stagedCommit(f);
  await assert.rejects(() => f.owner.request(`/upload/${SESSION}/commit`, 'POST', payload));
  await assert.rejects(() => f.api.replaceUploadPage(saved, page, replacementImage(), f.owner));
  assert.equal(replacementRequests(f).length, 0); assert.equal(f.owner.pendingReplacement(), undefined); assert.equal(f.owner.commitState(), 'uncertain');
});

test('a newer tab fence is never cleared by the older request refusal or unsent callback', async () => {
  for (const unsent of [false, true]) {
    const f = await replacementReady({ request: async (request, next) => {
      if (!request.url.endsWith('/replace')) return next(request);
      const pending = JSON.parse(f.storage.get(replacementStorageKey)); pending.attempt = 'different-tab-attempt'; f.storage.set(replacementStorageKey, JSON.stringify(pending));
      if (unsent) throw Object.assign(Error('Explicit finite unsent after another tab write'), { outcome: 'unsent' });
      return refusal(403);
    } });
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
    assert.equal(JSON.parse(f.storage.get(replacementStorageKey)).attempt, 'different-tab-attempt');
    await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE'));
  }
});

test('queued DELETE is blocked at callback time when another owner starts an unresolved replacement', { timeout: 2000 }, async () => {
  let entered, release; const queued = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const f = await replacementReady({ capabilities: async state => { if (state.queueDelete && state.preparing?.method === 'DELETE') { entered(); await gate; } }, request: async (request, next) => { if (request.url.endsWith('/replace')) throw unknownReplacementError(); return next(request); } });
  const user = await f.api.loadUploadAccount(), second = f.api.publicUploadOwner(() => MANGA, user), otherSession = await f.api.currentUpload(second), otherPage = otherSession.relationships.files[0];
  f.state.queueDelete = true; const deletion = f.owner.request(`/upload/${SESSION}`, 'DELETE'); await queued;
  await assert.rejects(() => f.api.replaceUploadPage(otherSession, otherPage, replacementImage(), second)); release(); await assert.rejects(() => deletion);
  assert.equal(f.requests.filter(request => request.method === 'DELETE').length, 0); assert(second.pendingReplacement());
});

test('old preview decoding callback cannot return stale bytes after a confirmed replacement', { timeout: 2000 }, async () => {
  let entered, release; const decoding = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const f = await replacementReady({ dependencies: { decodeImage: async () => { entered(); await gate; return { width: 2, height: 3 }; } } });
  const oldPreview = f.api.uploadPreview(f.owner, f.current, f.target); await decoding;
  await f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner); release(); await assert.rejects(() => oldPreview);
  assert.equal(f.target.thumbnail, 'blob:explicit-original-page'); assert.equal(f.target.original_file_name, 'page.png');
});

test('successful original and explicit reconciliation restore the proven returned snapshot authority after concurrent exact read swaps latest', async () => {
  for (const lost of [false, true]) {
    let f;
    f = await replacementReady({ dependencies: { digest: async bytes => { if (f?.state.swapLatestDuringProof) { f.state.swapLatestDuringProof = false; f.state.latestSwapped = true; await f.api.currentUpload(f.owner); } return digest(bytes); } }, request: async (request, next, state) => {
      const result = await next(request);
      if (lost && request.url.endsWith('/replace')) throw unknownReplacementError();
      if (state.replaceCount && !state.latestSwapped && request.method === 'GET' && request.url === `${PUBLIC}/api/v1/nostr/uploads/${SESSION}`) state.swapLatestDuringProof = true;
      return result;
    } });
    f.state.sessions.get(SESSION).canCancel = false;
    let result;
    if (lost) { await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner)); result = await f.api.reconcileUploadReplacement(f.owner); }
    else result = await f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner);
    assert.equal(f.state.latestSwapped, true); assert.equal(f.owner.pendingReplacement(), undefined);
    assert.equal(result.session.can_mutate, true); assert.equal(result.session.can_cancel, false); assert.equal(result.session.can_retry_commit, true);
  }
});

test('replacement callback-time signer, service, origin and input guards block dispatch and clear an affirmative unsent fence', async () => {
  for (const change of [(f, _file) => f.state.generation++, f => f.state.policy.service_key = 'e'.repeat(64), f => f.state.publicOrigin = RELAY, f => f.target.file_hash = 'e'.repeat(64), (f, file) => file.name = 'late.png']) {
    let f, file;
    f = await replacementReady({ capabilities: async state => { if (state.preparing?.url.endsWith('/replace')) change(f, file); } }); file = replacementImage();
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, file, f.owner));
    assert.equal(replacementRequests(f).length, 0); assert.equal(f.storage.size, 0);
  }
});

test('exclusive replacement clear rechecks another tab install queued before it can delete the fence', { timeout: 2000 }, async () => {
  let entered, release, calls = 0; const clearing = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const f = await replacementReady({ dependencies: { replacementLock: async (_key, action) => { if (++calls === 2) { entered(); await gate; } return action(); } } });
  const operation = f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner); await clearing;
  const pending = JSON.parse(f.storage.get(replacementStorageKey)); pending.attempt = 'newer-tab-before-clear'; f.storage.set(replacementStorageKey, JSON.stringify(pending));
  release(); await assert.rejects(() => operation);
  assert.equal(JSON.parse(f.storage.get(replacementStorageKey)).attempt, 'newer-tab-before-clear'); assert.equal(replacementRequests(f).length, 1);
  await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE'));
});

test('queued exclusive install rechecks current file identity before persisting or dispatching', { timeout: 2000 }, async () => {
  let entered, release; const installing = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const f = await replacementReady({ dependencies: { replacementLock: async (_key, action) => { entered(); await gate; return action(); } } });
  const file = replacementImage(), operation = f.api.replaceUploadPage(f.current, f.target, file, f.owner); await installing;
  file.name = 'changed-while-lock-queued.png'; release(); await assert.rejects(() => operation);
  assert.equal(f.storage.size, 0); assert.equal(replacementRequests(f).length, 0);
});

test('missing cross-tab replacement lock fails closed before fence install or POST', async () => {
  const f = await replacementReady({ dependencies: { replacementLock: async () => { throw Error('Explicit finite unavailable Web Lock'); } } });
  await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
  assert.equal(f.storage.size, 0); assert.equal(replacementRequests(f).length, 0); assert.equal(f.owner.pendingReplacement(), undefined);
});

test('actual replacement POST200 followed by an affirmative-unsent exact GET retains its replacement fence', async () => {
  const f = await replacementReady({ capabilities: async state => { if (state.replaceCount && state.preparing?.url === `${PUBLIC}/api/v1/nostr/uploads/${SESSION}`) throw Error('Finite follow-up GET failed pre-dispatch'); } }), old = clone(f.target);
  await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
  assert.equal(f.state.replaceCount, 1); assert(f.owner.pendingReplacement()); assert.deepEqual(clone(f.target), old); assert.equal(replacementRequests(f).length, 1);
  await assert.rejects(() => f.owner.request(`/upload/${SESSION}`, 'DELETE'));
});

test('actual replacement POST200 followed by unsent-shaped caller or proof errors cannot clear the replacement fence', async () => {
  for (const failure of ['caller', 'proof']) {
    let f;
    f = await replacementReady({
      afterTransport: (request, state) => { if (request.url.endsWith('/replace')) state.originalPostReturned = true; },
      dependencies: { digest: async bytes => { if (failure === 'proof' && f?.state.originalPostReturned) throw Object.assign(Error('Finite later proof error'), { outcome: 'unsent' }); return digest(bytes); } },
    });
    const old = clone(f.target);
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner, () => { if (failure === 'caller' && f.state.originalPostReturned) throw Object.assign(Error('Finite later caller callback error'), { outcome: 'unsent' }); }));
    assert.equal(f.state.replaceCount, 1); assert(f.owner.pendingReplacement()); assert.deepEqual(clone(f.target), old); assert.equal(replacementRequests(f).length, 1);
  }
});

test('busy bounded lock admission prevents install, while busy clear retains an already dispatched replacement fence', async () => {
  for (const busyAt of [1, 2]) {
    let calls = 0;
    const f = await replacementReady({ dependencies: { replacementLock: async (_key, action) => { if (++calls === busyAt) throw Error('Explicit finite busy lock admission'); return action(); } } });
    await assert.rejects(() => f.api.replaceUploadPage(f.current, f.target, replacementImage(), f.owner));
    assert.equal(replacementRequests(f).length, busyAt === 1 ? 0 : 1);
    assert.equal(!!f.owner.pendingReplacement(), busyAt === 2);
    assert.equal(f.storage.size, busyAt === 1 ? 0 : 1);
  }
});

const GROUP = uuid('9');
test('begin and edit-begin bind generated JSON to the captured native account, never caller overrides', async () => {
  const f = fixture(), { owner } = await f.owner();
  await owner.request('/upload/begin', 'POST', { manga: MANGA, groups: [], expectedAccountId: SECOND_SESSION });
  const begin = f.requests.find(row => row.url.endsWith('/upload/begin'));
  assert.deepEqual(JSON.parse(new TextDecoder().decode(begin.body)), { manga: MANGA, groups: [], expectedAccountId: USER });
  const g = fixture({ request: (request, next, state) => {
    if (new URL(request.url).pathname === '/api/v1/upload/begin/' + CHAPTER) {
      const current = session(); current.chapterId = CHAPTER; state.sessions.set(SESSION, current); state.current = SESSION;
      return response({ id: SESSION, type: 'upload_session', attributes: { version: 1, isCommitted: false, isProcessed: false, isDeleted: false } });
    }
    return next(request);
  } });
  const user = await g.api.loadUploadAccount(), edit = g.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
  await edit.request('/upload/begin/' + CHAPTER, 'POST', { version: 2, expectedAccountId: SECOND_SESSION });
  const sent = g.requests.find(row => row.url.endsWith('/upload/begin/' + CHAPTER));
  assert.deepEqual(JSON.parse(new TextDecoder().decode(sent.body)), { version: 2, expectedAccountId: USER });
});
test('stale captured-account discovery sends its exact signed query and cannot adopt a foreign session after409', async () => {
  const f = fixture({ request: (request, next) => {
    const url = new URL(request.url);
    if (url.pathname === '/api/v1/upload' && request.method === 'GET') {
      assert.equal(url.search, '?expectedAccountId=' + USER); assert.equal(request.body, undefined);
      return refusal(409, 'Actual eligible native-account binding changed');
    }
    return next(request);
  } });
  const { owner } = await f.owner(); f.account.user.id = SECOND_SESSION;
  const foreign = session(SECOND_SESSION); f.state.sessions.set(SECOND_SESSION, foreign); f.state.current = SECOND_SESSION;
  await assert.rejects(() => f.api.currentUpload(owner), error => error.body === 'Actual eligible native-account binding changed');
  await assert.rejects(() => owner.request('/upload/' + SECOND_SESSION, 'DELETE'));
  assert.equal(mutations(f).length, 0); assert.equal(foreign.state, 'open');
  assert.equal(f.requests.filter(row => row.method === 'GET' && new URL(row.url).pathname.includes('/nostr/uploads/')).length, 0);
});
test('stale captured-account chapter and begin commands retain captured intent on a definitive409 without replay', async () => {
  for (const operation of ['begin', 'metadata', 'claim', 'remove']) {
    const f = fixture({ account: value => value.user.permissions.push('self.chapter.remove'), request: (request, next) => {
      if (request.method === 'GET') return next(request);
      const url = new URL(request.url);
      if (request.body) assert.equal(JSON.parse(new TextDecoder().decode(request.body)).expectedAccountId, USER);
      else assert.equal(url.search, '?expectedAccountId=' + USER + (operation === 'remove' ? '&removalMode=request-review' : ''));
      return refusal(409, 'Actual eligible native-account binding changed');
    } });
    const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => operation === 'begin' ? MANGA : CHAPTER, user, operation === 'begin' ? 'manga' : 'chapter');
    f.account.user.id = SECOND_SESSION;
    const result = operation === 'begin' ? await owner.request('/upload/begin', 'POST', { manga: MANGA, groups: [] })
      : operation === 'metadata' ? await owner.request('/chapter/' + CHAPTER, 'PUT', { version: 2, title: 'Captured A' })
      : await owner.request('/chapter/' + CHAPTER + (operation === 'claim' ? '/claim' : ''), operation === 'claim' ? 'POST' : 'DELETE', operation === 'claim' ? undefined : { removalMode: 'request-review' });
    assert.equal(result.status, 409); assert.equal(mutations(f).length, 1); assert.equal(f.state.sessions.size, 0);
  }
});
test('chapter metadata uses exact signed target and official fields, never upload/native fallback', async () => {
  const wire = chapterResource(3);
  const f = fixture({ request: (request, next) => request.url.endsWith('/chapter/' + CHAPTER)
    ? response({ result: 'ok', response: 'entity', data: wire }) : next(request) });
  const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
  const updated = await owner.request('/chapter/' + CHAPTER, 'PUT', { version: 2, title: '', language: 'es', groups: [GROUP] });
  assert.equal(updated.status, 200); assert.equal(updated.body.data.id, CHAPTER);
  const sent = f.requests.at(-1); assert.equal(sent.method, 'PUT'); assert.equal(sent.url, PUBLIC + '/api/v1/chapter/' + CHAPTER);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(sent.body)), { version: 2, expectedAccountId: USER, title: '', translatedLanguage: 'es', groups: [GROUP] });
  const count = mutations(f).length;
  for (const [route, method, payload] of [['/chapter/' + SECOND_SESSION, 'PUT', { version: 2 }], ['/chapter/' + CHAPTER + '?x=1', 'DELETE'], ['/chapter/' + CHAPTER, 'POST'], ['/chapter/' + CHAPTER + '/claim', 'DELETE'], ['/chapter/' + CHAPTER, 'PUT', { version: 2, data: [] }], ['/chapter/' + CHAPTER, 'PUT', { version: 2, groups: [GROUP, GROUP] }], ['/chapter/' + CHAPTER + '/claim', 'POST', {}]]) await assert.rejects(() => owner.request(route, method, payload));
  assert.equal(mutations(f).length, count);
});
test('chapter claims have no body and ordinary removal keeps actual durable moderation receipt', async () => {
  const f = fixture({ account: account => account.user.permissions.push('self.chapter.remove'), request: (request, next) => {
    if (new URL(request.url).pathname.endsWith('/chapter/' + CHAPTER + '/claim')) return response({ result: 'ok', response: 'entity', data: chapterResource(3) });
    if (new URL(request.url).pathname.endsWith('/chapter/' + CHAPTER)) { const result = response({ result: 'ok', status: 'pending-moderation', instanceRemoval: 'pending-moderation', sourceDeletionStatus: 'instance-local-only', requestId: SECOND_SESSION, sourceDeletionRequestId: null }, 202); result.headers['x-weebdex-instance-removal'] = 'pending-moderation'; result.headers['x-weebdex-source-deletion-status'] = 'instance-local-only'; return result; }
    return next(request);
  } });
  const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
  await owner.request('/chapter/' + CHAPTER + '/claim', 'POST'); assert.equal(f.requests.at(-1).body, undefined); assert.equal(f.requests.at(-1).url, PUBLIC + '/api/v1/chapter/' + CHAPTER + '/claim?expectedAccountId=' + USER);
  const pending = await owner.request('/chapter/' + CHAPTER, 'DELETE', { removalMode: 'request-review' }); assert.equal(pending.status, 202); assert.equal(pending.body.instanceRemoval, 'pending-moderation'); assert.equal(pending.body.requestId, SECOND_SESSION); assert.equal(f.requests.at(-1).body, undefined); assert.equal(f.requests.at(-1).url, PUBLIC + '/api/v1/chapter/' + CHAPTER + '?expectedAccountId=' + USER + '&removalMode=request-review');
});
test('chapter removal requires a closed explicit effect and refuses ordinary approval before dispatch', async () => {
  const f = fixture({ account: value => value.user.permissions.push('self.chapter.remove') });
  const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
  for (const payload of [undefined, {}, { removalMode: null }, { removalMode: false }, { removalMode: ['request-review'] }, { removalMode: { toString: () => 'request-review' } }, { removalMode: 'delete' }, { removalMode: 'approve-hide' }, { removalMode: 'request-review', expectedAccountId: SECOND_SESSION }]) {
    await assert.rejects(() => owner.request('/chapter/' + CHAPTER, 'DELETE', payload));
  }
  await assert.rejects(() => owner.request('/chapter/' + CHAPTER + '/claim', 'POST', { removalMode: 'request-review' }));
  await assert.rejects(() => owner.request('/chapter/' + CHAPTER, 'PUT', { version: 2, removalMode: 'request-review' }));
  assert.equal(mutations(f).length, 0);
});
test('captured chapter review stays an empty-body signed review when the same server account gains moderator grants', async () => {
  const f = fixture({ account: value => value.user.permissions.push('self.chapter.remove'), request: (request, next) => {
    if (request.method === 'GET') return next(request);
    assert.equal(request.url, PUBLIC + '/api/v1/chapter/' + CHAPTER + '?expectedAccountId=' + USER + '&removalMode=request-review');
    assert.equal(request.body, undefined);
    const result = response({ result: 'ok', status: 'pending-moderation', instanceRemoval: 'pending-moderation', sourceDeletionStatus: 'instance-local-only', requestId: SECOND_SESSION, sourceDeletionRequestId: null }, 202);
    result.headers['x-weebdex-instance-removal'] = 'pending-moderation'; result.headers['x-weebdex-source-deletion-status'] = 'instance-local-only'; return result;
  } });
  const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
  f.account.user.roles = ['moderator']; f.account.user.permissions.push('chapter.remove');
  const result = await owner.request('/chapter/' + CHAPTER, 'DELETE', { removalMode: 'request-review' });
  assert.equal(result.status, 202); assert.equal(mutations(f).length, 1);
});
test('chapter removal refuses a different202 or204 effect even with valid actual receipt headers', async () => {
  for (const mode of ['request-review', 'approve-hide']) {
    const f = fixture({ account: value => { value.user.roles = ['moderator']; value.user.permissions.push('chapter.remove'); }, request: (request, next) => {
      if (request.method === 'GET') return next(request);
      const result = mode === 'request-review' ? response(undefined, 204) : response({ result: 'ok', status: 'pending-moderation', instanceRemoval: 'pending-moderation', sourceDeletionStatus: 'instance-local-only', requestId: SECOND_SESSION, sourceDeletionRequestId: null }, 202);
      result.headers['x-weebdex-instance-removal'] = mode === 'request-review' ? 'soft-hidden' : 'pending-moderation'; result.headers['x-weebdex-source-deletion-status'] = 'instance-local-only'; return result;
    } });
    const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
    await assert.rejects(() => owner.request('/chapter/' + CHAPTER, 'DELETE', { removalMode: mode }));
    assert.equal(mutations(f).length, 1); assert.equal(f.requests.at(-1).body, undefined);
  }
});
test('captured moderator approval signs only its explicit effect and confirms exact204 headers', async () => {
  const f = fixture({ account: value => { value.user.roles = ['moderator']; value.user.permissions.push('chapter.remove'); }, request: (request, next) => {
    if (request.method === 'GET') return next(request);
    assert.equal(request.url, PUBLIC + '/api/v1/chapter/' + CHAPTER + '?expectedAccountId=' + USER + '&removalMode=approve-hide');
    const result = response(undefined, 204); result.headers['x-weebdex-instance-removal'] = 'soft-hidden'; result.headers['x-weebdex-source-deletion-status'] = 'instance-local-only'; return result;
  } });
  const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
  const result = await owner.request('/chapter/' + CHAPTER, 'DELETE', { removalMode: 'approve-hide' });
  assert.equal(result.status, 204); assert.equal(f.requests.at(-1).body, undefined); assert.equal(mutations(f).length, 1);
});
test('chapter write results are exact-target, identity-fenced and never automatically replayed', async () => {
  for (const change of ['wrong-id', 'wrong-envelope', 'unavailable-body', 'scope', 'disposed']) {
    let owner;
    const f = fixture({ request: async (request, next, state) => { if (!request.url.includes('/chapter/')) return next(request); if (change === 'unavailable-body') throw Object.assign(Error('Explicit uncertain write'), { outcome: 'uncertain' }); if (change === 'scope') state.generation++; if (change === 'disposed') owner.dispose(); return response({ result: 'ok', response: change === 'wrong-envelope' ? 'collection' : 'entity', data: { ...chapterResource(3), id: change === 'wrong-id' ? SECOND_SESSION : CHAPTER } }); } });
    const user = await f.api.loadUploadAccount(); owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
    await assert.rejects(() => owner.request('/chapter/' + CHAPTER, 'PUT', { version: 2, title: 'Exact' }));
    assert.equal(mutations(f).length, 1, change);
  }
});
test('chapter removal cannot invent pending moderation or successful hide from status alone', async () => {
  for (const result of [response({ result: 'ok', status: 'pending-moderation', instanceRemoval: 'pending-moderation', sourceDeletionStatus: 'instance-local-only', requestId: SECOND_SESSION, sourceDeletionRequestId: null }, 202), response(undefined, 204)]) {
    const f = fixture({ account: account => account.user.permissions.push('self.chapter.remove'), request: (request, next) => request.url.includes('/chapter/') ? result : next(request) });
    const user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => CHAPTER, user, 'chapter');
    await assert.rejects(() => owner.request('/chapter/' + CHAPTER, 'DELETE', { removalMode: 'request-review' })); assert.equal(mutations(f).length, 1);
  }
});


// Finite stale-read controls: the fake server snapshot is captured before an
// explicit confirmed cancel; delivery or group hydration alone is delayed.
// No network or real server/preview acceptance is represented by these gates.
for (const phase of ['snapshot', 'group']) for (const beginAnother of [false, true]) test('preview exact read cannot revive a confirmed cancelled owner session; ' + phase + '; begin another=' + beginAnother, { timeout: 2000 }, async () => {
  let holdNext = false, entered, release;
  const held = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const pause = async () => { holdNext = false; entered(); await gate; };
  const groupId = SECOND_FILE;
  const f = await ready({ request: async (request, next) => {
    const result = await next(request);
    if (phase === 'snapshot' && holdNext && request.method === 'GET' && new URL(request.url).pathname === `/api/v1/nostr/uploads/${SESSION}`) await pause();
    return result;
  }, dependencies: { catalogueRead: async (operation, { id }) => {
    assert.equal(operation, 'group.get'); assert.equal(id, groupId);
    if (phase === 'group' && holdNext) await pause();
    const resource = { id, type: 'scanlation_group', attributes: { name: 'Actual finite selected group', version: 1 }, relationships: [] };
    return response({ id, name: resource.attributes.name, version: 1, public_api: { resource } });
  } } });
  const page = await attach(f);
  if (phase === 'group') f.state.sessions.get(SESSION).groupIds = [groupId];
  holdNext = true;
  const preview = f.api.uploadPreview(f.owner, f.current, page).then(value => ({ ok: true, value }), error => ({ ok: false, error }));
  await held;
  try {
    await f.api.cancelResume(f.current, f.owner);
    if (beginAnother) assert.equal((await f.begin(f.owner)).id, SECOND_SESSION);
  } finally { release(); }
  const result = await preview;
  assert.equal(f.requests.filter(request => request.url.endsWith('/preview')).length, 0, 'A read captured before confirmed cancellation must not dispatch a preview afterward');
  assert.equal(result.ok, false, 'A cancelled session snapshot must not become current again');
  if (beginAnother) assert.equal((await f.api.currentUpload(f.owner)).id, SECOND_SESSION);
  else assert.equal(await f.api.currentUpload(f.owner), undefined);
});

test('delayed preview snapshot remains valid across same-session metadata refresh', { timeout: 2000 }, async () => {
  let holdNext = false, entered, release;
  const held = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const f = await ready({ request: async (request, next) => {
    const result = await next(request);
    if (holdNext && request.method === 'GET' && new URL(request.url).pathname === `/api/v1/nostr/uploads/${SESSION}`) { holdNext = false; entered(); await gate; }
    return result;
  } });
  const page = await attach(f); holdNext = true;
  const preview = f.api.uploadPreview(f.owner, f.current, page);
  await held;
  try { await f.api.saveResume(f.current, { draft: { title: 'Updated finite annotation', language: 'en' }, page_order: [page.id] }, f.owner); }
  finally { release(); }
  assert.equal((await preview).size, PNG.length);
  assert.equal(f.requests.filter(request => request.url.endsWith('/preview')).length, 1);
  assert.equal((await f.api.currentUpload(f.owner)).draft.title, 'Updated finite annotation');
});

test('a newly bound owner can adopt its first exact preview read', async () => {
  const f = await ready(), page = await attach(f), user = await f.api.loadUploadAccount();
  const owner = f.api.publicUploadOwner(() => MANGA, user);
  assert.equal((await f.api.uploadPreview(owner, SESSION, page)).size, PNG.length);
  assert.equal((await f.api.currentUpload(owner)).id, SESSION);
  owner.dispose(); assert.equal(owner.alive(), false);
});

// The undefined→bound→cancelled→undefined cycle must invalidate an earlier
// unbound read even though its final session UUID is equal to its initial one.
for (const read of ['preview-first', 'discovery']) for (const beginAnother of [false, true]) test('unbound ' + read + ' cannot survive a session ownership ABA; begin another=' + beginAnother, { timeout: 2000 }, async () => {
  let holdNext = false, entered, release;
  const held = new Promise(resolve => entered = resolve), gate = new Promise(resolve => release = resolve);
  const f = await ready({ request: async (request, next) => {
    const result = await next(request), route = new URL(request.url).pathname;
    const matches = read === 'preview-first' ? route === `/api/v1/nostr/uploads/${SESSION}` : route === '/api/v1/upload';
    if (holdNext && request.method === 'GET' && matches) { holdNext = false; entered(); await gate; }
    return result;
  } });
  const page = await attach(f), user = await f.api.loadUploadAccount(), owner = f.api.publicUploadOwner(() => MANGA, user);
  holdNext = true;
  const stale = (read === 'preview-first' ? f.api.uploadPreview(owner, SESSION, page) : f.api.currentUpload(owner)).then(value => ({ ok: true, value }), error => ({ ok: false, error }));
  await held;
  try {
    const current = await f.api.currentUpload(owner); assert.equal(current.id, SESSION);
    await f.api.cancelResume(current, owner);
    if (beginAnother) assert.equal((await f.begin(owner)).id, SECOND_SESSION);
  } finally { release(); }
  assert.equal((await stale).ok, false, 'An earlier unbound read must not overwrite the current ownership generation');
  assert.equal(f.requests.filter(request => request.url.endsWith('/preview')).length, 0);
  if (beginAnother) assert.equal((await f.api.currentUpload(owner)).id, SECOND_SESSION);
  else assert.equal(await f.api.currentUpload(owner), undefined);
  owner.dispose(); assert.equal(owner.alive(), false);
});
