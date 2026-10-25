#!/usr/bin/env node
// Execute the exact installed, hash-pinned Trigger props expression with explicit
// rune/DOM stand-ins, then exercise the actual harness selector. No browser,
// network, services, database, package installation, or application success claim.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { selectTitleLibraryStatus, titleLibraryControlName, titleLibraryTrigger } from '../../scripts/browser-select-controls.mjs';
import { expandDetailsPanel } from '../../scripts/browser-details-controls.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = process.env.WEEBDEX_FRONTEND_ROOT;
if (!frontend) throw Error('Set WEEBDEX_FRONTEND_ROOT to the existing verified exact-lock dependency staging copy');
const bits = path.join(frontend, 'node_modules/bits-ui');
const pins = {
  'dist/bits/select/select.svelte.js': 'f608bf550819a996ac412d87de666349f4585ea79fc1822226780405ec9707c1',
  'dist/bits/select/components/select-trigger.svelte': '0f9094d7a347aef7eed751becb56957144b066180797fe4e40673c5950cc294c',
};
const source = Object.fromEntries(Object.entries(pins).map(([relative, expected]) => {
  const bytes = fs.readFileSync(path.join(bits, relative));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), expected, relative);
  return [relative, bytes.toString('utf8')];
}));
assert.equal(JSON.parse(fs.readFileSync(path.join(bits, 'package.json'))).version, '2.16.5');
const lock = fs.readFileSync(path.join(root, 'frontend/yarn.lock'), 'utf8');
assert.match(lock, /bits-ui@\^2\.14\.4:\r?\n  version "2\.16\.5"\r?\n  resolved "https:\/\/registry\.yarnpkg\.com\/bits-ui\/-\/bits-ui-2\.16\.5\.tgz#229f14ad4ac05ff47f06ccab273cdcad7534d805"\r?\n  integrity sha512-Dx\+Sc1DGzRaRfzpxrBI40d\+O7vykIZ8E2G6tCaOzqYLzg\+mN3YYK\+8f8ysAFLfkHzey1MACGrYO3IcO6ROKlSA==/);

function actualTriggerProps() {
  const module = source['dist/bits/select/select.svelte.js'];
  const start = module.indexOf('export class SelectTriggerState {');
  const end = module.indexOf('export class SelectContentState {', start);
  const props = module.slice(start, end).match(/props = \$derived\.by\(\(\) => \(([\s\S]*?)\)\);/);
  assert.ok(props, 'Actual pinned Trigger props initializer must be located exactly');
  const operation = vm.runInNewContext('(function(){ return (' + props[1] + '); })', {
    boolToStr: value => String(value), boolToEmptyStrOrUndef: value => value ? '' : undefined,
    getDataOpenClosed: value => value ? 'open' : 'closed',
  });
  return operation.call({ opts: { id: { current: 'synthetic-trigger' } },
    root: { opts: { disabled: { current: false }, open: { current: false } }, hasValue: false, getBitsAttr: value => 'data-select-' + value },
    attachment: {},
  });
}

test('retained title and pinned package establish native button semantics', () => {
  const retained = fs.readFileSync(path.join(root, 'frontend/src/routes/(app)/title/MangaActions.svelte'), 'utf8');
  assert.match(retained, /<div class="min-w-45">/);
  const trigger = retained.match(/<SelectPrimitive\.Trigger\b[^>]*>/)[0];
  assert.match(trigger, /data-slot="select-trigger"/); assert.doesNotMatch(trigger, /role=/);
  assert.match(source['dist/bits/select/components/select-trigger.svelte'], /<button \{\.\.\.mergedProps\}>/);
  const props = actualTriggerProps();
  assert.equal(props.role, undefined); assert.equal(props['aria-haspopup'], 'listbox');
  assert.equal(props['aria-expanded'], 'false');
});

function pageStandIn(name = 'Add to Library') {
  const events = [], trigger = { tag: 'button', ...actualTriggerProps(), name, titleLibrary: true };
  const decoys = [
    { tag: 'button', name: 'Add to Library', 'aria-haspopup': 'dialog', titleLibrary: true },
    { tag: 'button', name: 'Reading', 'aria-haspopup': 'listbox', titleLibrary: false },
    { tag: 'input', role: 'combobox', name: 'Chapter language', titleLibrary: false },
  ];
  const all = [trigger, ...decoys];
  const locator = rows => ({
    and(other) { return locator(rows.filter(row => other.rows.includes(row))); }, rows,
    filter({ hasText }) { return locator(rows.filter(row => hasText.test(row.name))); },
    async click() { assert.equal(rows.length, 1, 'A specific real control must be selected'); events.push(rows[0] === trigger ? 'title-trigger' : 'Reading-option'); },
    async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); assert.equal(rows.length, 1, 'Readback must find the actual selected title status'); events.push('Reading-visible'); },
  });
  const page = {
    getByRole(role, options) {
      if (role === 'option') { assert.equal(options.name, 'Reading'); assert.equal(options.exact, true); return locator([{ role: 'option', name: 'Reading' }]); }
      return locator(all.filter(row => (row.role ?? (row.tag === 'button' ? 'button' : undefined)) === role && (options.name instanceof RegExp ? options.name.test(row.name) : options.name === row.name)));
    },
    locator(selector) { assert.equal(selector, 'div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible'); return locator(all.filter(row => row.tag === 'button' && row.titleLibrary && row['aria-haspopup'] === 'listbox')); },
  };
  return { page, events };
}

test('old combobox role cannot locate actual title Trigger; repaired scoped selector can', async () => {
  const old = pageStandIn();
  await assert.rejects(old.page.getByRole('combobox', { name: titleLibraryControlName }).click(), /specific real control/);
  assert.deepEqual(old.events, []);
  const repaired = pageStandIn(); await selectTitleLibraryStatus(repaired.page, 'Reading');
  assert.deepEqual(repaired.events, ['title-trigger', 'Reading-option']);
});

test('current followed statuses stay selectable without matching unrelated button names', () => {
  for (const label of ['Add to Library', 'Reading', 'Plan to Read', 'On Hold', 'Completed', 'Dropped', 'Re-reading']) assert.ok(titleLibraryControlName.test(label));
  for (const label of ['Continue Reading', 'Start Reading', 'Reading Settings', 'Use device library', 'Add to Library elsewhere']) assert.equal(titleLibraryControlName.test(label), false);
});

test('Reading reload assertion uses the same exact native title Trigger', async () => {
  const current = pageStandIn('Reading'); await titleLibraryTrigger(current.page, 'Reading').waitFor({ state: 'visible' });
  assert.deepEqual(current.events, ['Reading-visible']);
  const wrong = pageStandIn('Completed');
  await assert.rejects(titleLibraryTrigger(wrong.page, 'Reading').waitFor({ state: 'visible' }), /actual selected title status/);
  const journey = fs.readFileSync(path.join(root, 'scripts/browser-journeys.mjs'), 'utf8');
  assert.match(journey, /const follow = status => selectTitleLibraryStatus\(page, status\)/);
  assert.match(journey, /await titleLibraryTrigger\(page, 'Reading'\)\.waitFor\(\{ state: 'visible' \}\)/);
  assert.doesNotMatch(journey, /getByRole\('combobox', \{ name: 'Reading'/);
});

test('closed, empty-string open, and already-open native details expand idempotently', async () => {
  for (const initial of [null, '', 'open']) {
    let attribute = initial, clicks = 0, waits = 0;
    const target = {
      async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); waits++; },
      async getAttribute(name) { assert.equal(name, 'open'); return attribute; },
      locator(selector) { assert.equal(selector, ':scope > summary'); return { async click() { clicks++; attribute = attribute === null ? '' : null; } }; },
    };
    await expandDetailsPanel(target); await expandDetailsPanel(target);
    assert.notEqual(attribute, null); assert.equal(clicks, initial === null ? 1 : 0); assert.equal(waits, 2);
  }
  const journey = fs.readFileSync(path.join(root, 'scripts/browser-journeys.mjs'), 'utf8');
  assert.match(journey, /openLibraryTool\('Manage lists', 'lists'\)/);
  assert.match(journey, /inspect\('signer-section-open', async \(\) => \{ await open\('\/settings\/identity'\); await target\.waitFor\(\{ state: 'visible' \}\)/);
  assert.doesNotMatch(journey, /expandDetailsPanel\(panel\(\)\)/);
});

test('actual library-management helper follows the exact native Library Settings link to one section', async () => {
  const journey = fs.readFileSync(path.join(root, 'scripts/browser-journeys.mjs'), 'utf8');
  const start = journey.indexOf('\tconst panel = '), end = journey.indexOf('\n\tconst follow = ', start);
  assert(start > 0 && end > start);
  const management = journey.slice(start, end);
  for (const duplicates of [0, 1, 2]) {
    const events = []; let route = '/title/synthetic';
    const scope = vm.createContext({
      async open(target) { assert.equal(target, '/library'); events.push('library-route'); route = target; },
      page: {
        getByRole(role, options) {
          assert.equal(role, 'link'); assert.equal(options.name, 'Manage library'); assert.equal(options.exact, true);
          return {
            async getAttribute(name) { assert.equal(name, 'href'); events.push('management-href'); return '/settings/library'; },
            async click() { assert.equal(route, '/library'); events.push('management-link'); route = '/settings/library'; },
          };
        },
        async waitForURL(accepts) {
          assert(accepts(new URL('http://127.0.0.1:4000' + route)));
          for (const rejected of ['/library', '/settings/preferences', '/settings/preferences#local-library', '/settings/library#other', '/settings/identity']) assert.equal(accepts(new URL('http://127.0.0.1:4000' + rejected)), false);
          events.push('settings-route');
        },
        locator(selector) {
          assert.equal(selector, 'section[data-local-library-panel]');
          return { async waitFor(options) { assert.equal(options.state, 'visible'); assert.deepEqual(Object.keys(options), ['state']); assert.equal(route, '/settings/library'); assert.equal(duplicates, 1, 'normal management section must be visible and unique'); events.push('settings-section'); } };
        },
      },
      insist(condition, message) { assert(condition, message); },
    });
    vm.runInContext(management, scope);
    const run = vm.runInContext('openLibraryManagement', scope);
    if (duplicates === 1) { await run(); assert.deepEqual(events, ['library-route', 'management-href', 'management-link', 'settings-route', 'settings-section']); }
    else await assert.rejects(run(), /management section must be visible and unique/);
  }
});
