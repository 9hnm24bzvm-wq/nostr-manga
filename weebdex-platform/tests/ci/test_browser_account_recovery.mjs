#!/usr/bin/env node
// Finite runner/account-recovery seams. Browser, API and storage are explicit
// stand-ins; these tests do not establish actual recovery/browser acceptance.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { accountRecoveryCheckIds, createAccountRecoveryChecks, submitRecoveryPassword } from '../../scripts/browser-account-recovery.mjs';
import { runJourneys } from '../../scripts/browser-journeys.mjs';

test('fixed receipts retain no inspected values and persist each transition', async () => {
  const report = {}, snapshots = [];
  const check = createAccountRecoveryChecks(report, () => snapshots.push(structuredClone(report)));
  assert.equal(report.account_recovery_checks.length, 18);
  for (const checkId of accountRecoveryCheckIds) {
    const privateValue = { password: 'private-value', mail: 'private@example.invalid' };
    assert.equal(await check(checkId, async () => privateValue), privateValue);
  }
  assert.equal(snapshots.length, 37);
  assert.ok(report.account_recovery_checks.every(row => row.status === 'passed'));
  assert.deepEqual(snapshots[1].account_recovery_checks[0], { check_id: 'device-library-seeded', status: 'running' });
  assert.doesNotMatch(JSON.stringify(snapshots), /private-value|private@example/);
  assert.ok(report.account_recovery_checks.every(row => Object.keys(row).join(',') === 'check_id,status'));
});

test('failed receipt keeps its failure and later checks pending', async () => {
  const report = {}, cause = Error('private failure body');
  const check = createAccountRecoveryChecks(report);
  await assert.rejects(check('device-library-seeded', async () => { throw cause; }), error => error === cause);
  await assert.rejects(check('logout-before-recovery', async () => assert.fail('must not advance')), /check order changed/);
  await assert.rejects(check('device-library-seeded', async () => assert.fail('must not replay')), /check order changed/);
  assert.equal(report.account_recovery_checks[0].status, 'failed');
  assert.ok(report.account_recovery_checks.slice(1).every(row => row.status === 'pending'));
  assert.doesNotMatch(JSON.stringify(report), /private failure body/);
});

test('missing password input creates no response waiter', async () => {
  let waits = 0, clicks = 0;
  const page = {
    locator() { return { fill: async () => { throw Error('missing input'); } }; },
    waitForResponse() { waits++; throw Error('waiter must not exist'); },
    getByRole() { return { click: async () => { clicks++; } }; },
  };
  await assert.rejects(submitRecoveryPassword(page, 'synthetic-password'), /missing input/);
  assert.equal(waits, 0); assert.equal(clicks, 0);
});

test('proved old waiter crash is handled after the repaired click failure', () => {
  const helper = new URL('../../scripts/browser-account-recovery.mjs', import.meta.url).href;
  const setup = `
    const page = {
      locator() { return { fill: async () => {} }; },
      waitForResponse() { return new Promise((_, reject) => setTimeout(() => reject(Error('late settings response timeout')), 10)); },
      getByRole() { return { click: async () => { throw Error('click failure'); } }; },
    };
  `;
  const original = spawnSync(process.execPath, ['--unhandled-rejections=strict', '--input-type=module', '-e', setup + `
    try {
      const responsePromise = page.waitForResponse(() => true);
      await page.locator('input[name="password"]').fill('synthetic-password');
      await page.getByRole('button').click();
      await responsePromise;
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 30));
  `], { encoding: 'utf8', timeout: 3000 });
  assert.equal(original.status, 1); assert.match(original.stderr, /late settings response timeout/);
  const repaired = spawnSync(process.execPath, ['--unhandled-rejections=strict', '--input-type=module', '-e', `import { submitRecoveryPassword } from ${JSON.stringify(helper)};` + setup + `
    try { await submitRecoveryPassword(page, 'synthetic-password'); } catch {}
    await new Promise(resolve => setTimeout(resolve, 30));
  `], { encoding: 'utf8', timeout: 3000 });
  assert.equal(repaired.error, undefined); assert.equal(repaired.status, 0, repaired.stderr);
});

function journeyStandIns({ failSeed = false } = {}) {
  const origin = 'http://127.0.0.1:1', report = { steps: [] }, inputs = {}, calls = [];
  const device = { records: [{ family: 'series', id: 'SeedManga1', status: 'reading' }] };
  let session = 'none', initialPassword, currentPassword, verified = false, responseWaits = 0;
  const response = (status, body) => ({ status: () => status, ok: () => status >= 200 && status < 300, json: async () => body, headers: () => ({ 'content-type': 'application/json' }) });
  const locator = selector => ({
    fill: async value => { inputs[selector] = value; }, count: async () => 0,
    filter({ hasText }) { assert(hasText.test('Add to Library')); return { click: async () => { if (failSeed) throw Error('synthetic title control unavailable'); calls.push('title-trigger'); } }; },
    first() { return this; }, waitFor: async () => {},
  });
  const role = (kind, options) => ({
    first() { return this; }, and() { return this; }, waitFor: async () => {}, count: async () => 0,
    click: async () => {
      calls.push(options.name);
      if (options.name === 'Create Account') { session = 'normal'; initialPassword = currentPassword = inputs['input[name="password"]']; }
      if (options.name === 'Logout') session = 'none';
      if (options.name === 'Reset Password') { currentPassword = inputs['input[name="password"]']; session = 'normal'; }
      if (options.name === 'Sign in') session = inputs['input[name="password"]'] === currentPassword ? 'normal' : 'none';
      if (options.name === 'Continue to Account Settings') page.current = origin + '/settings/account';
    },
  });
  const page = {
    current: origin + '/', locator, getByRole: role,
    getByText: () => ({ waitFor: async () => {} }),
    waitForTimeout: async () => {}, evaluate: async () => structuredClone(device),
    goto: async target => { page.current = target.includes('verification') ? origin + '/' : origin + '/auth/recovery'; if (target.includes('verification')) verified = true; else session = 'recovery'; },
    url: () => page.current,
    waitForURL: async predicate => assert.ok(predicate(new URL(page.current))),
    waitForResponse: async predicate => { responseWaits++; const value = { ...response(200), url: () => origin + '/auth/self-service/settings', request: () => ({ method: () => 'POST' }) }; assert.ok(predicate(value)); return value; },
  };
  const context = {
    cookies: async () => [{ name: 'synthetic_session' }],
    request: {
      fetch: async target => target.endsWith('/auth/check')
        ? session === 'recovery' ? response(200, { session_purpose: 'recovery', recovery: { can_reset_password: true }, roles: [], permissions: [] })
          : session === 'normal' ? response(200, { identity: { username: inputs['input[name="username"]'], is_verified: verified } }) : response(401)
        : response(403),
      post: async () => response(200),
      get: async target => target.includes('/mail/latest?') ? response(200, { fixture_only: true, link: origin + '/auth/self-service/' + (target.includes('kind=recovery') ? 'recovery' : 'verification') + '?token=private-fixture-token' }) : response(400),
    },
  };
  const h = {
    phase: 'account-recovery', page, context, origin, report, fixture: { mail_origin: 'http://127.0.0.1:2' }, redactions: new Set(),
    insist: (condition, message) => { if (!condition) throw Error(message); },
    open: async route => { page.current = origin + route; },
    step: async (name, action) => { const row = { name }; report.steps.push(row); try { await action(); row.status = 'passed'; } catch (cause) { row.status = 'failed'; row.error = cause.message; } },
  };
  return { h, report, calls, responseWaits: () => responseWaits, passwords: () => [initialPassword, currentPassword] };
}

test('actual journey failed device precondition remains failed without a reset waiter', async () => {
  const value = journeyStandIns({ failSeed: true }); await runJourneys(value.h);
  assert.deepEqual(value.report.steps.map(row => row.status), ['passed', 'failed', 'failed']);
  assert.match(value.report.steps[1].error, /title control unavailable/);
  assert.match(value.report.steps[2].error, /prerequisite did not pass/);
  assert.equal(value.responseWaits(), 0);
  assert.equal(value.report.account_recovery_checks[0].status, 'failed');
  assert.ok(value.report.account_recovery_checks.slice(1).every(row => row.status === 'pending'));
  assert.equal(value.calls.includes('Reset Password'), false);
});

test('actual journey keeps every recovery assertion and all eighteen checkpoints', async () => {
  const value = journeyStandIns(); await runJourneys(value.h);
  assert.deepEqual(value.report.steps.map(row => row.status), ['passed', 'passed', 'passed'], JSON.stringify(value.report.steps));
  assert.deepEqual(value.report.account_recovery_checks.map(row => row.check_id), [...accountRecoveryCheckIds]);
  assert.ok(value.report.account_recovery_checks.every(row => row.status === 'passed'));
  assert.equal(value.responseWaits(), 1);
  assert.notEqual(...value.passwords());
  assert.doesNotMatch(JSON.stringify(value.report.account_recovery_checks), /private-fixture-token|browser_|Fixture!|Recovered!/);
});
