// Prospective browser-case controls: actual pure validators and the actual case
// under explicit finite Playwright/fixture/crypto stand-ins. No browser, HTTP,
// native account, signing dependency, key generation or service is started.
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';
import { chapterFeaturePrerequisites, chapterFeatureTargets, chapterFeatureRecord, publicChapterFeatureCases } from '../../scripts/public-chapter-feature-browser.mjs';

const origin = 'http://127.0.0.1:32101', sink = 'http://127.0.0.1:32102';
const manga = '11111111-1111-8111-8111-111111111111', chapter = '22222222-2222-8222-8222-222222222222';
const clone = value => JSON.parse(JSON.stringify(value));
const item = (native_id, canonical_kind) => ({ native_id, canonical_kind, event_id: 'a'.repeat(64), visible: true, projected: true, mapped: true });
const fixture = { mail_origin: sink, protocol_seed: { fixture_only: true, fully_ready: true, catalog_namespace: manga, series: item('SeedManga1', 1002), chapters: [item('SeedChapt1', 1001), item('SeedChapt2', 1001)] } };
const mapping = { result: 'ok', data: [{ kind: 'manga', legacy_id: 'SeedManga1', id: manga }, { kind: 'chapter', legacy_id: 'SeedChapt1', id: chapter }], unresolved: [] };
const record = { result: 'ok', data: { type: 'chapter', id: chapter, attributes: { version: 7, title: 'Synthetic original chapter', volume: '1', chapter: '2', translatedLanguage: 'en' }, relationships: [{ type: 'manga', id: manga }] } };
const pre = chapterFeaturePrerequisites(origin, fixture);

test('fixed chapter case is immutable and uses actual legacy fields, never invented public fixture UUIDs', () => {
  assert.deepEqual(publicChapterFeatureCases.map(row => row.id), ['chapter-edit-entry-signed']);
  assert.ok(Object.isFrozen(publicChapterFeatureCases) && Object.isFrozen(publicChapterFeatureCases[0]));
  assert.equal(pre.chapterLegacy, 'SeedChapt1');
  assert.deepEqual(chapterFeatureTargets(mapping, pre), { manga, chapter });
  assert.deepEqual(chapterFeatureRecord(record, { manga, chapter }), { title: 'Synthetic original chapter', volume: '1', chapter: '2', language: 'en' });
});
for (const value of ['https://example.invalid', 'http://user:secret@127.0.0.1:32101', origin + '/path', origin + '/?token=secret', origin + '/#fragment']) test('fixture origin refuses nonexact local scope: ' + value.split('@').at(-1), () => {
  assert.throws(() => chapterFeaturePrerequisites(value, fixture));
});
test('unready, wrong-kind and unproven source prerequisites fail closed', () => {
  for (const change of [f => { f.protocol_seed.fully_ready = false; }, f => { f.protocol_seed.series.canonical_kind = 1001; }, f => { f.protocol_seed.chapters[0].canonical_kind = 1002; }, f => { f.protocol_seed.chapters[0].visible = false; }, f => { f.protocol_seed.chapters[0].mapped = false; }, f => { f.protocol_seed.chapters[0].native_id = chapter; }]) {
    const f = clone(fixture); change(f); assert.throws(() => chapterFeaturePrerequisites(origin, f));
  }
});
test('typed resolution rejects wrong/missing/duplicate associations and unresolved targets', () => {
  for (const change of [m => { m.data[1].legacy_id = 'Different1'; }, m => { m.data[1].kind = 'manga'; }, m => { m.data.push(m.data[1]); }, m => { m.data.pop(); }, m => { m.data[1].id = 'SeedChapt1'; }, m => { m.unresolved = [{ kind: 'chapter', id: 'SeedChapt1' }]; }]) {
    const m = clone(mapping); change(m); assert.throws(() => chapterFeatureTargets(m, pre));
  }
});
test('canonical chapter read refuses another chapter/parent/type or untyped metadata', () => {
  for (const change of [r => { r.data.id = manga; }, r => { r.data.type = 'manga'; }, r => { r.data.relationships[0].id = chapter; }, r => { r.data.relationships.push(r.data.relationships[0]); }, r => { r.data.attributes.version = 0; }, r => { r.data.attributes.translatedLanguage = '../en'; }, r => { r.data.attributes.title = {}; }]) {
    const r = clone(record); change(r); assert.throws(() => chapterFeatureRecord(r, { manga, chapter }));
  }
});

async function controlled(options = {}) {
  const source = fs.readFileSync(new URL('../../scripts/public-chapter-feature-browser.mjs', import.meta.url), 'utf8');
  const state = { selected: false, requests: [], ui: [], handlers: new Set(), url: origin + '/search', secret: new Uint8Array(32).fill(7), generated: 0 };
  const local = vm.createContext({ Buffer, URL });
  const dependency = (name, values) => new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context: local, identifier: name });
  const module = new vm.SourceTextModule(source, { context: local });
  await module.link(name => name === 'node:assert/strict' ? dependency(name, { default: assert }) : dependency(name, { verifiedSchnorr: async () => ({ utils: { randomSecretKey() { state.generated++; return state.secret; } }, getPublicKey: () => new Uint8Array(32).fill(10) }) }));
  await module.evaluate();
  const response = (body, status = 200) => ({ status: () => status, body: async () => Buffer.from(JSON.stringify(body)) });
  let waitResponse, waitFinished;
  function locator(kind, label) {
    return {
      filter() { return this; },
      locator(selector) { return locator(kind === 'row' ? 'edit' : selector.startsWith('a[href="/chapter/') ? 'chapter-anchor' : 'leaf', selector); },
      getByRole(role, value = {}) { return locator(role === 'status' ? 'status' : role, value.name); },
      getByText(value) { return locator('text', String(value)); },
      getByLabel(value) { return locator('input', value); },
      async count() { if (kind === 'edit') return state.selected ? 1 : 0; if (kind === 'text' && label.includes('No signing identity')) return state.selected ? 0 : 1; if (kind === 'text' && label.includes('Session private key')) return state.selected ? 1 : 0; return 1; },
      async isEnabled() { return true; },
      async isDisabled() { return options.disabled !== false; },
      async getAttribute(name) { assert.equal(name, 'type'); return 'password'; },
      async fill(value, timeout) { assert.ok(timeout.timeout > 0); assert.equal(value, '07'.repeat(32)); state.ui.push('private-input-filled'); },
      async check(timeout) { assert.ok(timeout.timeout > 0); state.ui.push('explicit-synthetic-backup-acknowledgement'); },
      async inputValue(timeout) { assert.ok(timeout.timeout > 0); return ({ Title: record.data.attributes.title, Volume: '1', Chapter: '2', Language: 'en' })[label]; },
      async click(timeout) {
        assert.ok(timeout.timeout > 0); state.ui.push(label ?? kind);
        if (kind === 'settings') state.url = origin + '/settings/preferences';
        if (label === 'Public identity') state.url = origin + '/settings/identity';
        if (label === 'Use this key for this session') state.selected = true;
        if (kind === 'edit') {
          state.url = origin + '/chapter/' + chapter + '/edit';
          const request = { url: () => origin + '/api/v1/nostr/account-context', method: () => 'GET' };
          for (const callback of state.handlers) callback(request);
          const reply = { url: request.url, request: () => request, status: () => options.accountStatus ?? 403 };
          assert.equal(waitResponse.predicate(reply), true); waitResponse.resolve(reply);
          assert.equal(waitFinished.predicate(request), true);
          if (options.accountFinished === false) waitFinished.reject(Error('Synthetic missing account terminal')); else waitFinished.resolve(request);
        }
      }
    };
  }
  const page = {
    url: () => state.url,
    locator(selector) { return locator(selector === 'article' ? 'row' : selector.includes('/settings') ? 'settings' : selector === 'section' ? 'editor' : 'leaf', selector); },
    getByRole: (role, value) => locator(role, value.name),
    async goto(url, options) { state.titleUrl = url; state.url = url; assert.ok(options.timeout > 0); return response({}); },
    async goBack(options) { assert.ok(options.timeout > 0); state.url = state.url.endsWith('/settings/identity') ? origin + '/settings/preferences' : state.titleUrl; },
    async evaluate(action) { assert.equal(action.toString(), '() => performance.timeOrigin'); return 17; },
    async waitForTimeout(ms) { assert.ok(ms > 0 && ms <= 75); },
    waitForResponse(predicate, options) { assert.ok(options.timeout > 0 && options.timeout <= 10000); return new Promise(resolve => { waitResponse = { predicate, resolve }; }); },
    waitForEvent(event, options) { assert.equal(event, 'requestfinished'); assert.ok(options.timeout > 0 && options.timeout <= 10000); return new Promise((resolve, reject) => { waitFinished = { predicate: options.predicate, resolve, reject }; }); },
    on(event, callback) { assert.equal(event, 'request'); state.handlers.add(callback); },
    off(event, callback) { assert.equal(event, 'request'); state.handlers.delete(callback); }
  };
  async function call(method, url, requestOptions) {
    assert.equal(requestOptions.maxRedirects, 0); assert.ok(requestOptions.timeout > 0 && requestOptions.timeout <= 5000); state.requests.push({ method, url, data: requestOptions.data });
    if (url.endsWith('/api/auth/check')) return response({}, options.cookieStatus ?? 401);
    if (url.endsWith('/api/v1/legacy/resolve')) return response(mapping);
    if (url.includes('/api/v1/chapter/')) return response(record);
    assert.equal(url, sink + '/fixture/protocol/account');
    assert.deepEqual(Object.keys(requestOptions.data).sort(), ['allowed', 'pubkey']);
    assert.equal(requestOptions.data.pubkey, '0a'.repeat(32));
    return response({ fixture_only: true, pubkey: requestOptions.data.pubkey, allowed: true, publisher_permission_granted: false, private_key_received: options.privateKeyReceived ?? false });
  }
  const h = { page, context: { request: { get: (url, options) => call('GET', url, options), post: (url, options) => call('POST', url, options) } }, origin, fixture, insist: assert.ok, redactions: options.redactionThrows ? { add() { throw Error('Synthetic redaction refusal'); } } : new Set() };
  return { state, run: () => module.namespace.publicChapterFeatureCases[0].run(h) };
}
test('actual case traverses UI/typed HTTP stand-ins, observes real-refusal shape and never provisions upload/native authority', async () => {
  const f = await controlled(), observation = await f.run();
  assert.deepEqual(clone(observation), { actual_identity_ui: true, no_cookie_session: true, typed_target_association: true, edit_link_visible: true, canonical_edit_child_mounted: true, actual_account_status: 403, account_request_finished: true, mutation_controls_disabled: true, native_binding_ready_tested: false, metadata_save_tested: false });
  assert.equal(f.state.handlers.size, 0); assert.ok(f.state.secret.every(value => value === 0));
  assert.equal(f.state.requests.filter(row => row.url.endsWith('/api/auth/check')).length, 2);
  assert.equal(f.state.requests.some(row => row.url.includes('upload-account')), false);
  assert.ok(f.state.requests.filter(row => row.method === 'POST').every(row => row.url.endsWith('/legacy/resolve') || row.url.endsWith('/fixture/protocol/account')));
});
for (const options of [{ accountStatus: 200 }, { disabled: false }, { privateKeyReceived: true }]) test('unexpected authority/fixture outcome refuses and clears owned observer/secret: ' + JSON.stringify(options), async () => {
  const f = await controlled(options); await assert.rejects(f.run); assert.equal(f.state.handlers.size, 0); assert.ok(f.state.secret.every(value => value === 0));
});
test('a403 header without the exact completed child request cannot qualify account refusal', async () => {
  const f = await controlled({ accountFinished: false }); await assert.rejects(f.run, /Synthetic missing account terminal/); assert.equal(f.state.handlers.size, 0); assert.ok(f.state.secret.every(value => value === 0));
});
test('redaction admission failure clears the newly created in-memory secret before any account action', async () => {
  const f = await controlled({ redactionThrows: true }); await assert.rejects(f.run, /Synthetic redaction refusal/); assert.ok(f.state.secret.every(value => value === 0)); assert.equal(f.state.requests.some(row => row.url.endsWith('/fixture/protocol/account')), false);
});
test('native-cookie admission is refused before creating a synthetic signer', async () => {
  const f = await controlled({ cookieStatus: 200 }); await assert.rejects(f.run); assert.equal(f.state.generated, 0);
});
test('browser source preserves real UI/HTTP and read-only chapter boundary without interception or activation', () => {
  const source = fs.readFileSync(new URL('../../scripts/public-chapter-feature-browser.mjs', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /\.route\(|\.fulfill\(|addInitScript|exposeFunction|setExtraHTTPHeaders|upload-account|process\.env|console\.|writeFile|chromium\.launch/);
  assert.match(source, /\/api\/v1\/legacy\/resolve/);
  assert.match(source, /childReads\.has\(response.request\(\)\)/);
  assert.match(source, /native_binding_ready_tested: false, metadata_save_tested: false/);
  assert.match(source, /const CASE_MS = 45000/);
});
