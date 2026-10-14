#!/usr/bin/env node
// Finite checkpoint wiring and native-control diagnostic seams only. The real
// checkpoint bodies in the journey wiring probes are explicit stand-ins. No
// browser, network, application, fixture, signer, service or database is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { titleFollowCheckSuffixes, selectTitleLibraryStatus } from '../../scripts/browser-select-controls.mjs';
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
    if (label.startsWith('real reader progress')) return; // Not instrumented by this change.
    const row = { label }; steps.push(row);
    try { await operation(); row.status = 'passed'; } catch (cause) { row.status = 'failed'; row.error = cause; }
  };
  const globals = { h: { check }, check, phase, step, page: checkOnlyPage, selectTitleLibraryStatus,
    insist(condition, message) { assert(condition, message); }, panel: () => ({}),
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
