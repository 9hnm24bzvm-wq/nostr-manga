#!/usr/bin/env node
// Exact composed keyboard-handler and actual outer-checkpoint seams. DOM,
// browser transport and inner journey operations are explicit stand-ins.
// This starts no browser or service and does not establish hosted acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { runUi10Journeys, ui10DiagnosticChecks } from '../../scripts/browser-ui10-journeys.mjs';
import { ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { focusReaderShortcut } from '../../scripts/browser-reader-controls.mjs';
import { runHeldAccountDenial } from '../../scripts/browser-account-capacity.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';

const controls = ['403', '404', 'delayed', 'normal'];
const suffixes = ['open', 'page-one', 'image-bytes', 'settings', 'shortcut-focus', 'navigation', 'destination'];
const ids = [...controls.flatMap(control => suffixes.map(suffix => `reader-transition-${control}-${suffix}`)), ...['title-control','title-route','chapter-link','page-one','image-bytes','realm','progress','incomplete','settled'].map(suffix => `reader-transition-normal-destination-${suffix}`)];
const accountSuffixes = ['prerequisite', 'open', 'account-mode', 'durable-baseline', 'trigger', 'option', 'pressure-captured', 'capacity-ready', 'pressure-released', 'denial', 'feedback', 'no-replay', 'durable-unchanged'];

test('new phase inventories are fixed, ordered and frozen', () => {
  assert.deepEqual(ui10DiagnosticChecks['ui10-reader-transition'], ids);
  assert.deepEqual(ui11To16DiagnosticChecks['ui14-account-failures'], accountSuffixes.map(suffix => `account429-title-${suffix}`));
  for (const value of [ui10DiagnosticChecks['ui10-reader-transition'], ui11To16DiagnosticChecks['ui14-account-failures']]) {
    assert(Object.isFrozen(value)); assert.equal(new Set(value).size, value.length);
  }
});

function keyboardSeam() {
  const frontend = path.resolve(new URL('../../frontend', import.meta.url).pathname);
  const file = path.join(frontend, 'src/lib/components/reader/reader-keyboard-listener.svelte');
  let code = fs.readFileSync(file, 'utf8');
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)]) code = plugin.transform.handler(code, file)?.code ?? code;
  const body = code.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^\n]*\n/gm, '');
  const script = stripTypeScriptTypes(body);
  class Element { constructor(owned = false) { this.owned = owned; } closest() { return this.owned ? this : null; } }
  class HTMLElement extends Element { isContentEditable = false; }
  const prefs = { layout: 'paged', direction: 'ltr' }, calls = [];
  const reader = { prefs, scroll: {}, gotoPreviousPage() { calls.push('previous'); } };
  const handler = vm.runInNewContext(script + '\nonKeyDown;', {
    Element, HTMLElement, Constants: { Reader: { Layout: { Scroll: 'scroll' }, Direction: { RTL: 'rtl' } } },
    $readerPrefs: prefs, useReader: () => reader, getIsOpen: () => false,
    onDestroy() {}, onMount() {}, window: { cancelAnimationFrame() {} },
  });
  const press = (target, extra = {}) => handler({ code: 'ArrowLeft', target, composedPath: () => [target], preventDefault() {}, stopPropagation() {}, stopImmediatePropagation() {}, ...extra });
  return { Element, HTMLElement, calls, press };
}

test('actual composed reader handler retains focused-control protection', () => {
  const { Element, HTMLElement, calls, press } = keyboardSeam();
  press(new Element(true)); // Button or nested SVG whose closest is a button.
  const editable = new HTMLElement(); editable.isContentEditable = true; press(editable);
  press(new Element(), { defaultPrevented: true });
  assert.deepEqual(calls, []);
  press(new Element());
  assert.deepEqual(calls, ['previous']);
});

test('real shortcut helper clicks the inert retained label and verifies focus', async () => {
  const calls = [];
  const page = {
    getByText(label, options) { assert.equal(label, 'Direction'); assert.deepEqual(options, { exact: true }); return { async click() { calls.push('real-label-click'); } }; },
    async evaluate(operation) {
      calls.push('verify-focus');
      return vm.runInNewContext('(' + operation.toString() + ')()', { document: { activeElement: { closest: () => null } } });
    },
  };
  await focusReaderShortcut(page, (condition, message) => assert(condition, message));
  assert.deepEqual(calls, ['real-label-click', 'verify-focus']);
  page.evaluate = async () => false;
  await assert.rejects(focusReaderShortcut(page, (condition, message) => assert(condition, message)), /still owned/);
});

for (const failAt of [undefined, ...ids]) test('actual reader checkpoints propagate and clean up: ' + (failAt ?? 'complete'), async () => {
  const report = {}, snapshots = [], steps = [], routes = new Map(), failure = Error('private synthetic navigation state');
  const record = createJourneyCheck(report, () => snapshots.push(structuredClone(report)));
  await runUi10Journeys({ phase: 'ui10-reader-transition', origin: 'http://127.0.0.1:1', fixture: { ids: { manga: 'm', chapter: 'p', next_chapter: 'c', alternative_language_chapter: 'u' } }, redactions: new Set(),
    page: { async evaluate() {} },
    context: {
      request: { async fetch() { return { status: () => 200, headers: () => ({ 'content-type': 'application/json' }), async json() { return { data: [1, 2, 3] }; } }; } },
      async route(pattern, intercept) { assert(!routes.has(pattern)); routes.set(pattern, intercept); },
      async unroute(pattern, intercept) { assert.equal(routes.get(pattern), intercept); routes.delete(pattern); },
    },
    insist(condition, message) { assert(condition, message); },
    async step(name, operation) {
      const row = { name }; steps.push(row);
      if (steps.length === 1) return; // Original prerequisite is outside this seam.
      try { await operation(); row.status = 'passed'; } catch (error) { row.status = 'failed'; row.error = error; }
    },
    async check(id, operation) { assert(ids.includes(id)); assert.equal(typeof operation, 'function'); return record(id, async () => { if (id === failAt) throw failure; if (id === 'reader-transition-normal-destination') return operation(); }); },
  });
  assert.equal(steps.length, 5);
  assert.equal(routes.size, 0);
  assert.equal(steps.filter(row => row.status === 'failed').length, failAt ? 1 : 0);
  if (failAt) assert.equal(steps.find(row => row.status === 'failed').error, failure);
  const failedControl = failAt && controls.find(control => failAt.startsWith(`reader-transition-${control}-`));
  const reached = ids.filter(id => !failAt || !id.startsWith(`reader-transition-${failedControl}-`) || ids.indexOf(id) <= ids.indexOf(failAt));
  assert.deepEqual(report.journey_checks.map(row => row.check_id), reached);
  assert.deepEqual(report.journey_checks.map(row => row.status), reached.map(id => id === failAt || id === 'reader-transition-normal-destination' && failAt?.startsWith('reader-transition-normal-destination-') ? 'failed' : 'passed'));
  assert.doesNotMatch(JSON.stringify(snapshots), /private synthetic|127\.0\.0\.1/);
});

const pressureChecks = ['pressure-captured', 'capacity-ready', 'pressure-released'];
for (const failAt of [undefined, 'gesture', 'capacity', 'denial', ...pressureChecks]) test('single held title gesture owns pressure cleanup: ' + (failAt ?? 'complete'), async () => {
  const seen = [], failure = Error('private capacity state'); let closed = 0;
  const act = async id => { seen.push(id); if (failAt === id) throw failure; };
  const execute = runHeldAccountDenial({
    hold: async (pattern, match, before) => { assert.equal(pattern, 'fixed'); assert.equal(typeof match, 'function'); assert.equal(before, true); return { wait: () => act('wait'), release: () => act('release'), async close() { closed++; } }; },
    pattern: 'fixed', match() {}, gesture: () => act('gesture'), denial: () => act('denial'),
    async capacity(operation) { await act('capacity'); return operation(); },
    async check(id, operation) { await act(id); return operation(); },
  });
  if (failAt) await assert.rejects(execute, error => error === failure); else await execute;
  assert.equal(closed, 1);
  assert.equal(seen.filter(id => id === 'gesture').length, 1);
  assert.equal(seen.filter(id => id === 'release').length, !failAt || failAt === 'denial' ? 1 : 0);
  if (!failAt) assert.deepEqual(seen, ['gesture', 'pressure-captured', 'wait', 'capacity', 'capacity-ready', 'pressure-released', 'release', 'denial']);
});

function actualAccountStep() {
  const source = fs.readFileSync(new URL('../../scripts/browser-ui11-16-journeys.mjs', import.meta.url), 'utf8');
  const start = source.indexOf('for (const status of [403, 429, 401]) for (const gesture of phase === "ui15-reader-feedback"');
  assert(start >= 0);
  const action = source.indexOf('async () => {', start) + 'async () => {'.length;
  const end = source.indexOf('\n      });\n      if (phase === "ui15-reader-feedback")', action);
  assert(end > action);
  return source.slice(action, end);
}

for (const failAt of [undefined, ...accountSuffixes]) test('actual title429 checkpoint body and cleanup: ' + (failAt ?? 'complete'), async () => {
  const report = {}, seen = [], failure = Error('private synthetic account denial');
  const record = createJourneyCheck(report);
  let observed = 0, observersClosed = 0, holds = 0, holdsClosed = 0;
  const locator = { filter() { return this; }, last() { return this; }, async click() {}, async waitFor() {} };
  const fixtureApi = async route => route === '/api/manga/m' ? { status: 200, body: { title: 'synthetic title' } } : route.startsWith('/api/user/follows/manga/m') ? { status: 204 } : { status: 200, body: {} };
  const bindings = {
    phase: 'ui14-account-failures', status: 429, gesture: 'title-unfollow', origin: 'http://127.0.0.1:1', ids: { manga: 'm', chapter: 'c' },
    check: (id, operation) => record(id, async () => { seen.push(id); if (id === `account429-title-${failAt}`) throw failure; return operation(); }),
    api: fixtureApi, open: async () => {}, ready: async () => {}, mode: async () => {},
    page: { getByText: () => locator }, context: {},
    observe() { observed++; return { rows: [{ status: 429 }], requests: 1, async settle() {}, async close() { observersClosed++; } }; },
    insist(condition, message) { assert(condition, message); },
    selectListboxTrigger: () => locator, clickListboxOption: async () => {},
    async until(read, valid, message) { const value = await read(); assert(await valid(value), message); return value; },
    async stable(predicate, message) { assert(await predicate(), message); },
    hold: async () => { holds++; return { async wait() {}, async release() {}, async close() { holdsClosed++; } }; },
    withCapacity: async operation => operation(), runHeldAccountDenial,
  };
  const execute = new (Object.getPrototypeOf(async function() {}).constructor)(...Object.keys(bindings), actualAccountStep())(...Object.values(bindings));
  if (failAt) await assert.rejects(execute, error => error === failure); else await execute;
  assert.equal(observed, observersClosed);
  assert.equal(holds, holdsClosed);
  const reached = failAt ? accountSuffixes.slice(0, accountSuffixes.indexOf(failAt) + 1) : accountSuffixes;
  assert.deepEqual(seen, reached.map(suffix => `account429-title-${suffix}`));
  assert.deepEqual(report.journey_checks.map(row => row.status), reached.map(suffix => suffix === failAt ? 'failed' : 'passed'));
  assert.doesNotMatch(JSON.stringify(report), /private synthetic|127\.0\.0\.1/);
});

test('focus transfer target is unique inert retained settings markup', () => {
  const settings = fs.readFileSync(new URL('../../frontend/src/lib/components/reader/reader-settings.svelte', import.meta.url), 'utf8');
  assert.equal((settings.match(/>Direction<\/ButtonGroup\.Text>/g) ?? []).length, 1);
  const textComponent = fs.readFileSync(new URL('../../frontend/src/lib/components/ui/button-group/button-group-text.svelte', import.meta.url), 'utf8');
  assert.match(textComponent, /<div bind:this=\{ref\} \{\.\.\.mergedProps\}>/);
  assert.doesNotMatch(textComponent, /onclick=/);
});

test('missing inert target rejects before any focus assertion or fallback', async () => {
  const failure = Error('missing or ambiguous inert text'); let reads = 0;
  await assert.rejects(focusReaderShortcut({ getByText: () => ({ async click() { throw failure; } }), async evaluate() { reads++; } }, assert), error => error === failure);
  assert.equal(reads, 0);
});
