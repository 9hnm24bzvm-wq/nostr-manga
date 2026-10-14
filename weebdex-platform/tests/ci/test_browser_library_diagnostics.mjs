#!/usr/bin/env node
// Finite journey-diagnostic wiring seam. Inner checkpoint operations and browser
// APIs are explicit stand-ins; this proves recorder reachability/failure cleanup,
// never application behavior or genuine browser acceptance.
import assert from 'node:assert/strict';
import { runUi11To16Journeys, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';

const controls = ['timer-page1', 'timer-page3', 'lock-page1', 'lock-page3'];
const suffixes = ['setup', 'select-page', 'no-prior-progress', 'gate-captured', 'old-intent-cancelled', 'page-retained', 'durable-progress', 'reload-progress', 'stable-revision'];
const expected = {
  'ui11-device-progress': ['device-progress-signup-context', ...controls.flatMap(control => suffixes.map(suffix => `device-progress-${control}-${suffix}`))],
  'ui12-compact-feed': ['compact-feed-seed-pagination', 'compact-feed-spanish-preferences', 'compact-feed-fixture-total', 'compact-feed-open-updates', 'compact-feed-third-page', 'compact-feed-resume-visible', 'compact-feed-bounded-burst', 'compact-feed-compact-geometry', 'compact-feed-trimmed-cards', 'compact-feed-manual-terminal-page', 'compact-feed-unique-sequential-pages', 'compact-feed-scenario-membership', 'compact-feed-all-compact-chapters', 'compact-feed-terminal-idle', 'compact-feed-resize-retains-entries', 'compact-feed-back-restores-entries']
};
assert.deepEqual(ui11To16DiagnosticChecks, expected);
assert(Object.isFrozen(ui11To16DiagnosticChecks));
for (const ids of Object.values(ui11To16DiagnosticChecks)) {
  assert(Object.isFrozen(ids)); assert.equal(ids.length, new Set(ids).size);
  for (const id of ids) assert.match(id, /^[a-z0-9]+(?:-[a-z0-9]+)+$/);
}

let cases = 1;
async function probe(phase, failAt) {
  const visited = [], steps = [], listeners = new Map(); let gateInstallations = 0, gateClosures = 0;
  const failure = new Error('owned synthetic checkpoint failure');
  const fixture = { ids: { manga: 'synthetic-manga', chapter: 'synthetic-chapter' }, ui_regressions: { fixture_only: true, compact: { total: 505, language: 'es', manga_id: 'synthetic-compact' } } };
  const page = {
    // Gate installation/cleanup is called by the real outer journey, but its
    // evaluated function is deliberately not run in this no-browser seam.
    async evaluate(operation, kind) { if (kind) gateInstallations++; else gateClosures++; },
    getByRole() { return {}; },
    on(name, callback) { assert(!listeners.has(name)); listeners.set(name, callback); },
    off(name, callback) { assert.equal(listeners.get(name), callback); assert(listeners.delete(name)); }
  };
  await runUi11To16Journeys({ phase, fixture, page, context: {}, origin: 'http://127.0.0.1:1', report: {}, redactions: new Set(),
    insist(value, message) { assert(value, message); },
    async check(id, operation) {
      assert.equal(typeof operation, 'function'); assert(expected[phase].includes(id));
      assert(!visited.includes(id), 'An existing action must not be automatically replayed'); visited.push(id);
      if (id === failAt) throw failure;
      if (id.endsWith('-gate-captured')) return id.includes('page3') ? 3 : 1;
      if (id.endsWith('-durable-progress')) return { completed: id.includes('page3') };
      if (id === 'compact-feed-fixture-total') return 505;
      // This stand-in does not manufacture native success. It isolates only
      // the actual outer call-site sequence and exception propagation.
    },
    async step(label, operation) { try { await operation(); steps.push({ label, passed: true }); } catch (error) { steps.push({ label, passed: false, error }); } }
  });
  assert.equal(listeners.size, 0, 'All observed request listeners must be released');
  assert.equal(gateInstallations, gateClosures, 'Every installed owned gate must close after success or failure');
  assert.equal(steps.length, phase === 'ui11-device-progress' ? 5 : 1);
  const failed = steps.filter(step => !step.passed);
  assert.equal(failed.length, failAt ? 1 : 0);
  if (failAt) assert.equal(failed[0].error, failure, 'The original failure must reach the outer step unchanged');
  const expectedVisited = expected[phase].filter(id => {
    if (!failAt) return true;
    if (phase === 'ui12-compact-feed') return expected[phase].indexOf(id) <= expected[phase].indexOf(failAt);
    const failedControl = controls.find(control => failAt.startsWith(`device-progress-${control}-`));
    return !failedControl || !id.startsWith(`device-progress-${failedControl}-`) || expected[phase].indexOf(id) <= expected[phase].indexOf(failAt);
  });
  assert.deepEqual(visited, expectedVisited, 'A failed checkpoint leaves the rest of that original step unexecuted');
  cases++;
}
for (const [phase, ids] of Object.entries(expected)) {
  await probe(phase);
  for (const id of ids) await probe(phase, id);
}
console.log(JSON.stringify({ status: 'passed', cases, phase_check_counts: Object.fromEntries(Object.entries(expected).map(([phase, ids]) => [phase, ids.length])), boundary: 'Actual outer journey checkpoint wiring and exception cleanup with explicit inner-action/browser stand-ins; no browser, network, service, database or package install.' }));
