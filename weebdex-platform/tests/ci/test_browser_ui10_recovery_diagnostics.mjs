#!/usr/bin/env node
// Actual outer checkpoint wiring/cleanup with explicit inner-action stand-ins.
// The actions are not executed, so these cases establish no application or
// browser/authentication behavior. No network, services or storage are used.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { runUi10Journeys, ui10DiagnosticChecks } from '../../scripts/browser-ui10-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';

const identifiers = [
  'recovery-reset-realm', 'recovery-reset-hold-session', 'recovery-reset-submit',
  'recovery-reset-held-session', 'recovery-reset-newer-search', 'recovery-reset-late-response',
  'recovery-reset-no-stale-ui', 'recovery-reset-logout', 'recovery-reset-old-login-form',
  'recovery-reset-old-password', 'recovery-reset-old-login-settled', 'recovery-reset-old-session-denied',
  'recovery-reset-new-password', 'recovery-reset-link-replay',
];

function actualOldPasswordCheck() {
  const source = fs.readFileSync(new URL('../../scripts/browser-ui10-journeys.mjs', import.meta.url), 'utf8');
  const anchor = 'await check("recovery-reset-old-password", async () => {';
  const start = source.indexOf(anchor); assert.ok(start >= 0);
  const end = source.indexOf('\n        });', start); assert.ok(end > start);
  return source.slice(start + anchor.length, end);
}

test('actual old-password checkpoint still requires real server rejection status400', async () => {
  const operation = new (Object.getPrototypeOf(async function() {}).constructor)('page', 'insist', actualOldPasswordCheck());
  for (const status of [400, 200]) {
    const page = {
      async waitForResponse(predicate) { const response = { status: () => status, url: () => 'http://127.0.0.1:1/auth/self-service/login', request: () => ({ method: () => 'POST' }) }; assert.ok(predicate(response)); return response; },
      getByRole(role, options) { assert.equal(role, 'button'); assert.deepEqual(options, { name: 'Sign in', exact: true }); return { async click() {} }; },
    };
    const result = operation(page, (condition, message) => assert.ok(condition, message));
    if (status === 400) await result; else await assert.rejects(result, /old-password login was not rejected/);
  }
});

test('actual old-password checkpoint owns later response rejection when click fails', () => {
  const body = actualOldPasswordCheck();
  const child = spawnSync(process.execPath, ['--unhandled-rejections=strict', '--input-type=module', '-e', `
    const page = {
      waitForResponse() { return new Promise((_, reject) => setTimeout(() => reject(Error('late login response timeout')), 10)); },
      getByRole() { return { async click() { throw Error('synthetic click failure'); } }; },
    };
    const insist = condition => { if (!condition) throw Error('unexpected successful auth'); };
    try { await (async () => { ${body} })(); } catch {}
    await new Promise(resolve => setTimeout(resolve, 30));
  `], { encoding: 'utf8', timeout: 3000 });
  assert.equal(child.error, undefined); assert.equal(child.status, 0, child.stderr);
});

test('UI10 exported checkpoint contract is fixed, ordered and frozen', () => {
  assert.deepEqual(ui10DiagnosticChecks, { 'ui10-reader-transition': ['403','404','delayed','normal'].flatMap(control => ['open','page-one','image-bytes','settings','shortcut-focus','navigation','destination'].map(suffix => `reader-transition-${control}-${suffix}`)), 'ui10-recovery-lifetimes': identifiers });
  assert.ok(Object.isFrozen(ui10DiagnosticChecks));
  assert.ok(Object.isFrozen(ui10DiagnosticChecks['ui10-recovery-lifetimes']));
  assert.equal(new Set(identifiers).size, 14);
});

for (const failAt of [undefined, ...identifiers]) test('actual final-step wiring and cleanup: ' + (failAt ?? 'complete'), async () => {
  const report = {}, snapshots = [], steps = [], listeners = new Map();
  const privateError = Error('private synthetic response body must never enter finite receipt');
  const held = { closeCalls: 0, async close() { this.closeCalls++; } };
  const record = createJourneyCheck(report, () => snapshots.push(structuredClone(report)));
  await runUi10Journeys({ phase: 'ui10-recovery-lifetimes', origin: 'http://127.0.0.1:1', fixture: {}, context: {}, redactions: new Set(),
    page: { on(name, callback) { assert.equal(name, 'request'); assert.equal(listeners.size, 0); listeners.set(name, callback); }, off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); } },
    insist(condition, message) { assert.ok(condition, message); },
    async step(name, operation) {
      const row = { name }; steps.push(row);
      // Earlier five steps are deliberately outside this finite wiring seam.
      if (steps.length < 6) return;
      try { await operation(); row.status = 'passed'; } catch (error) { row.status = 'failed'; row.error = error; }
    },
    async check(id, operation) {
      assert.ok(identifiers.includes(id)); assert.equal(typeof operation, 'function');
      return record(id, async () => { if (id === failAt) throw privateError; return id === 'recovery-reset-hold-session' ? held : undefined; });
    },
  });
  assert.equal(steps.length, 6); assert.equal(steps[5].status, failAt ? 'failed' : 'passed');
  if (failAt) assert.equal(steps[5].error, privateError);
  const reached = failAt ? identifiers.slice(0, identifiers.indexOf(failAt) + 1) : identifiers;
  assert.deepEqual(report.journey_checks.map(row => row.check_id), reached);
  assert.ok(report.journey_checks.every(row => Object.keys(row).join(',') === 'check_id,status'));
  assert.deepEqual(report.journey_checks.map(row => row.status), reached.map(id => id === failAt ? 'failed' : 'passed'));
  assert.equal(listeners.size, 0);
  assert.equal(held.closeCalls, failAt && identifiers.indexOf(failAt) < 2 ? 0 : 1);
  assert.doesNotMatch(JSON.stringify(snapshots), /private synthetic|127\.0\.0\.1|Recovered!|password=|@/);
});
