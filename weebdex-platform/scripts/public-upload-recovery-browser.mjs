// Genuine disposable UI/HTTP/DB journey. Test-only, opt-in, and local-only.
// This deliberately does not reconstruct the lost transport-proof framework.
import assert from 'node:assert/strict';
import { deflateSync } from 'node:zlib';
import { verifiedSchnorr } from './browser-signed-journeys.mjs';
import { createPublicUploadRecoveryControls } from './public-upload-recovery-controls.mjs';
import { connectPublicUploadKey, createKeyImportStep, uniqueUploadControl } from './public-upload-key-import.mjs';
import { createUploadJourneyProgress } from './public-upload-journey-progress.mjs';
import { captureUploadRequestRefusals, uploadRefusalReaderReady } from './public-upload-transport-refusals.mjs';
import { createUploadIndependentReader, observePublicUpload, uploadDigest } from './public-upload-recovery-observer.mjs';
export const publicUploadRecoveryPhase = 'public-upload-recovery';
export const uploadPreviewRefusalCodes = Object.freeze(['checkpoint', 'fetch', 'response-header', 'response-media', 'declared-size', 'body-read', 'body-budget', 'decode', 'other', 'unavailable']);
export function projectUploadPreviewRefusals(raw) {
  const invalid = { record_valid: false, truncated: false, codes: [] };
  try {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw) || Object.keys(raw).sort().join(',') !== 'codes,count') return invalid;
    const count = raw.count, source = raw.codes;
    if (!Number.isInteger(count) || count < 0 || count > 500 || !Array.isArray(source)) return invalid;
    const length = source.length;
    if (length !== Math.min(count, 17)) return invalid;
    const codes = [];
    for (let index = 0; index < length; index++) {
      const code = source[index];
      if (typeof code !== 'string' || !uploadPreviewRefusalCodes.includes(code)) return invalid;
      codes.push(code);
    }
    return { record_valid: true, truncated: count > 16, codes: codes.slice(0, 16) };
  } catch { return invalid; }
}
export async function captureUploadPreviewRefusals(page) {
  let timer;
  try {
    // One passive snapshot of currently rendered error attributes. No event
    // hook, forced action, response substitution or raw error text is retained.
    const value = await Promise.race([
      page.locator('p[role="status"][data-public-upload-preview-refusal]').evaluateAll(nodes => ({
        count: nodes.length, codes: nodes.slice(0, 17).map(node => node.getAttribute('data-public-upload-preview-refusal')),
      })),
      new Promise(resolve => { timer = setTimeout(() => resolve(null), 1000); }),
    ]);
    return projectUploadPreviewRefusals(value);
  } catch { return { record_valid: false, truncated: false, codes: [] }; }
  finally { if (timer !== undefined) clearTimeout(timer); }
}
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export const uploadCompletionTitles = Object.freeze(['Upload request accepted', 'Chapter added to approval queue', 'Chapter uploaded successfully']);
export function assertUploadRecoveryRoute(value, origin, pathname) {
  const url = new URL(value); assert.equal(url.origin, origin); assert.equal(url.href, origin + pathname); assert.equal(value, url.href);
}
export function assertUploadCompletion(title, session) {
  assert.ok(uploadCompletionTitles.includes(title), 'Upload completion is not an accepted/approval/uploaded result');
  assert.ok(['available', 'unavailable'].includes(session.receiptStatus), 'Committed receipt availability was not observed');
  if (session.receiptStatus === 'unavailable' || session.receipt?.attributes?.isUnavailable === true || ['accepted_pending_projection', 'pending_projection'].includes(session.publicationStatus)) {
    assert.equal(title, 'Upload request accepted', 'An unavailable presentation requires the exact accepted-only UI branch');
  }
  return title;
}
function crc32(bytes) { let value = 0xffffffff; for (const byte of bytes) { value ^= byte; for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ (0xedb88320 & -(value & 1)); } return (value ^ 0xffffffff) >>> 0; }
export function tinyUploadPng(name, width, height, rgb) {
  assert.match(name, /^[a-z0-9-]+\.png$/); assert.ok(width >= 16 && width <= 64 && height >= 16 && height <= 64);
  assert.ok(Array.isArray(rgb) && rgb.length === 3 && rgb.every(value => Number.isInteger(value) && value >= 0 && value <= 255));
  const chunk = (kind, data) => { const type = Buffer.from(kind), size = Buffer.alloc(4), checksum = Buffer.alloc(4); size.writeUInt32BE(data.length); checksum.writeUInt32BE(crc32(Buffer.concat([type, data]))); return Buffer.concat([size, type, data, checksum]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(width); ihdr.writeUInt32BE(height, 4); ihdr[8] = 8; ihdr[9] = 2;
  const scan = Buffer.alloc(height * (1 + width * 3)); for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) for (let c = 0; c < 3; c++) scan[y * (1 + width * 3) + 1 + x * 3 + c] = rgb[c];
  const bytes = Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(scan)), chunk('IEND', Buffer.alloc(0))]); scan.fill(0);
  assert.ok(bytes.length <= 512); return { name, mimeType: 'image/png', buffer: bytes, width, height, sha256: uploadDigest(bytes) };
}
export function validateDurableUpload(value, { account, sessionId, state = 'open', draft, images, order }) {
  assert.equal(value.fixture_only, true); assert.equal(value.scenario, 'public-upload-v1'); assert.equal(value.pubkey, account.pubkey);
  for (const kind of ['user', 'group', 'manga']) assert.deepEqual(value[kind], account[kind]);
  const s = value.session; assert.equal(s.id, sessionId); assert.equal(s.state, state); assert.ok(Number.isSafeInteger(s.version) && s.version >= 1);
  if (draft) for (const [key, expected] of Object.entries(draft)) assert.equal(s.draft?.[key], expected, `Durable draft ${key}`);
  assert.equal(s.page_order_source, 'effective_stored_plus_current_files');
  if (order) assert.deepEqual(s.stored_page_order, order, 'Raw persisted order, without normalization');
  assert.equal(s.files.length, images.length);
  for (const image of images) { const matches = s.files.filter(file => file.original_file_name === image.name); assert.equal(matches.length, 1); const file = matches[0]; assert.match(file.id, uuid); assert.equal(file.file_hash, image.sha256); assert.equal(file.storage_sha256, image.sha256); assert.equal(file.file_size, image.buffer.length); assert.equal(file.storage_bytes, image.buffer.length); assert.equal(file.fresh_original_read, true); }
  return s;
}
export function assertReplacementMutation(rows, { sessionId, fileId }) {
  assert.match(sessionId, uuid); assert.match(fileId, uuid); assert.equal(rows.length, 1, 'Exactly one replacement mutation required');
  const row = rows[0]; assert.equal(row.method, 'POST'); assert.equal(row.path, `/api/v1/nostr/uploads/${sessionId}/files/${fileId}/replace`);
  assert.equal(row.status, 200); assert.equal(row.native, 'finished'); return row;
}
// Normal-success fixture contract. Compare parsed DTO values independently of
// wire serialization, preserving the exact target ID, order and all neighbours.
export function assertReplacementReadback(before, after, { fileId, image }) {
  assert.match(fileId, uuid); assert.equal(before.state, 'open'); assert.equal(before.canMutate, true);
  const matches = before.files.filter(file => file.id === fileId); assert.equal(matches.length, 1);
  const old = matches[0]; assert.notEqual(old.fileHash, image.sha256, 'This positive fixture replaces genuinely different bytes');
  assert.equal(after.version, before.version + 1);
  assert.ok(Number.isFinite(Date.parse(before.updatedAt)) && Number.isFinite(Date.parse(after.updatedAt)) && Date.parse(after.updatedAt) >= Date.parse(before.updatedAt));
  const file = { ...old, originalFileName: image.name, fileHash: image.sha256, fileSize: image.buffer.length, mimeType: image.mimeType, dimensions: { width: image.width, height: image.height } };
  const expected = { ...before, version: before.version + 1, updatedAt: after.updatedAt, files: before.files.map(row => row.id === fileId ? file : row) };
  assert.deepEqual(after, expected, 'Replacement changed target identity, metadata, order, neighbour or authority');
  return file;
}
export async function runPublicUploadRecovery(h) {
  const { page, context, fixture, origin, report, step, save, insist, redactions, runnerStage } = h;
  assert.equal(typeof runnerStage, 'function', 'Fixed runner-stage recorder required');
  runnerStage('public-upload-fixture');
  const declaration = fixture.public_upload; assert.equal(declaration?.enabled, true); assert.equal(declaration.fixture_only, true); assert.equal(declaration.scenario, 'public-upload-v1');
  const sink = new URL(fixture.mail_origin); assert.equal(sink.protocol, 'http:'); assert.ok(['127.0.0.1', 'localhost'].includes(sink.hostname)); assert.ok(sink.port && !sink.username && !sink.password && sink.pathname === '/' && !sink.search && !sink.hash);
  runnerStage('public-upload-signer');
  const schnorr = await verifiedSchnorr(insist); let secret = schnorr.utils.randomSecretKey(), secretHex = Buffer.from(secret).toString('hex');
  redactions.add(secretHex); const owner = Buffer.from(schnorr.getPublicKey(secret)).toString('hex');
  const verify = (signature, id, key) => schnorr.verify(Buffer.from(signature, 'hex'), Buffer.from(id, 'hex'), Buffer.from(key, 'hex'));
  const sign = async template => { assert.ok(secret); const id = uploadDigest(JSON.stringify([0, owner, template.created_at, template.kind, template.tags, template.content])); return { ...template, pubkey: owner, id, sig: Buffer.from(schnorr.sign(Buffer.from(id, 'hex'), secret)).toString('hex') }; };
  runnerStage('public-upload-observers');
  const observer = observePublicUpload(page, { origin, owner, service: fixture.service_key, verify });
  const reader = createUploadIndependentReader({ origin, owner, service: fixture.service_key, sign });
  const result = report.public_upload = { scope: 'reconstructed-public-upload-replacement-v1', completed: false, supported_replacement: true, chapter_edit: 'unimplemented',
    fixture_control_calls: 0, key_import: [], journey_progress: [], durable: [], replacement: null, preview_decodes: [], preview_transport_refusals: { record_valid: false, truncated: false, codes: [] }, transport_refusal_captures: [], transport_refusal_reader_checks: 0, browser_http: null, independent_http: null, ui_controls: [], actual_context_closed: false,
    limitations: ['Immediate image decode can use an acknowledged local File; signed-preview proof requires hard reload and real key reimport.', 'Publication acceptance and readable catalogue projection are checked separately.', 'No full40 or lost-source equivalence claim.'] };
  let account, sessionId, lastVersion = 0;
  const progress = createUploadJourneyProgress(result.journey_progress, save), mark = stage => progress.mark(stage);
  let nextCheck = 0;
  const images = [tinyUploadPng('01-first.png',24,32,[71,111,153]),tinyUploadPng('02-remove.png',26,34,[157,69,83]),tinyUploadPng('03-last.png',28,36,[63,147,91])];
  const cancelImage = tinyUploadPng('cancel.png',22,30,[120,87,143]);
  const replacementImage = tinyUploadPng('04-replacement.png',30,38,[91,58,173]);
  const healthy = () => { observer.assertHealthy(); assert.equal(report.page_errors.length, 0, 'Application page error'); };
  async function until(read, valid, label, milliseconds = 15000) { const deadline = Date.now() + milliseconds; for (;;) { healthy(); if (Date.now() >= deadline) throw Object.assign(Error(label), { code: 'UPLOAD_UI_READINESS_TIMEOUT' }); const value = await read(); healthy(); if (Date.now() >= deadline) throw Object.assign(Error(label), { code: 'UPLOAD_UI_READINESS_TIMEOUT' }); if (valid(value)) return value; await page.waitForTimeout(100); } }
  const unique = locator => uniqueUploadControl(locator, until);
  const button = async (scope, name) => { const target = await unique(scope.getByRole('button', { name, exact: true })); await until(() => target.isEnabled(), Boolean, `Enabled ${name}`); return target; };
  const pathname = () => `/title/${account.manga.id}/upload`;
  const assertUploadRoute = () => { assertUploadRecoveryRoute(page.url(), origin, pathname()); healthy(); };
  const controls = () => createPublicUploadRecoveryControls(page, { origin, pathname: pathname(), assertHealthy: healthy, timeoutMs: 15000 });
  const panel = async () => (await controls().panel());
  async function run(name, action) {
    const checkIndex = nextCheck++;
    await step(name, async () => {
      try { progress.begin(checkIndex); const value = await action(); progress.passed(); return value; }
      catch (error) { try { progress.failed(error); } catch { /* Keep the original action/save failure. */ } throw error; }
    });
    assert.equal(report.steps.at(-1).status, 'passed', `Stopped after ${name}`);
  }
  async function control(action, body) { assert.ok(result.fixture_control_calls < 28); result.fixture_control_calls++; const endpoint = action === 'account' ? '/fixture/protocol/upload-account' : '/fixture/protocol/upload-snapshot';
    const response = await context.request.post(sink.origin + endpoint, { data: body, maxRedirects: 0, timeout: 20000 }); assert.equal(response.status(), 200); const value = await response.json(); assert.equal(value.fixture_only, true); assert.equal(value.scenario, 'public-upload-v1'); return value; }
  const current = async () => { mark('current-read'); const value = await reader.get('/api/v1/upload'); assert.equal(value.status, 200); assert.match(value.body.id, uuid); return value.body.id; };
  async function exact() { mark('exact-read'); assert.match(sessionId, uuid); const value = await reader.get(`/api/v1/nostr/uploads/${sessionId}`); assert.equal(value.status, 200); assert.equal(value.body.result, 'ok'); assert.equal(value.body.session.id, sessionId); assert.equal(value.body.session.mangaId, account.manga.id); assert.deepEqual(value.body.session.groupIds, [account.group.id]); return value.body.session; }
  async function completeWrites(start, count) { mark('mutation-count'); await until(() => observer.rows.filter(row => row.method !== 'GET').length, value => value >= start + count, 'Expected genuine UI mutation'); mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Browser request terminals'); assert.equal(observer.mutationCount(), start + count, 'Unexpected or missing UI mutation'); for (const row of observer.rows.filter(row => row.method !== 'GET').slice(start)) assert.equal(row.status, 200, 'Natural-success UI mutation returned an actual refusal or unexpected status'); mark('route'); assertUploadRoute(); }
  async function durable(label, expected) { mark('durable-read'); const value = await control('snapshot', { session_id: sessionId }); assertUploadRoute(); mark('durable-check'); const s = validateDurableUpload(value, { account, sessionId, ...expected }); assert.ok(s.version >= lastVersion); lastVersion = s.version; result.durable.push({ label, ...value }); save(); return value; }
  async function snapshotStage(label, expected, count, action) { const start = observer.mutationCount(); mark('snapshot-action'); await action(); await completeWrites(start, count); const fresh = await exact(); const value = await durable(label, { ...expected, order: fresh.pageOrder }); mark('snapshot-check'); assert.equal(value.session.version, fresh.version); assert.deepEqual(fresh.draft, value.session.draft); return value; }
  async function chooseGroup() {
    mark('group-trigger');
    // Dialog.Trigger renders its child InputGroup.Root as a div role=group,
    // not a button. Click its exact preserved text through the real DOM action.
    const p = await panel(), search = p.getByText('Search...', { exact: true });
    await (await unique(search)).click();
    const dialog = await unique(page.getByRole('dialog'));
    mark('group-query'); await dialog.getByPlaceholder('Search...', { exact: true }).fill('Synthetic signed upload group');
    mark('group-result'); const match = dialog.locator('button[title="Synthetic signed upload group"]');
    await (await button(dialog, 'Synthetic signed upload group')).click(); assert.equal(await match.count(), 1);
    mark('group-close'); await page.keyboard.press('Escape');
    await until(() => page.getByRole('dialog').count(), n => n === 0, 'Group dialog closed');
  }
  async function connectKey() {
    await connectPublicUploadKey({ page, unique, button, until, assertUploadRoute, secretHex, importStep: createKeyImportStep(result.key_import, save) });
  }
  async function readyTransportReader() {
    assert.ok(result.transport_refusal_reader_checks < 2);
    assert.equal(await uploadRefusalReaderReady(page), true, 'QA transport reader handshake failed');
    result.transport_refusal_reader_checks++;
  }

  async function english() {
    mark('language');
    // Use the accessible named language selector; its submitted value remains the exact code.
    const p = await panel(), input = await unique(p.getByLabel('Language', { exact: true }));
    assert.equal(await input.getAttribute('aria-label'), 'Language');
    await until(() => input.isEnabled(), Boolean, 'Language selector enabled');
    await input.selectOption({ value: 'en' }); assert.equal(await input.inputValue(), 'en'); assertUploadRoute();
  }
  async function decoded(expected, sourceBasis = 'acknowledged-local-or-signed-preview') { mark('preview-order'); const c = controls(); await c.readSettledOrder(expected.map(image => image.name)); const rows = [];
    for (const image of expected) { mark('preview-decode'); const card = await c.card(image.name), img = await unique(card.locator('img')); const value = await img.evaluate(async (node) => { const source = node.currentSrc; if (!source.startsWith('blob:')) throw Error('Expected private preview Blob URL'); await node.decode(); if (node.currentSrc !== source || !node.complete || !node.naturalWidth || !node.naturalHeight) throw Error('Preview changed during decode'); return { decoded: true, width: node.naturalWidth, height: node.naturalHeight, blob_source: true }; }); assert.equal(value.width, image.width); assert.equal(value.height, image.height); rows.push({ filename: image.name, ...value }); }
    assertUploadRoute(); result.preview_decodes.push({ session_id: sessionId, version: lastVersion, source_basis: sourceBasis, files: rows }); return rows;
  }
  async function restoredMetadata() {
    mark('metadata-read');
    await until(async () => { const p = await panel(); const value = { title: await p.getByPlaceholder('Chapter Name', { exact: true }).inputValue(), chapter: await p.getByPlaceholder('Chapter No.', { exact: true }).inputValue(), language: await p.getByLabel('Language', { exact: true }).inputValue() }; assertUploadRoute(); return value; }, value => value.title === 'Synthetic public upload recovery' && value.chapter === '9001' && value.language === 'en', 'Restored exact metadata controls');
  }
  async function visibleCompletion(session) {
    mark('completion');
    await until(() => page.getByRole('dialog').count(), count => count === 1, 'Actual upload completion dialog');
    const dialog = await unique(page.getByRole('dialog')), title = await unique(dialog.locator('[data-slot="dialog-title"]'));
    await title.waitFor({ state: 'visible' }); const text = (await title.innerText()).trim();
    assertUploadCompletion(text, session); assertUploadRoute(); return text;
  }
  async function attach(image, expected) { return snapshotStage(`attach-${image.name}`, { images: expected }, 2, async () => { mark('file-input'); const p = await panel(); const input = await unique(p.locator('input[type="file"][accept*="image/jpeg"]')); await input.setInputFiles({ name: image.name, mimeType: image.mimeType, buffer: image.buffer }); }); }
  try {
    runnerStage('running-checks');
    await run('provision real bound synthetic upload publisher', async () => { mark('fixture-account'); account = await control('account', { pubkey: owner }); assert.equal(account.pubkey, owner); assert.equal(account.service_key, fixture.service_key); assert.equal(account.browser_operator_binding, false); assert.equal(account.browser_publisher_admission_granted, false); for (const kind of ['user','group','manga']) assert.match(account[kind].id, uuid); observer.bindAccount(account.user.id); return { admission: true, original_group_role: account.original_group_role, native_permissions: account.native_permissions }; });
    await run('import actual session key through Public identity UI', async () => { mark('navigation'); const response = await page.goto(origin + pathname(), { waitUntil: 'domcontentloaded' }); assert.equal(response.status(), 200); await readyTransportReader(); mark('key-import'); await connectKey(); mark('account-context'); const accountRead = await reader.get('/api/v1/nostr/account-context'); assert.equal(accountRead.status, 200); assert.equal(accountRead.body.user.id, account.user.id); assert.equal(accountRead.body.nostr.pubkey, owner); assert.equal(accountRead.body.nostr.nativeSessionAuthority, false); return { imported_via_real_ui: true, cookies_required: false }; });
    await run('begin, language autosave and real image before cancellation', async () => { await chooseGroup(); await snapshotStage('begin-cancel', { images: [] }, 1, async () => { const start = observer.mutationCount(); mark('begin-control'); await (await button(await panel(), 'Confirm')).click(); mark('begin-dispatch'); await until(() => observer.mutationCount(), n => n === start + 1, 'Begin dispatched'); mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Begin browser terminal'); sessionId = await current(); lastVersion = 0; }); await snapshotStage('language-cancel', { images: [], draft: { language: 'en' } }, 1, english); await attach(cancelImage, [cancelImage]); await decoded([cancelImage]); return { session_id: sessionId, version: lastVersion }; });
    await run('dismiss cancellation without writes then cancel exact session', async () => { const start = observer.mutationCount(); mark('cancel-dialog'); await (await button(await panel(), 'Cancel staged upload')).click(); const dialog = await unique(page.getByRole('dialog')); mark('cancel-dismiss'); await (await button(dialog, 'Cancel')).click(); mark('cancel-check'); assert.equal(observer.mutationCount(), start); const savedId = sessionId; mark('cancel-dialog'); await (await button(await panel(), 'Cancel staged upload')).click(); mark('cancel-confirm'); await (await button(await unique(page.getByRole('dialog')), 'Cancel staged upload')).click(); await completeWrites(start, 1); await durable('cancelled', { state: 'deleted', images: [] }); mark('cancel-check'); const absent = await reader.get('/api/v1/upload'); assert.equal(absent.status, 404); return { cancelled_session: savedId, exact_tombstone_observed: true, current_absent: true }; });
    await run('new begin preserves retained language as a genuine save', async () => { await snapshotStage('begin-retained-language', { images: [], draft: { language: 'en' } }, 2, async () => { const start = observer.mutationCount(); mark('begin-control'); await (await button(await panel(), 'Confirm')).click(); mark('begin-dispatch'); await until(() => observer.mutationCount(), count => count >= start + 1, 'Second begin started'); mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Second begin browser terminal'); sessionId = await current(); lastVersion = 0; }); return { session_id: sessionId, version: lastVersion }; });
    await run('autosave title and chapter', async () => { await snapshotStage('title', { images: [], draft: { language: 'en', title: 'Synthetic public upload recovery' } }, 1, async () => { mark('metadata-title'); await (await panel()).getByPlaceholder('Chapter Name', { exact: true }).fill('Synthetic public upload recovery'); }); await snapshotStage('chapter', { images: [], draft: { language: 'en', title: 'Synthetic public upload recovery', chapter: '9001' } }, 1, async () => { mark('metadata-chapter'); await (await panel()).getByPlaceholder('Chapter No.', { exact: true }).fill('9001'); }); return { version: lastVersion }; });
    for (let i = 0; i < images.length; i++) await run(`attach original image ${i + 1} with automatic staging`, async () => { await attach(images[i], images.slice(0, i + 1)); await decoded(images.slice(0, i + 1)); return { original_sha256: images[i].sha256, version: lastVersion }; });
    const ordered = [images[2], images[0], images[1]];
    await run('manually reorder real direct page cards', async () => { const c = controls(); try { await snapshotStage('reordered', { images, draft: { chapter: '9001' } }, 1, () => { mark('reorder'); return c.reorderLastToFirst(images.map(image => image.name)); }); } finally { const cleanup = c.cleanupSnapshot(); result.ui_controls.push({ down_attempted: cleanup.downAttempted, release_attempted: cleanup.releaseAttempted, release_succeeded: cleanup.releaseSucceeded, release_failed: cleanup.releaseError !== null, guard_failure_count: cleanup.guardErrors.length, fenced: cleanup.fenced, primary_failed: cleanup.primaryError !== null }); } await decoded(ordered); const fresh = await exact(); assert.deepEqual(fresh.files.map(file => file.originalFileName), ordered.map(image => image.name)); return { version: lastVersion, filenames: ordered.map(image => image.name) }; });
    let remaining = ordered.slice(0, 2);
    await run('remove exact middle-upload page and save remaining order', async () => { await snapshotStage('removed', { images: remaining }, 2, async () => { mark('remove'); const card = await controls().card(images[1].name); const buttons = card.getByRole('button'); assert.equal(await buttons.count(), 2); await buttons.nth(1).click(); }); await decoded(remaining); return { version: lastVersion }; });
    await run('replace one acknowledged page through its real file chooser', async () => {
      const before = await exact(), beforeStored = result.durable.at(-1).session; mark('replacement-check'); assert.equal(beforeStored.version, before.version);
      const old = before.files.find(file => file.originalFileName === images[0].name); assert.ok(old);
      mark('replacement-control'); const next = [remaining[0], replacementImage], card = await controls().card(images[0].name), buttons = card.getByRole('button');
      assert.equal(await buttons.count(), 2); const replaceButton = buttons.nth(0); assert.equal(await replaceButton.isVisible(), true);
      await until(() => replaceButton.isEnabled(), Boolean, 'Replacement control enabled');
      const firstRow = observer.rows.length;
      const value = await snapshotStage('replaced', { images: next }, 1, async () => {
        mark('replacement-chooser'); const chooserTask = page.waitForEvent('filechooser', { timeout: 5000 });
        try { await replaceButton.click({ timeout: 5000 }); const chooser = await chooserTask; await chooser.setFiles({ name: replacementImage.name, mimeType: replacementImage.mimeType, buffer: replacementImage.buffer }); }
        catch (error) { await chooserTask.catch(() => {}); throw error; }
      });
      const mutation = assertReplacementMutation(observer.rows.slice(firstRow).filter(row => row.method !== 'GET'), { sessionId, fileId: old.id });
      const after = await exact(); mark('replacement-check'); assertReplacementReadback(before, after, { fileId: old.id, image: replacementImage });
      assert.deepEqual(value.session.stored_page_order, before.pageOrder); assert.equal(value.session.version, before.version + 1);
      const stored = value.session.files.find(file => file.id === old.id), previous = beforeStored.files.find(file => file.id === old.id);
      assert.ok(stored && previous); assert.equal(stored.position, previous.position);
      assert.deepEqual(value.session.files.filter(file => file.id !== old.id), beforeStored.files.filter(file => file.id !== old.id));
      remaining = next; await decoded(remaining);
      result.replacement = { session_id: sessionId, file_id: old.id, mutation_event_id: mutation.event_id, version_before: before.version, version_after: after.version, old_sha256: old.fileHash, new_sha256: stored.storage_sha256, stored_original_bytes: stored.storage_bytes, stable_file_id: true, native_position_preserved: true, stored_order_preserved: true, neighbours_unchanged: true };
      return { ...result.replacement };
    });
    await run('hard reload, real key reconnect and durable signed private previews', async () => {
      mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Pre-reload signed terminals');
      result.transport_refusal_captures.push(await captureUploadRequestRefusals(page));
      mark('reload'); const start = observer.mutationCount(), before = lastVersion, oldDocument = await page.evaluate(() => performance.timeOrigin);
      await page.reload({ waitUntil: 'domcontentloaded' }); assert.notEqual(await page.evaluate(() => performance.timeOrigin), oldDocument, 'Mandatory fresh document before signed-preview proof');
      await readyTransportReader();
      const firstRow = observer.rows.length; // Actual replacement-page sign-in and preview reads occur after this new-document boundary.
      mark('key-import'); await connectKey(); const fresh = await exact(); await durable('reloaded', { images: remaining, draft: { title: 'Synthetic public upload recovery', chapter: '9001', language: 'en' }, order: fresh.pageOrder });
      assert.equal(lastVersion, before); await restoredMetadata(); await decoded(remaining, 'signed-preview-after-hard-reload-and-key-reimport');
      mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Restored signed preview terminals'); mark('preview-read'); assertUploadRoute(); assert.equal(observer.mutationCount(), start);
      const previews = fresh.files.map(file => { const path = `/api/v1/nostr/uploads/${sessionId}/files/${file.id}/preview`; const rows = observer.rows.slice(firstRow).filter(row => row.method === 'GET' && row.path === path); assert.ok(rows.length >= 1, 'New-document genuine signed preview GET missing'); for (const row of rows) { assert.equal(row.status, 200); assert.equal(row.native, 'finished'); } return { file_id: file.id, file_hash: file.fileHash, event_ids: rows.map(row => row.event_id) }; });
      return { hard_reload: true, explicit_key_reimport: true, local_File_thumbnails_absent: true, signed_previews: previews, version: lastVersion };
    });
    await run('commit once through actual terms and approval controls', async () => { const start = observer.mutationCount(), beforeCommit = await exact(), p = await panel(); mark('commit-control'); await p.locator('#terms').check(); await (await button(p, 'Upload')).click(); await completeWrites(start, 2); const value = await durable('committed', { state: 'committed', images: [], order: beforeCommit.pageOrder }); mark('publication-check'); assert.match(value.publication?.event_id ?? '', /^[0-9a-f]{64}$/); assert.equal(value.chapter?.event_id, value.publication.event_id); assert.equal(value.publication.original_event?.id, value.publication.event_id); assert.equal(value.publication.original_event?.kind, 1001); assert.equal(value.publication.original_event?.verified, true); assert.equal(value.publication.original_event?.pubkey, account.group.pubkey); assert.equal(value.chapter.pages.length, remaining.length); for (let i = 0; i < remaining.length; i++) { const stored = value.chapter.pages[i]; assert.equal(stored.file_hash, remaining[i].sha256); assert.equal(stored.storage_sha256, remaining[i].sha256); assert.equal(stored.storage_bytes, remaining[i].buffer.length); assert.equal(stored.fresh_original_read, true); } const receipt = await exact(); mark('receipt-read'); assert.equal(receipt.isCommitted, true); assert.equal(receipt.canMutate, false); assert.equal(receipt.canCancel, false); assert.equal(receipt.chapterId, value.chapter.id); const completionTitle = await visibleCompletion(receipt); const checkReceipt = page.getByRole('button', { name: 'Check saved publication receipt', exact: true }); if (await checkReceipt.count()) { mark('receipt-refresh'); const before = observer.mutationCount(); await (await button(page, 'Check saved publication receipt')).click(); await until(() => observer.settled(), Boolean, 'Exact receipt read terminals'); assert.equal(observer.mutationCount(), before); } mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Final signed browser terminals'); mark('route'); assertUploadRoute(); mark('mutation-count'); assert.equal(observer.mutationCount(), 21, 'Exact natural-success journey mutation count'); result.completed = true; return { completion_title: completionTitle, chapter_id: value.chapter.id, event_id: value.publication.event_id, publication_status: value.session.publication_status, readable_projection_not_assumed: true }; });
  } finally {
    result.transport_refusal_captures.push(await captureUploadRequestRefusals(page,
      observer.rows.filter(row => row.native === 'failed').map(row => row.event_id)));
    result.preview_transport_refusals = await captureUploadPreviewRefusals(page);
    try {
      if (result.completed) await until(() => {
        // Captures above can yield to late signed reads. Check and detach in the
        // same synchronous turn, so no admitted row misses its native terminal.
        if (!observer.settled()) return false;
        assert.equal(observer.mutationCount(), 21, 'Unexpected final UI mutation');
        observer.close(); return true;
      }, Boolean, 'Final signed browser closure');
    } finally {
      try { observer.close(); reader.close(); } finally { result.browser_http = observer.snapshot(); result.independent_http = reader.snapshot();
        secret?.fill(0); secret = undefined; secretHex = ''; for (const image of [...images, cancelImage, replacementImage]) image.buffer.fill(0); save(); }
    }
    // The outer safe-error serializer still owns its redaction entry until the
    // final report has been persisted; removing it here could expose a late error.
  }
}
