import {restoreReaderUxPredecessor} from './reviewed-reader-ux-source-successor.mjs';
import { restoreUploadFinalization } from './upload-finalization-source-association.mjs';
// Finite actual-composition/source-extracted locator seams only. No browser,
// Svelte runtime, signer, HTTP, database, dependency install or service is used.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
import { createPublicUploadOverlays } from '../../platform-integrations/public-api/upload-overlays.mjs';
import { createBoundedReaderOverlays } from '../../platform-integrations/bounded-reader/overlays.mjs';
import { uniqueUploadControl } from '../../scripts/public-upload-key-import.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const read = relative => fs.readFileSync(path.join(root, relative), 'utf8');
const hash = value => createHash('sha256').update(value).digest('hex');
const source = read('scripts/public-upload-recovery-browser.mjs');
const baseline = read('tests/fixtures/qualification-baselines/sources/WEEBDEX_UPLOAD_SECOND_BEGIN_BASELINE_SOURCE/public-upload-recovery-browser.mjs.txt');
assert.equal(hash(baseline), '808d27e183168c5b53f2e37317789b79d21601b75eb515c53d68824766605307');
const relative = 'src/lib/components/upload/UploadChapterMetadata.svelte';
const original = read('frontend/' + relative);

function compose(enabled) {
  const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend),
    createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled }),
    createPublicUploadOverlays(frontend, { enabled }), createBoundedReaderOverlays(frontend)];
  let code = original;
  for (const plugin of plugins) code = plugin.transform.handler(code, path.join(frontend, relative))?.code ?? code;
  return code;
}
const composed = compose(true);
const inputs = [...composed.matchAll(/<select\b[^>]*\bbind:value=\{draft\.language\}[^>]*>/g)];
assert.equal(inputs.length, 1, 'Use the single real composed language selector');
const inputSource = inputs[0][0];

function block(text, start, end) {
  const at = text.indexOf(start); assert.ok(at >= 0); assert.equal(text.indexOf(start, at + start.length), -1);
  const stop = text.indexOf(end, at + start.length); assert.ok(stop > at);
  return text.slice(at, stop);
}
const englishBlock = text => block(text, '  async function english()', '\n  async function decoded');
const metadataBlock = text => block(text, '  async function restoredMetadata()', '\n  async function visibleCompletion');
function actualFunction(text, env, name, sourceBlock) {
  return new Function('env', `with (env) { ${sourceBlock(text)}; return ${name}; }`)(env);
}

function harness(options = {}, text = source) {
  let now = 0, ticks = 0, countReads = 0, enabledReads = 0;
  const state = { value: options.language ?? '', fills: [], marks: [], routeChecks: 0 };
  const missing = { count: async () => 0, filter() { return this; } };
  const input = {
    count: async () => options.duplicate ? 2 : options.missing || ++countReads <= (options.attachDelay ?? 0) ? 0 : 1,
    getAttribute: async name => { assert.equal(name, 'aria-label'); return options.inputLabel ?? 'Language'; },
    isEnabled: async () => !options.disabled && ++enabledReads > (options.enableDelay ?? 0),
    async selectOption({value}) {
      if (options.fillFailure) throw options.fillFailure;
      state.fills.push(value); if (!options.ignoreFill) state.value = value;
    },
    inputValue: async () => state.value,
  };
  const panel = {
    getByLabel(name, settings) { assert.equal(name, 'Language'); assert.deepEqual(settings, { exact: true }); return input; },
    getByPlaceholder(name, settings) {
      assert.deepEqual(settings, { exact: true });
      assert.ok(['Chapter Name', 'Chapter No.'].includes(name));
      return { inputValue: async () => name === 'Chapter Name' ? options.title ?? 'Synthetic public upload recovery' : options.chapter ?? '9001' };
    },
    locator(selector) { assert.equal(selector, 'button[data-slot="select-trigger"]'); return missing; },
  };
  const env = {
    assert, Boolean, Date: { now: () => now },
    mark: value => state.marks.push(value), panel: async () => panel,
    healthy() { if (options.healthFailure && ticks > 0) throw options.healthFailure; },
    assertUploadRoute() { state.routeChecks++; if (options.routeFailure) throw options.routeFailure; },
    page: { async waitForTimeout(ms) { assert.equal(ms, 100); now += ms; ticks++; } },
  };
  env.until = actualFunction(text, env, 'until', value => block(value, '  async function until(', '\n  const unique ='));
  env.unique = locator => uniqueUploadControl(locator, env.until);
  return { state, ticks: () => ticks, english: actualFunction(text, env, 'english', englishBlock),
    restoredMetadata: actualFunction(text, env, 'restoredMetadata', metadataBlock) };
}

test('current public composition names languages and preserves imported values while disabled mode retains native Combobox', () => {
  assert.equal(hash(original), 'c94de7465c23c046f6277dae37266b845b62ba8c5097980a3629014f6f3d7ff9');
  assert.match(inputSource, /aria-label="Language"/); assert.match(inputSource, /required \{disabled\}/);
  assert.match(composed, /Object.entries\(Locale.Languages\)/); assert.match(composed, /Previously saved language/);
  assert.match(compose(false), /<Combobox\.Root type="single" bind:value=\{draft\.language\}/);
});

test('exact pre-selector UX predecessor preserves finalization and language comparison to run40133800331', () => {
  const predecessor = restoreReaderUxPredecessor('weebdex-platform/scripts/public-upload-recovery-browser.mjs',source);
  const reverted = restoreUploadFinalization(predecessor).replace(englishBlock(predecessor), englishBlock(baseline)).replace(metadataBlock(predecessor), metadataBlock(baseline));
  assert.equal(hash(reverted), '2c1edd5124418e1fb7c1d8a50e93f16aa2bc10bcbb42e7512f7dc2a47392975d');
  for (const code of [englishBlock(source), metadataBlock(source)]) {
    assert.doesNotMatch(code, /select-trigger|select-content|select-item|\.evaluate\(|dispatchEvent|fetch\(|request\(|force:/);
  }
});

for (const options of [{}, { attachDelay: 3, enableDelay: 2 }]) test('real english action selects the exact language value after bounded readiness ' + JSON.stringify(options), async () => {
  const h = harness(options); await h.english();
  assert.deepEqual(h.state.fills, ['en']); assert.equal(h.state.value, 'en');
  assert.deepEqual(h.state.marks, ['language']); assert.equal(h.state.routeChecks, 1);
  assert.equal(h.ticks(), (options.attachDelay ?? 0) + (options.enableDelay ?? 0));
});
for (const options of [{ missing: true }, { disabled: true }]) test('missing or permanently disabled public input retains the existing 15-second readiness failure ' + JSON.stringify(options), async () => {
  const h = harness(options); await assert.rejects(h.english(), error => error.code === 'UPLOAD_UI_READINESS_TIMEOUT');
  assert.equal(h.ticks(), 150); assert.deepEqual(h.state.fills, []);
});
test('duplicate input fails closed before a fill', async () => {
  const h = harness({ duplicate: true }); await assert.rejects(h.english(), error => error.code === 'UPLOAD_UI_DUPLICATE_CONTROL');
  assert.deepEqual(h.state.fills, []); assert.equal(h.ticks(), 0);
});
test('an unexpected control label is not used', async () => {
  const h = harness({ inputLabel: 'Password' }); await assert.rejects(h.english(), error => error.code === 'ERR_ASSERTION');
  assert.deepEqual(h.state.fills, []);
});
test('the genuine fill must actually change the observed input value', async () => {
  const h = harness({ ignoreFill: true }); await assert.rejects(h.english(), error => error.code === 'ERR_ASSERTION');
  assert.deepEqual(h.state.fills, ['en']); assert.equal(h.state.routeChecks, 0);
});
for (const property of ['fillFailure', 'healthFailure', 'routeFailure']) test('original ' + property + ' remains an unreplaced failure', async () => {
  const failure = Error('controlled ' + property), h = harness({ [property]: failure, ...(property === 'healthFailure' ? { enableDelay: 2 } : {}) });
  await assert.rejects(h.english(), error => error === failure);
});
test('restored metadata reads the exact language code without changing it', async () => {
  const h = harness({ language: 'en' }); await h.restoredMetadata();
  assert.deepEqual(h.state.fills, []); assert.deepEqual(h.state.marks, ['metadata-read']); assert.equal(h.state.routeChecks, 1);
});
for (const options of [{ language: 'English' }, { language: 'EN' }, { language: 'en ' }, { language: 'ja' },
  { language: 'en', title: 'changed' }, { language: 'en', chapter: '9002' }]) test('restored metadata refuses changed values ' + JSON.stringify(options), async () => {
  const h = harness(options); await assert.rejects(h.restoredMetadata(), error => error.code === 'UPLOAD_UI_READINESS_TIMEOUT');
  assert.equal(h.ticks(), 150); assert.deepEqual(h.state.fills, []);
});
test('exact retained old english action cannot find its absent select trigger in the actual public composition', async () => {
  const h = harness({}, baseline); await assert.rejects(h.english(), error => error.code === 'UPLOAD_UI_READINESS_TIMEOUT');
  assert.equal(h.ticks(), 150); assert.deepEqual(h.state.fills, []); assert.deepEqual(h.state.marks, ['language']);
});
