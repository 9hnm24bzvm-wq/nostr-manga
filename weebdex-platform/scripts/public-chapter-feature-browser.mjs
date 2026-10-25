// Prospective genuine UI/HTTP entrypoint coverage. No chapter mutation, native
// account provisioning, browser route substitution or response interception.
import assert from 'node:assert/strict';
import { verifiedSchnorr } from './browser-signed-journeys.mjs';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const NATIVE = /^[A-Za-z0-9]{10}$/;
const HEX = /^[0-9a-f]{64}$/;
const CASE_MS = 45000;
const ACCOUNT_PATH = '/api/v1/nostr/account-context';

function loopback(value) {
  const url = new URL(value);
  assert.equal(url.protocol, 'http:');
  assert.ok(['127.0.0.1', 'localhost'].includes(url.hostname));
  assert.ok(url.port && Number(url.port) > 0);
  assert.equal(url.username + url.password + url.search + url.hash, '');
  assert.equal(url.pathname, '/');
  return url.origin;
}
export function chapterFeaturePrerequisites(origin, fixture) {
  const platform = loopback(origin), sink = loopback(fixture.mail_origin);
  const seed = fixture.protocol_seed;
  assert.equal(seed?.fixture_only, true);
  assert.equal(seed.fully_ready, true);
  assert.match(seed.catalog_namespace, UUID);
  assert.equal(seed.series?.canonical_kind, 1002);
  assert.ok(Array.isArray(seed.chapters) && seed.chapters.length >= 2);
  const selected = seed.chapters[0];
  assert.equal(selected.canonical_kind, 1001);
  for (const item of [seed.series, selected]) {
    assert.match(item.native_id, NATIVE);
    assert.match(item.event_id, HEX);
    assert.equal(item.visible, true);
    assert.equal(item.projected, true);
    assert.equal(item.mapped, true);
  }
  return { platform, sink, namespace: seed.catalog_namespace, mangaLegacy: seed.series.native_id, chapterLegacy: selected.native_id };
}
export function chapterFeatureTargets(body, prerequisite) {
  assert.equal(body?.result, 'ok');
  assert.ok(Array.isArray(body.data) && body.data.length === 2);
  assert.deepEqual(body.unresolved, []);
  const target = {};
  for (const [kind, legacy] of [['manga', prerequisite.mangaLegacy], ['chapter', prerequisite.chapterLegacy]]) {
    const matches = body.data.filter(row => row.kind === kind && row.legacy_id === legacy);
    assert.equal(matches.length, 1);
    assert.match(matches[0].id, UUID);
    target[kind] = matches[0].id;
  }
  return target;
}
export function chapterFeatureRecord(body, target) {
  const row = body?.data;
  assert.equal(body?.result, 'ok');
  assert.equal(row?.type, 'chapter');
  assert.equal(row.id, target.chapter);
  assert.ok(Number.isSafeInteger(row.attributes?.version) && row.attributes.version >= 1);
  assert.ok(Array.isArray(row.relationships));
  const parents = row.relationships.filter(item => item.type === 'manga');
  assert.equal(parents.length, 1);
  assert.equal(parents[0].id, target.manga);
  const value = row.attributes;
  assert.equal(typeof value.translatedLanguage, 'string');
  assert.match(value.translatedLanguage, /^[a-z]{2,3}(?:-[a-zA-Z0-9]{2,8})?$/);
  for (const name of ['title', 'volume', 'chapter']) assert.ok(value[name] === null || value[name] === undefined || typeof value[name] === 'string');
  return { title: value.title ?? '', volume: value.volume ?? '', chapter: value.chapter ?? '', language: value.translatedLanguage };
}

async function runChapterEntry(h) {
  const { page, context, origin, fixture, insist, redactions } = h;
  const pre = chapterFeaturePrerequisites(origin, fixture);
  assert.equal(typeof insist, 'function');
  assert.equal(typeof redactions?.add, 'function');
  const deadline = Date.now() + CASE_MS;
  const left = (cap = 5000) => { const value = Math.min(cap, deadline - Date.now()); assert.ok(value > 0, 'Chapter feature case deadline'); return value; };
  const unique = async locator => { assert.equal(await locator.count(), 1, 'Expected one chapter feature control'); return locator; };
  async function until(read, accept) {
    for (;;) { left(); const value = await read(); if (accept(value)) return value; await page.waitForTimeout(Math.min(75, left())); }
  }
  async function button(scope, name) {
    const locator = scope.getByRole('button', { name, exact: true });
    await until(() => locator.count(), count => count === 1);
    await until(() => locator.isEnabled(), Boolean);
    return unique(locator);
  }
  async function json(response) {
    assert.equal(response.status(), 200, 'Chapter feature fixture/read refused');
    const bytes = await response.body(); // APIRequestContext has already downloaded this bounded fixture reply.
    assert.ok(bytes.length > 0 && bytes.length <= 65536, 'Chapter feature fixture/read body bound');
    return JSON.parse(bytes.toString('utf8'));
  }
  const read = async path => json(await context.request.get(pre.platform + path, { maxRedirects: 0, timeout: left() }));
  const noCookie = async () => {
    const response = await context.request.get(pre.platform + '/api/auth/check', { maxRedirects: 0, timeout: left() });
    assert.equal(response.status(), 401, 'Chapter feature requires no native CookieSession');
  };
  await noCookie();
  const mapping = await json(await context.request.post(pre.platform + '/api/v1/legacy/resolve', { data: { catalog_namespace: pre.namespace, resources: [{ kind: 'manga', id: pre.mangaLegacy }, { kind: 'chapter', id: pre.chapterLegacy }] }, headers: { Origin: pre.platform }, maxRedirects: 0, timeout: left() }));
  const target = chapterFeatureTargets(mapping, pre);
  const metadata = chapterFeatureRecord(await read('/api/v1/chapter/' + target.chapter + '?includes[]=manga'), target);
  const titleUrl = new URL('/title/' + target.manga, pre.platform);
  titleUrl.searchParams.set('tab', 'chapters'); titleUrl.searchParams.set('tlang', metadata.language);
  const navigation = await page.goto(titleUrl.href, { waitUntil: 'domcontentloaded', timeout: left(10000) });
  assert.equal(navigation?.status(), 200);
  const row = page.locator('article').filter({ has: page.locator('a[href="/chapter/' + target.chapter + '"]') });
  await until(() => row.count(), count => count === 1);
  assert.equal(await row.locator('a[title="Edit"]').count(), 0, 'Anonymous list must not invent a selected signer');
  const timeOrigin = await page.evaluate(() => performance.timeOrigin);
  await (await unique(page.locator('a[href="/settings"]:visible'))).click({ timeout: left() });
  await until(() => new URL(page.url()).pathname, value => value === '/settings/preferences');
  await (await unique(page.getByRole('link', { name: 'Public identity', exact: true }))).click({ timeout: left() });
  await until(() => new URL(page.url()).pathname, value => value === '/settings/identity');
  const identity = page.locator('[data-protocol-identity-panel]');
  await until(() => identity.getByText(/^No signing identity connected\./).count(), count => count === 1);
  const schnorr = await verifiedSchnorr(insist);
  const secret = schnorr.utils.randomSecretKey();
  let secretHex = '', observeAccountRequest;
  try {
    secretHex = Buffer.from(secret).toString('hex');
    redactions.add(secretHex);
    const pubkey = Buffer.from(schnorr.getPublicKey(secret)).toString('hex');
    const admission = await json(await context.request.post(pre.sink + '/fixture/protocol/account', { data: { pubkey, allowed: true }, maxRedirects: 0, timeout: left() }));
    assert.equal(admission.fixture_only, true); assert.equal(admission.pubkey, pubkey); assert.equal(admission.allowed, true);
    assert.equal(admission.publisher_permission_granted, false); assert.equal(admission.private_key_received, false);
    await (await button(identity, 'Create or paste a key')).click({ timeout: left() });
    const setup = page.locator('[data-identity-setup]');
    await (await button(setup, 'Paste an existing key')).click({ timeout: left() });
    const input = await unique(setup.getByLabel('Private key (nsec or hex)', { exact: true }));
    assert.equal(await input.getAttribute('type'), 'password');
    await input.fill(secretHex, { timeout: left() });
    await (await button(setup, 'Use this key')).click({ timeout: left() });
    await setup.getByRole('checkbox', { name: 'I have saved my private key somewhere safe.', exact: true }).check({ timeout: left() });
    await (await button(setup, 'Use this key for this session')).click({ timeout: left() });
    await (await button(setup, 'Done')).click({ timeout: left() });
    await until(() => identity.getByText(/^Session private key connected\./).count(), count => count === 1);
    await page.goBack({ waitUntil: 'domcontentloaded', timeout: left() });
    await page.goBack({ waitUntil: 'domcontentloaded', timeout: left() });
    assert.equal(page.url(), titleUrl.href);
    assert.equal(await page.evaluate(() => performance.timeOrigin), timeOrigin, 'Signer must survive actual same-document Settings navigation');
    const edit = row.locator('a[title="Edit"][href="/chapter/' + target.chapter + '/edit"]');
    await until(() => edit.count(), count => count === 1);
    const clickTarget = await unique(edit);
    const childReads = new WeakSet();
    observeAccountRequest = request => {
      const url = new URL(request.url());
      if (url.origin === pre.platform && url.pathname === ACCOUNT_PATH && !url.search && request.method() === 'GET' && new URL(page.url()).pathname === '/chapter/' + target.chapter + '/edit') childReads.add(request);
    };
    page.on('request', observeAccountRequest);
    const refusal = page.waitForResponse(response => childReads.has(response.request()), { timeout: left(10000) });
    const finished = page.waitForEvent('requestfinished', { predicate: request => childReads.has(request), timeout: left(10000) });
    // Attach both branches before clicking so a failed navigation cannot leave
    // an unhandled response waiter. Both native Playwright waits are bounded.
    const [response, finishedRequest] = await Promise.all([refusal, finished, clickTarget.click({ timeout: left() })]);
    assert.equal(finishedRequest, response.request(), 'The exact refused child account read must finish its body');
    assert.equal(response.status(), 403, 'A genuinely unbound signer must not gain native account authority');
    await until(() => new URL(page.url()).pathname, value => value === '/chapter/' + target.chapter + '/edit');
    const editor = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Edit chapter', exact: true }) });
    await until(() => editor.count(), count => count === 1);
    await unique(editor);
    await until(() => editor.getByRole('status').count(), count => count === 1);
    assert.equal(await editor.getByLabel('Title', { exact: true }).inputValue({ timeout: left() }), metadata.title);
    assert.equal(await editor.getByLabel('Volume', { exact: true }).inputValue({ timeout: left() }), metadata.volume);
    assert.equal(await editor.getByLabel('Chapter', { exact: true }).inputValue({ timeout: left() }), metadata.chapter);
    assert.equal(await editor.getByLabel('Language', { exact: true }).inputValue({ timeout: left() }), metadata.language);
    assert.equal(await editor.getByRole('button', { name: 'Save instance metadata', exact: true }).isDisabled(), true);
    assert.equal(await editor.getByRole('button', { name: 'Claim local chapter ownership', exact: true }).isDisabled(), true);
    assert.equal(await editor.getByRole('button', { name: 'Request chapter removal', exact: true }).isDisabled(), true);
    assert.equal(await editor.getByText('Changes are instance-local catalogue annotations. Original signed source metadata and protocol image pages are immutable. Eligible local chapter pages use separate private staging below.', { exact: true }).count(), 1);
    await noCookie();
    return { actual_identity_ui: true, no_cookie_session: true, typed_target_association: true, edit_link_visible: true, canonical_edit_child_mounted: true, actual_account_status: 403, account_request_finished: true, mutation_controls_disabled: true, native_binding_ready_tested: false, metadata_save_tested: false };
  } finally { if (observeAccountRequest) page.off('request', observeAccountRequest); secret.fill(0); secretHex = ''; }
}

export const publicChapterFeatureCases = Object.freeze([
  Object.freeze({ id: 'chapter-edit-entry-signed', run: runChapterEntry })
]);
