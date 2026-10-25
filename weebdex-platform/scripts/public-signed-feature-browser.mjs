// Prospective genuine browser qualification against an opted-in disposable
// application/PostgreSQL/original fixture. No routes or responses are mocked.
import assert from 'node:assert/strict';
import { verifiedSchnorr } from './browser-signed-journeys.mjs';
import { chapterFeatureRecord } from './public-chapter-feature-browser.mjs';
import { observeSignedFeatures, featureUuid } from './public-signed-feature-observer.mjs';
export const publicSignedFeaturePhase = 'public-signed-feature-recovery';
export const signedFeatureCaseIds = Object.freeze([
  'signed-feature-identity', 'signed-report-cancel', 'signed-report-submit',
  'signed-chapter-save', 'signed-draft-owner-preview-cancel', 'signed-draft-submit-removal-review',
]);
export const signedFeatureScope = 'actual-signed-report-draft-chapter-v1';
const scenario = 'public-signed-features-v1';
export function signedFeaturePrerequisites(origin, fixture) {
  function local(value) { const u = new URL(value); assert.equal(u.protocol, 'http:'); assert.ok(['127.0.0.1', 'localhost'].includes(u.hostname)); assert.ok(Number(u.port) > 0); assert.equal(u.pathname, '/'); assert.equal(u.username + u.password + u.search + u.hash, ''); return u.origin; }
  assert.equal(fixture.signed_features?.enabled, true); assert.equal(fixture.signed_features?.fixture_only, true); assert.equal(fixture.signed_features?.scenario, scenario);
  assert.equal(fixture.protocol_seed?.fixture_only, true); assert.equal(fixture.protocol_seed?.fully_ready, true);
  assert.match(fixture.service_key, /^[0-9a-f]{64}$/); assert.match(fixture.protocol_seed.catalog_namespace, featureUuid);
  return { origin: local(origin), sink: local(fixture.mail_origin), service: fixture.service_key, namespace: fixture.protocol_seed.catalog_namespace };
}
export function validateSignedFeatureAccount(value, pre, pubkey) {
  assert.equal(value.fixture_only, true); assert.equal(value.scenario, scenario); assert.equal(value.pubkey, pubkey); assert.equal(value.allowed, true);
  assert.equal(value.catalog_namespace, pre.namespace); assert.equal(value.service_key, pre.service);
  for (const name of ['user', 'draft', 'foreign_draft', 'manga', 'chapter']) { assert.match(value[name]?.id, featureUuid); assert.match(value[name]?.native_id, /^[A-Za-z0-9]{10}$/); }
  assert.match(value.reason?.id, featureUuid); assert.equal(value.reason?.native_id, 'manga_signed_feature_fixture');
  assert.notEqual(value.draft.id, value.foreign_draft.id); assert.notEqual(value.draft.id, value.manga.id);
  assert.deepEqual(value.roles, ['user']); assert.deepEqual(value.native_permissions, ['report.create', 'manga.create', 'self.chapter.update']);
  for (const name of ['browser_private_key_received', 'secret_received', 'browser_operator_binding', 'browser_publisher_admission_granted']) assert.equal(value[name], false);
  assert.ok(Number.isSafeInteger(value.reason.version) && value.reason.version > 0); assert.equal(typeof value.reason.text, 'string'); assert.ok(value.reason.text.length > 0 && value.reason.text.length <= 1000);
  assert.equal(value.report_body, 'Synthetic signed feature report details.'); assert.equal(value.chapter_saved_title, 'Synthetic signed feature saved chapter');
  return value;
}
export function validateSignedFeatureSnapshot(value, account) {
  assert.equal(value.fixture_only, true); assert.equal(value.scenario, scenario); assert.equal(value.pubkey, account.pubkey);
  for (const name of ['draft', 'foreign_draft', 'chapter']) { assert.equal(value[name]?.id, account[name].id); assert.equal(value[name]?.native_id, account[name].native_id); assert.ok(Number.isSafeInteger(value[name].version) && value[name].version > 0); }
  assert.equal(value.draft.owner_id, account.user.id); assert.equal(value.foreign_draft.deleted, false);
  assert.equal(value.draft.deleted, false); assert.equal(value.foreign_draft.state, 'draft'); assert.notEqual(value.foreign_draft.owner_id, account.user.id);
  assert.equal(value.chapter.uploader_id, account.user.id); assert.equal(value.chapter.deleted, false); assert.equal(value.chapter.is_unavailable, false);
  assert.equal(value.browser_private_key_received, false); assert.equal(value.secret_received, false);
  assert.ok(Number.isSafeInteger(value.report?.count) && value.report.count >= 0 && value.report.count <= 1); assert.equal(value.report.records.length, value.report.count);
  assert.equal(value.original?.unchanged, true); assert.equal(value.original.fresh_original_reads, true); assert.match(value.original.source_event_id, /^[0-9a-f]{64}$/); assert.ok(Number.isSafeInteger(value.original.source_version) && value.original.source_version > 0); for (const key of ['source_event_sha256', 'manifest_sha256']) assert.match(value.original[key], /^[0-9a-f]{64}$/);
  assert.ok(Array.isArray(value.original.pages) && value.original.pages.length > 0 && value.original.pages.length <= 12);
  for (const page of value.original.pages) { assert.match(page.sha256, /^[0-9a-f]{64}$/); assert.ok(Number.isSafeInteger(page.bytes) && page.bytes > 0 && page.bytes <= 4 * 1024 * 1024); }
  return value;
}
export async function runPublicSignedFeatures(h) {
  const { page, context, origin, fixture, report, step, save, insist, redactions } = h;
  assert.equal(h.phase, publicSignedFeaturePhase); assert.equal(report.steps.length, 0);
  const pre = signedFeaturePrerequisites(origin, fixture), deadline = Date.now() + 240000;
  const result = report.signed_features = { schema: 1, scope: signedFeatureScope, checks: [], completed: false, actual_context_closed: false, observer_closed: false };
  save();
  const left = (cap = 7000) => { const ms = Math.min(cap, deadline - Date.now()); assert.ok(ms > 0, 'Signed feature deadline exhausted'); return ms; };
  let secret, secretHex = '', account, baseline, observer, chapterMetadata, next = 0;
  function healthy() { observer?.healthy(); assert.equal(report.page_errors.length, 0, 'Actual application page error'); left(); }
  async function until(read, accept) { for (;;) { healthy(); const value = await read(); if (accept(value)) return value; await page.waitForTimeout(Math.min(75, left())); } }
  async function unique(locator) { await until(() => locator.count(), n => { assert.ok(n <= 1, 'Duplicate signed feature control'); return n === 1; }); return locator; }
  async function button(scope, name) { const target = await unique(scope.getByRole('button', { name, exact: true })); await until(() => target.isEnabled(), Boolean); return target; }
  const click = async target => (await target).click({ timeout: left() });
  async function json(response) { assert.equal(response.status(), 200, 'Signed feature fixture/read must succeed'); const bytes = await response.body(); assert.ok(bytes.length > 0 && bytes.length <= 65536); return JSON.parse(bytes.toString('utf8')); }
  let controls = 0;
  async function control(kind) { assert.ok(++controls <= 14); return json(await context.request.post(pre.sink + '/fixture/protocol/signed-feature-' + kind, { data: { pubkey: account?.pubkey ?? pubkey }, maxRedirects: 0, timeout: left(20000) })); }
  const snapshot = async () => validateSignedFeatureSnapshot(await control('snapshot'), account);
  async function noCookie() { const response = await context.request.get(pre.origin + '/api/auth/check', { maxRedirects: 0, timeout: left() }); assert.equal(response.status(), 401, 'Signed features must not rely on native CookieSession'); }
  async function settled() { await until(() => observer.settled(), Boolean); }
  async function writes(count) { await settled(); assert.equal(observer.writes().length, count, 'Unexpected browser mutation or replay'); }
  async function run(id, action) { assert.equal(id, signedFeatureCaseIds[next]); const index = next++; await step('Signed feature ' + id, async () => { await action(); healthy(); return { actual_ui_http_and_durable_checks: true }; }); assert.equal(report.steps.length, next); const status = report.steps[index].status; assert.ok(['passed', 'failed'].includes(status)); result.checks.push({ id, status }); save(); assert.equal(status, 'passed', 'Stopped after the actual failed signed feature case'); }
  async function openReport() { await click(button(page, 'Report')); const dialog = await unique(page.getByRole('dialog', { name: 'Report Manga', exact: true })); await click(button(dialog, 'Load report reasons')); await until(() => dialog.getByText(/^Reporting as /).count(), n => n === 1); await click(unique(dialog.locator('button[data-slot="select-trigger"]'))); const list = await unique(page.locator('[data-slot="select-content"][role="listbox"]:visible')); await click(unique(list.getByRole('option', { name: account.reason.text, exact: true }))); return dialog; }
  const card = () => page.locator('article').filter({ has: page.locator('a[href="/title/' + account.draft.id + '?draft=true"]') });
  let pubkey;
  try {
    await run('signed-feature-identity', async () => {
      await noCookie(); const schnorr = await verifiedSchnorr(insist); secret = schnorr.utils.randomSecretKey(); secretHex = Buffer.from(secret).toString('hex'); redactions.add(secretHex); pubkey = Buffer.from(schnorr.getPublicKey(secret)).toString('hex');
      account = validateSignedFeatureAccount(await control('account'), pre, pubkey); baseline = await snapshot();
      assert.equal(baseline.report.count, 0); assert.equal(baseline.draft.state, 'draft'); assert.equal(baseline.draft.removal_request.count, 0);
      observer = observeSignedFeatures(page, { origin: pre.origin, service: pre.service, owner: pubkey, verify: (sig, id, key) => schnorr.verify(Buffer.from(sig, 'hex'), Buffer.from(id, 'hex'), Buffer.from(key, 'hex')) });
      chapterMetadata = chapterFeatureRecord(await json(await context.request.get(pre.origin + '/api/v1/chapter/' + account.chapter.id + '?includes[]=manga', { maxRedirects: 0, timeout: left() })), { manga: account.manga.id, chapter: account.chapter.id });
      const response = await page.goto(pre.origin + '/title/' + account.manga.id + '?tab=chapters&tlang=' + encodeURIComponent(chapterMetadata.language), { waitUntil: 'domcontentloaded', timeout: left(15000) }); assert.equal(response?.status(), 200);
      await button(page, 'Report'); const timeOrigin = await page.evaluate(() => performance.timeOrigin);
      await click(unique(page.locator('a[href="/settings"]:visible'))); await until(() => new URL(page.url()).pathname, path => path === '/settings/preferences');
      await click(unique(page.getByRole('link', { name: 'Public identity', exact: true })));
      const identity = page.locator('[data-protocol-identity-panel]'); await click(button(identity, 'Create or paste a key'));
      const setup = page.locator('[data-identity-setup]'); await click(button(setup, 'Paste an existing key'));
      const input = await unique(setup.getByLabel('Private key (nsec or hex)', { exact: true })); assert.equal(await input.getAttribute('type'), 'password'); await input.fill(secretHex, { timeout: left() });
      await click(button(setup, 'Use this key')); await setup.getByRole('checkbox', { name: 'I have saved my private key somewhere safe.', exact: true }).check({ timeout: left() });
      await click(button(setup, 'Use this key for this session')); await click(button(setup, 'Done'));
      await until(() => identity.getByText(/^Session private key connected\./).count(), n => n === 1);
      await page.goBack({ waitUntil: 'domcontentloaded', timeout: left() }); await page.goBack({ waitUntil: 'domcontentloaded', timeout: left() });
      assert.equal(new URL(page.url()).pathname, '/title/' + account.manga.id); assert.equal(await page.evaluate(() => performance.timeOrigin), timeOrigin); await writes(0); await noCookie();
    });
    await run('signed-report-cancel', async () => {
      const dialog = await openReport(); assert.equal(await dialog.getByRole('button', { name: 'Send', exact: true }).isDisabled(), true);
      await dialog.locator('textarea').fill(account.report_body, { timeout: left() }); await button(dialog, 'Send');
      await click(button(dialog, 'Cancel')); await until(() => page.getByRole('dialog').count(), n => n === 0); await writes(0); assert.equal((await snapshot()).report.count, 0);
    });
    await run('signed-report-submit', async () => {
      const dialog = await openReport(); await dialog.locator('textarea').fill(account.report_body, { timeout: left() }); await click(button(dialog, 'Send'));
      await until(() => dialog.getByText('Report submitted to this instance’s moderators.', { exact: true }).count(), n => n === 1);
      assert.equal(await dialog.getByRole('button', { name: 'Send', exact: true }).isDisabled(), true); await writes(1);
      observer.assertWrite(0, { method: 'POST', path: '/api/v1/report', status: 201, body: { category: 'manga', reason: account.reason.id, objectId: account.manga.id, details: account.report_body, expectedAccountId: account.user.id, expectedReasonVersion: account.reason.version } });
      const after = await snapshot(); assert.equal(after.report.count, 1); const row = after.report.records[0]; assert.equal(row.submitter_id, account.user.id); assert.equal(row.entity_type, 'manga'); assert.equal(row.entity_id, account.manga.id); assert.equal(row.reason_id, account.reason.id); assert.equal(row.body, account.report_body); assert.equal(row.status, 'waiting');
      await click(button(dialog, 'Close')); await click(button(page, 'Report')); const reopened = await unique(page.getByRole('dialog', { name: 'Report Manga', exact: true }));
      await click(button(reopened,'Cancel'));await writes(1);await click(button(page,'Report'));const inspectionDialog=await unique(page.getByRole('dialog',{name:'Report Manga',exact:true}));
      await click(unique(inspectionDialog.getByRole('link', { name: 'My signed reports', exact: true })));
      await until(() => new URL(page.url()).pathname, p => p === '/reports/signed');
      await click(button(page, 'Load personal reports')); await until(() => page.getByText(/^Report snapshot for /).count(), n => n === 1);
      assert.equal(await page.getByText(account.report_body, { exact: true }).count(), 1); await writes(1); await noCookie();
      observer.assertRead({path:'/api/v1/report',query:new URLSearchParams([['scope','mine'],['limit','10'],['offset','0'],['order[createdAt]','desc'],['includes[]','reason'],['expectedAccountId',account.user.id]])});
      await page.getByLabel('Report status', { exact: true }).selectOption('accepted', {timeout:left()}); await click(button(page,'Apply report filters'));
      await until(() => page.getByText(/^No reports were returned for this account/).count(), n => n === 1);await writes(1);
      observer.assertRead({path:'/api/v1/report',query:new URLSearchParams([['scope','mine'],['limit','10'],['offset','0'],['order[createdAt]','desc'],['includes[]','reason'],['expectedAccountId',account.user.id],['status','accepted']])});
      assert.equal((await snapshot()).report.count, 1);
      await page.goto(pre.origin+'/title/'+account.manga.id,{waitUntil:'domcontentloaded',timeout:left()});await button(page,'Report');

    });
    await run('signed-chapter-save', async () => {
      const target = { manga: account.manga.id, chapter: account.chapter.id };
      const metadata = chapterFeatureRecord(await json(await context.request.get(pre.origin + '/api/v1/chapter/' + target.chapter + '?includes[]=manga', { maxRedirects: 0, timeout: left() })), target); assert.deepEqual(metadata, chapterMetadata);
      const edit = page.locator('article').filter({ has: page.locator('a[href="/chapter/' + target.chapter + '"]') }).locator('a[title="Edit"]'); await click(unique(edit));
      await until(() => new URL(page.url()).pathname, p => p === '/chapter/' + target.chapter + '/edit');
      const editor = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Edit chapter', exact: true }) });
      await button(editor, 'Save instance metadata'); assert.equal(await editor.getByRole('button', { name: 'Request chapter removal', exact: true }).isDisabled(), true);
      assert.equal(await editor.getByLabel('Title', { exact: true }).inputValue({ timeout: left() }), metadata.title);
      const groups = (await editor.getByLabel('Group UUIDs, comma separated', { exact: true }).inputValue({ timeout: left() })).split(',').map(v => v.trim()).filter(Boolean);
      await editor.getByLabel('Title', { exact: true }).fill(account.chapter_saved_title, { timeout: left() }); await click(button(editor, 'Save instance metadata'));
      await until(() => editor.getByText('Instance chapter metadata updated. Original signed metadata and image pages are unchanged.', { exact: true }).count(), n => n === 1); await writes(2);
      observer.assertWrite(1, { method: 'PUT', path: '/api/v1/chapter/' + target.chapter, status: 200, body: { version: baseline.chapter.version, expectedAccountId: account.user.id, title: account.chapter_saved_title, volume: metadata.volume, chapter: metadata.chapter, translatedLanguage: metadata.language, groups } });
      const after = await snapshot(); assert.equal(after.chapter.title, account.chapter_saved_title); assert.equal(after.chapter.version, baseline.chapter.version + 1); assert.deepEqual(after.original, baseline.original);
      await click(button(editor, 'Reload current chapter')); await until(() => editor.getByText('Current chapter reloaded. No mutation was replayed.', { exact: true }).count(), n => n === 1); assert.equal(await editor.getByLabel('Title', { exact: true }).inputValue({ timeout: left() }), account.chapter_saved_title); await writes(2);
      await click(unique(editor.getByRole('link', { name: 'Return to the title', exact: true }))); await until(() => new URL(page.url()).pathname, p => p === '/title/' + account.manga.id);
    });
    await run('signed-draft-owner-preview-cancel', async () => {
      await click(unique(page.getByRole('link', { name: 'Add Title', exact: true }))); await until(() => new URL(page.url()).pathname, p => p === '/drafts/title'); await click(button(page, 'Load drafts'));
      await unique(card()); assert.equal(await page.locator('article').count(), 1); assert.equal(await page.locator('a[href="/title/' + account.foreign_draft.id + '?draft=true"]').count(), 0);
      await click(unique(card().getByRole('link', { name: 'Preview draft', exact: true }))); await until(() => page.url(), p => p === pre.origin + '/title/' + account.draft.id + '?draft=true');
      await unique(page.locator('a[href="/title/' + account.draft.id + '/edit?draft=true"]'));
      await page.goBack({ waitUntil: 'domcontentloaded', timeout: left() }); await until(() => new URL(page.url()).pathname, p => p === '/drafts/title'); await click(button(page, 'Load drafts')); await unique(card());
      await click(unique(card().locator('button[title="Submit"]'))); const dialog = await unique(page.getByRole('dialog', { name: 'Submit Draft', exact: true })); await click(button(dialog, 'Cancel')); await until(() => page.getByRole('dialog').count(), n => n === 0); await writes(2);
      const after = await snapshot(); assert.deepEqual(after.draft, baseline.draft); assert.deepEqual(after.foreign_draft, baseline.foreign_draft);
    });
    await run('signed-draft-submit-removal-review', async () => {
      await click(unique(card().locator('button[title="Submit"]'))); await click(button(await unique(page.getByRole('dialog', { name: 'Submit Draft', exact: true })), 'Submit'));
      await until(() => card().getByText('[Submitted]', { exact: true }).count(), n => n === 1); await writes(3);
      observer.assertWrite(2, { method: 'POST', path: '/api/v1/manga/draft/' + account.draft.id + '/commit', status: 201, body: { version: baseline.draft.version, expectedAccountId: account.user.id } });
      const submitted = await snapshot(); assert.equal(submitted.draft.state, 'submitted'); assert.equal(submitted.draft.version, baseline.draft.version + 1); assert.equal(submitted.draft.removal_request.count, 0);
      const state = page.locator('button[data-slot="select-trigger"]').filter({ hasText: /^State:/ });
      async function filter(label, value) { await click(unique(state)); await click(unique(page.locator('[data-slot="select-content"][role="listbox"]:visible').getByRole('option', { name: label, exact: true }))); await click(button(page, 'Search')); await until(() => new URL(page.url()).searchParams.get('state'), s => s === value); await click(button(page, 'Load drafts')); }
      await filter('Draft', 'draft'); await unique(page.getByText('No drafts were returned for this account and filter.', { exact: true })); assert.equal(await page.locator('article').count(), 0); await writes(3);
      await filter('Submitted', 'submitted'); await unique(card()); assert.equal(await page.locator('article').count(), 1); assert.equal(await card().locator('button[title="Submit"]').count(), 0);
      await click(unique(card().locator('button[title="Request removal review"]'))); const removal = await unique(page.getByRole('dialog', { name: 'Request Draft Removal Review', exact: true })); await click(button(removal, 'Request review'));
      await until(() => card().getByText(/^Removal review queued: .*\. The draft remains here until approved\.$/).count(), n => n === 1); await writes(4);
      observer.assertWrite(3, { method: 'DELETE', path: '/api/v1/manga/' + account.draft.id, status: 202, body: { version: submitted.draft.version, expectedAccountId: account.user.id, removalMode: 'request-review' } });
      const after = await snapshot(); assert.equal(after.draft.state, 'submitted'); assert.equal(after.draft.version, submitted.draft.version); assert.equal(after.draft.removal_request.count, 1); const removalRecord = after.draft.removal_request.records[0]; assert.equal(removalRecord.submitter_id, account.user.id); assert.equal(removalRecord.entity_type, 'manga'); assert.equal(removalRecord.entity_id, account.draft.id); assert.equal(removalRecord.status, 'waiting'); assert.equal(removalRecord.target_version, submitted.draft.version); assert.equal(removalRecord.body, 'Authorized current owner requests instance title removal; no original deletion requested'); assert.equal(after.total_report_count, 2); assert.deepEqual(after.foreign_draft, baseline.foreign_draft); assert.deepEqual(after.original, baseline.original); assert.equal(after.report.count, 1);
      await click(button(page, 'Reload drafts')); await unique(card()); assert.equal(await card().getByText('[Submitted]', { exact: true }).count(), 1); await writes(4); await noCookie();
    });
    await settled(); assert.ok(observer.count() > 4); result.completed = true; save();
  } finally { await observer?.close(); result.observer_closed = !!observer; secret?.fill(0); secretHex = ''; save(); }
}
