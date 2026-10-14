#!/usr/bin/env node
// Finite checkpoint wiring and native-control diagnostic seams only. The real
// checkpoint bodies in the journey wiring probes are explicit stand-ins. No
// browser, network, application, fixture, signer, service or database is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { titleFollowCheckSuffixes, selectTitleLibraryStatus, nativeSelectForLabel } from '../../scripts/browser-select-controls.mjs';
import { localLibraryDiagnosticChecks } from '../../scripts/browser-journeys.mjs';
import { signedJourneyPhases, signedLibraryDiagnosticChecks, signedDownstreamCheckSuffixes } from '../../scripts/browser-signed-journeys.mjs';

const localSource = fs.readFileSync(new URL('../../scripts/browser-journeys.mjs', import.meta.url), 'utf8');
const signedSource = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const localSuffixes = [
  'device-title-navigation', 'device-follow-trigger-visible-unique', 'device-follow-popup-open', 'device-follow-option-selected',
  'device-status-durable', 'device-authority-unpublished', 'device-reload-status', 'device-library-navigation',
  'custom-list-navigation', 'custom-list-durable', 'custom-member-durable', 'custom-backup-export', 'custom-foreign-backup-fenced',
  'custom-backup-merged', 'account-mode-ready', 'account-follow-trigger-visible-unique', 'account-follow-popup-open',
  'account-follow-option-selected', 'account-status-durable', 'account-device-preserved', 'account-import-reviewed',
  'signer-section-open', 'signer-binding-installed', 'signer-dismissal-visible', 'signer-outbox-empty'
];
const signedSuffixes = [
  'mapped-title-navigation', 'mapped-follow-trigger-visible-unique', 'mapped-follow-popup-open', 'mapped-follow-option-selected',
  'mapped-status-durable', 'native-title-navigation', 'native-follow-trigger-visible-unique', 'native-follow-popup-open',
  'native-follow-option-selected', 'reader-bytes', 'reader-progress-durable', 'library-navigation', 'device-list-durable',
  'device-member-durable', 'device-authority-unpublished', 'missing-signer-refused', 'signer-binding-installed',
  'signer-selection-denied', 'signer-connected', 'recipient-wrong-key-refused', 'recipient-disallowed-origin-refused',
  'recipient-verified', 'recipient-selected'
];
const expected = { 'local-library': localSuffixes.map(suffix => 'local-library-' + suffix),
  ...Object.fromEntries(signedJourneyPhases.map(phase => [phase, [...signedSuffixes, ...(signedDownstreamCheckSuffixes[phase] ?? [])].map(suffix => phase + '-' + suffix)])) };

test('closed inventories are fixed, unique, deeply frozen and phase scoped', () => {
  assert.deepEqual(titleFollowCheckSuffixes, ['trigger-visible-unique', 'popup-open', 'option-selected']);
  assert(Object.isFrozen(titleFollowCheckSuffixes));
  assert.deepEqual({ ...localLibraryDiagnosticChecks, ...signedLibraryDiagnosticChecks }, expected);
  assert(Object.isFrozen(localLibraryDiagnosticChecks)); assert(Object.isFrozen(signedLibraryDiagnosticChecks));
  for (const [phase, ids] of Object.entries({ ...localLibraryDiagnosticChecks, ...signedLibraryDiagnosticChecks })) {
    assert(Object.isFrozen(ids)); assert.equal(ids.length, new Set(ids).size);
    for (const id of ids) { assert(id.startsWith(phase + '-')); assert.match(id, /^[a-z0-9]+(?:-[a-z0-9]+)+$/); }
  }
});

function controlPage({ failAt } = {}) {
  const events = [];
  let popup = false, selected = false;
  const fail = action => { events.push(action); if (failAt === action) throw Error('synthetic exact control failure'); };
  const trigger = {
    and() { return this; },
    filter() { return this; },
    async waitFor({ state }) { assert.equal(state, 'visible'); fail('trigger-wait'); },
    async click() { fail('trigger-click'); popup = true; },
  };
  const page = {
    getByRole(role, options) {
      if (role === 'button') {
        if (options.exact) return { and() { return this; }, async waitFor({ state }) { assert.equal(state, 'visible'); assert.equal(options.name, 'Reading'); assert(selected); fail('status-visible'); } };
        return trigger;
      }
      assert.equal(role, 'option'); assert.equal(options.name, 'Reading'); assert.equal(options.exact, true);
      return { async click() { assert(popup); fail('option-click'); selected = true; popup = false; } };
    },
    locator(selector) {
      if (selector === '[data-slot="select-content"][role="listbox"]:visible') return { async waitFor({ state }) { assert.equal(state, 'visible'); assert(popup); fail('popup-wait'); } };
      assert.equal(selector, 'div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible');
      return { filter({ hasText }) { if (hasText.test('Add to Library')) return trigger; assert(hasText.test('Reading')); return { async waitFor({ state }) { assert.equal(state, 'visible'); assert(selected); fail('status-visible'); } }; } };
    },
  };
  return { page, events };
}

test('actual shared helper records strict readiness, popup and selected status in order', async () => {
  const report = {}, page = controlPage();
  await selectTitleLibraryStatus(page.page, 'Reading', { check: createJourneyCheck(report), prefix: 'local-library-device-follow' });
  assert.deepEqual(page.events, ['trigger-wait', 'trigger-click', 'popup-wait', 'option-click', 'status-visible']);
  assert.deepEqual(report.journey_checks, titleFollowCheckSuffixes.map(suffix => ({ check_id: 'local-library-device-follow-' + suffix, status: 'passed' })));
});

test('actual helper preserves failure and performs no later control after each interruption', async () => {
  for (const [failAt, failedStage] of [['trigger-wait', 0], ['trigger-click', 1], ['popup-wait', 1], ['option-click', 2], ['status-visible', 2]]) {
    const report = {}, page = controlPage({ failAt });
    await assert.rejects(selectTitleLibraryStatus(page.page, 'Reading', { check: createJourneyCheck(report), prefix: 'signed-library-mapped-follow' }), /synthetic exact control failure/);
    assert.equal(page.events.at(-1), failAt);
    assert.deepEqual(report.journey_checks.map(row => row.status), [...Array(failedStage).fill('passed'), 'failed']);
    assert.equal(report.journey_checks.length, failedStage + 1);
  }
});

test('legacy uninstrumented helper keeps exactly its original two clicks', async () => {
  const page = controlPage(); await selectTitleLibraryStatus(page.page, 'Reading');
  assert.deepEqual(page.events, ['trigger-click', 'option-click']);
});

const checkOnlyPage = { getByRole() { return { and() { return this; } }; }, locator() { return { filter() { return this; } }; } };
const localStart = localSource.indexOf("\t} else if (phase === 'local-library') {") + "\t} else if (phase === 'local-library') {".length;
const localEnd = localSource.indexOf('\n\t} else if (phase === "pwa")', localStart);
assert(localStart > 0 && localEnd > localStart);
const signedStart = signedSource.indexOf('    await step("anonymous device controls persist mapped');
const signedEnd = signedSource.indexOf('\n    if (phase === "signed-source-controls")', signedStart);
assert(signedStart > 0 && signedEnd > signedStart);
const followSource = signedSource.match(/const follow = ([^\n]+);/)[1];

async function wiringProbe(phase, failAt) {
  const report = {}, steps = [], failure = Error('synthetic private checkpoint failure');
  const recorder = createJourneyCheck(report);
  const inventory = phase === 'local-library' ? expected[phase] : signedSuffixes.map(suffix => phase + '-' + suffix);
  const check = async (id, operation) => {
    assert.equal(typeof operation, 'function'); assert(inventory.includes(id));
    // These probe bodies deliberately stand in for the original private actions.
    // The real recorder and outer step code still govern failure propagation.
    return recorder(id, async () => {
      if (id === failAt) throw failure;
      if (id.endsWith('account-status-durable')) return { status: 200 };
      return { private: 'must never be retained' };
    });
  };
  const step = async (label, operation) => {
    if (label.startsWith('real reader progress') || label.startsWith('relocated native Library')) return; // Separate actual-browser coverage, outside this fixed-check wiring seam.
    const row = { label }; steps.push(row);
    try { await operation(); row.status = 'passed'; } catch (cause) { row.status = 'failed'; row.error = cause; }
  };
  const globals = { h: { check }, check, phase, step, page: checkOnlyPage, selectTitleLibraryStatus,
    insist(condition, message) { assert(condition, message); }, panel: () => ({}), identityPanel: () => ({}),
    inspect: (suffix, operation) => check(phase + '-' + suffix, operation),
    // Assignment-only private fixtures are explicit stand-ins, never evidence.
    deviceList: { id: 'private synthetic list' }, owner: 'private owner', protocol: 'private origin',
    installSigner() { assert.fail('The wiring stand-in must not execute a real signer action'); },
  };
  globals.follow = vm.runInNewContext('(' + followSource + ')', globals);
  await vm.runInNewContext('(async () => {' + (phase === 'local-library' ? localSource.slice(localStart, localEnd) : signedSource.slice(signedStart, signedEnd)) + '})()', globals);
  const failed = steps.filter(row => row.status === 'failed');
  assert.equal(failed.length, failAt ? 1 : 0, JSON.stringify(steps.map(row => [row.label, row.status])));
  if (failAt) assert.equal(failed[0].error, failure);
  const observed = report.journey_checks.map(row => row.check_id);
  assert.equal(observed.length, new Set(observed).size, 'No checkpoint/action may be replayed');
  const indexes = observed.map(id => inventory.indexOf(id));
  assert.deepEqual(indexes, [...indexes].sort((a, b) => a - b));
  if (!failAt) assert.deepEqual(observed, inventory);
  else {
    assert.equal(report.journey_checks.find(row => row.check_id === failAt)?.status, 'failed');
    const failedStep = phase === 'local-library'
      ? inventory.indexOf(failAt) < 8 ? [0, 8] : inventory.indexOf(failAt) < 14 ? [8, 14] : inventory.indexOf(failAt) < 21 ? [14, 21] : [21, 25]
      : inventory.indexOf(failAt) < 16 ? [0, 16] : [16, 23];
    assert.deepEqual(observed, inventory.filter((id, index) => index < failedStep[0] || index >= failedStep[1] || index <= inventory.indexOf(failAt)));
  }
  assert.doesNotMatch(JSON.stringify(report), /private synthetic list|private owner|private origin|must never be retained|synthetic private checkpoint failure/);
  assert(report.journey_checks.every(row => Object.keys(row).join(',') === 'check_id,status'));
}

test('actual local outer code retains monotone checkpoints and original failure after every split', async () => {
  await wiringProbe('local-library');
  for (const id of expected['local-library']) await wiringProbe('local-library', id);
});

test('actual signed common outer code records every fixed phase split and original failure', async () => {
  for (const phase of signedJourneyPhases) {
    await wiringProbe(phase);
    for (const id of signedSuffixes.map(suffix => phase + '-' + suffix)) await wiringProbe(phase, id);
  }
});

// These execute the actual ordinary list/import callbacks and shared native
// management helpers. Only the driver, downloads and persisted rows are explicit
// stand-ins; passing this seam is not browser, storage or product acceptance.
const managementStart = localSource.indexOf('\tconst panel = () =>');
const managementEnd = localSource.indexOf('\n\tconst follow =', managementStart);
const customStart = localSource.indexOf("\t\tawait step('custom list controls and reviewed scoped backup export/import");
const customEnd = localSource.indexOf("\n\t\tawait step('explicit account mode", customStart);
assert(managementStart > 0 && managementEnd > managementStart && customStart > 0 && customEnd > customStart);
const customSource = localSource.slice(managementStart, managementEnd) + '\nlet backup;\n' + localSource.slice(customStart, customEnd);
const customIDs = localSuffixes.slice(8, 14).map(suffix => 'local-library-' + suffix);

async function customProbe(fault) {
  const events = [], report = {}, original = Error('Original ordinary native driver failure: ' + fault);
  const origin = 'http://127.0.0.1:1', manga = 'SyntheticManga', marker = 'synthetic';
  const state = { records: [], scope: { origin, catalogNamespace: 'synthetic-scope' } };
  let route = '/library', active, merged = false;
  const wait = async (value, expected) => { assert.deepEqual(value, { state: expected }); };
  const main = {
    async waitFor(options) { await wait(options, 'visible'); assert.equal(route, '/settings/library'); },
    getByRole(role, options) {
      assert.equal(role, 'button'); assert.equal(options.exact, true);
      if (options.name === 'Manage lists') return { async click() { assert.equal(active, undefined); active = 'lists'; events.push('lists-open'); } };
      assert.equal(options.name, 'Export device backup');
      return { async click() { assert.equal(active, undefined, 'Export belongs to native Library main after closing lists'); events.push('backup-export'); } };
    },
    getByLabel(name, options) {
      assert.equal(name, 'Preview device backup'); assert.equal(options.exact, true);
      return { async setInputFiles(file) {
        assert.equal(active, undefined, 'A review must be dismissed before choosing another backup');
        if (fault === 'preview-driver') throw original;
        const input = JSON.parse(file.buffer.toString('utf8')); active = input.scope.origin === origin ? 'local-review' : 'foreign-review'; events.push(active);
      } };
    },
    getByText(text, options) {
      assert.equal(text, 'Backup merged locally.'); assert.equal(options.exact, true);
      return { async waitFor(value) { await wait(value, 'visible'); assert(merged); assert.equal(active, undefined); events.push('merge-outcome'); } };
    },
  };
  const list = {
    async waitFor(value) { await wait(value, 'visible'); assert.equal(active, 'lists'); }, async count() { return active === 'lists' ? 1 : 0; },
    getByLabel(name, options) { assert(['List name', 'Position'].includes(name)); assert.equal(options.exact, true); return { async fill(value) { assert.equal(active, 'lists'); events.push(['fill', name, value]); } }; },
    locator(selector) {
      const match = selector.match(/^label:text-is\("(List|Series)"\) > select$/); assert(match);
      return { async selectOption(value) { assert.equal(active, 'lists'); events.push(['select', match[1], value]); } };
    },
    getByRole(role, options) {
      assert.equal(role, 'button'); assert.equal(options.exact, true); assert(['Create list', 'Save member'].includes(options.name));
      return { async click() {
        assert.equal(active, 'lists'); events.push(options.name);
        if (options.name === 'Create list' && fault !== 'list-not-durable') state.records.push({ family: 'lists', id: 'synthetic-list', name: 'Local ' + marker });
        if (options.name === 'Save member' && fault !== 'member-not-durable') state.records.push({ family: 'members', list: 'synthetic-list', series: manga, position: 3 });
      } };
    },
  };
  const dialog = {
    locator(selector) { assert.equal(selector, 'section[data-library-tool="lists"]'); return list; },
    async waitFor(value) { await wait(value, value.state); assert(['visible', 'hidden'].includes(value.state)); assert.equal(!!active, value.state === 'visible'); events.push('review-' + value.state); },
    getByText(text, options) {
      assert.equal(text, 'This backup belongs to another catalogue.'); assert.equal(options.exact, false);
      return { async waitFor(value) { await wait(value, 'visible'); assert.equal(active, 'foreign-review'); events.push('foreign-warning'); } };
    },
    getByRole(role, options) {
      assert.equal(role, 'button'); assert.equal(options.exact, true);
      const name = options.name; assert(['Close', 'Merge reviewed import', 'Replace with reviewed import', 'Cancel import'].includes(name));
      return {
        async count() { assert.equal(active, 'foreign-review'); events.push(['foreign-control', name]); return fault === 'foreign-merge' && name === 'Merge reviewed import' || fault === 'foreign-replace' && name === 'Replace with reviewed import' ? 1 : 0; },
        async click() {
          if (name === 'Close') { assert.equal(active, 'lists'); events.push('lists-close'); }
          else if (name === 'Cancel import') { assert.equal(active, 'foreign-review'); events.push('review-cancel'); }
          else { assert.equal(name, 'Merge reviewed import'); assert.equal(active, 'local-review'); merged = true; events.push('review-merge'); }
          active = undefined;
        },
      };
    },
  };
  const page = {
    locator(selector) { assert.equal(selector, 'section[data-local-library-panel]'); return main; },
    getByRole(role, options) {
      if (role === 'dialog') { if (options) assert.deepEqual(options, { name: 'Review library import', exact: true }); return dialog; }
      assert.equal(role, 'link'); assert.deepEqual(options, { name: 'Manage library', exact: true });
      return { async getAttribute(name) { assert.equal(name, 'href'); return '/settings/library'; }, async click() { assert.equal(route, '/library'); route = '/settings/library'; events.push('manage-arrival'); } };
    },
    async waitForURL(predicate) { assert(predicate(new URL(route, origin))); },
  };
  let result, error;
  const recorder = createJourneyCheck(report);
  const globals = {
    page, origin, manga, marker, Buffer,
    async open(value) { assert.equal(value, '/library'); route = value; },
    insist(value, message) { assert(value, message); },
    async step(label, operation) { assert.match(label, /^custom list controls/); result = await operation(); },
    inspect: (suffix, operation) => recorder('local-library-' + suffix, operation),
    async device() { return state; }, async outboxCount() { return fault === 'automatic-publication' ? 1 : 0; },
    async until(read, valid, message) { const value = await read(); assert(valid(value), message); return value; },
    nativeSelectForLabel,
    async actAndWaitForPageEvent(target, event, operation) {
      assert.equal(target, page); assert.equal(event, 'download'); await operation();
      return { async *createReadStream() {
        const exported = { format: 'weebdex-device-library', scope: { ...state.scope, origin: fault === 'wrong-export-scope' ? 'https://foreign.example.invalid' : origin }, records: [...state.records, { family: 'series', id: manga }, { family: 'chapters', id: 'synthetic-chapter' }] };
        yield Buffer.from(JSON.stringify(exported));
      } };
    },
  };
  try { await new (Object.getPrototypeOf(async function() {}).constructor)(...Object.keys(globals), customSource)(...Object.values(globals)); } catch (cause) { error = cause; }
  return { error, original, events, report, result };
}

test('ordinary actual custom-list and import callbacks use separate native modal owners and reviewed outcomes', async () => {
  const value = await customProbe(); assert.equal(value.error, undefined);
  assert.deepEqual(value.report.journey_checks.map(row => row.check_id), customIDs);
  assert(value.report.journey_checks.every(row => row.status === 'passed'));
  const close = value.events.indexOf('lists-close'), exported = value.events.indexOf('backup-export');
  assert(close >= 0 && close < exported, 'Lists dialog must close before main-page export');
  assert(value.events.indexOf('foreign-warning') < value.events.indexOf('review-cancel'));
  assert(value.events.indexOf('review-cancel') < value.events.indexOf('local-review'));
  assert(value.events.indexOf('review-merge') < value.events.indexOf('merge-outcome'));
  assert.equal(value.result.foreign_scope_blocked, true); assert.equal(value.result.public_outbox_unchanged, true);
});

for (const [fault, check, message] of [
  ['list-not-durable', 'custom-list-durable', /List control did not persist/],
  ['member-not-durable', 'custom-member-durable', /Membership control did not persist/],
  ['wrong-export-scope', 'custom-backup-export', /UI export was not the scoped device backup/],
  ['foreign-merge', 'custom-foreign-backup-fenced', /Foreign native IDs could be merged or replaced silently/],
  ['foreign-replace', 'custom-foreign-backup-fenced', /Foreign native IDs could be merged or replaced silently/],
  ['automatic-publication', 'custom-backup-merged', /Backup\/list controls automatically published/],
]) test('ordinary actual native import/list callbacks preserve ' + fault + ' gate', async () => {
  const value = await customProbe(fault); assert.match(value.error?.message ?? '', message);
  assert.equal(value.report.journey_checks.at(-1).check_id, 'local-library-' + check); assert.equal(value.report.journey_checks.at(-1).status, 'failed');
  assert.equal(value.result, undefined);
});

test('ordinary actual native preview preserves original driver failure without a later merge', async () => {
  const value = await customProbe('preview-driver'); assert.equal(value.error, value.original);
  assert.equal(value.report.journey_checks.at(-1).check_id, 'local-library-custom-foreign-backup-fenced');
  assert(!value.events.includes('review-merge')); assert.equal(value.result, undefined);
});
