// Historical signed UI regression only; current native coverage is test_native_contribution_dom.mjs.
import {historicalContributionSource} from './historical-contribution-ui-fixture.mjs';
// Production UI driver and fixed progress with delayed explicit control fakes.
// No browser, network, real key, application state, service or install is used.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import { connectPublicUploadKey, createKeyImportStep, keyImportSubsteps, keyImportCategories, keyImportFailureCategory } from '../../scripts/public-upload-key-import.mjs';
import { importFixture, serializedImportTrace } from './public-upload-key-import-fixture.mjs';

test('actual import driver waits through delayed hydration, routes, dialogs, input and recipient review with closed enrollment', async () => {
  const f = importFixture(); await f.run();
  assert.deepEqual(f.snapshot(), { selected: true, acknowledged: true, route: '/title/synthetic/upload' });
  assert.deepEqual(f.attempts, [{ attempt: 1, substep: 'upload-ready', completed_substeps: keyImportSubsteps.length, status: 'passed', caught_category: 'none' }]);
  assert.equal(f.calls.filter(row => row[0] === 'click' && row[1] === 'Use this key for this session').length, 1);
  assert.equal(f.calls.some(row => row[1] === 'Join this instance'), false);
  assert.equal(f.calls.filter(row => row[0] === 'back').length, 0);
  assert(f.calls.filter(row => row[0] === 'count' && row[1] === 'Use this key for this session').length > 1);
  assert.doesNotMatch(JSON.stringify(f.attempts), /private|synthetic|http:|settings|Nostr |nsec|pubkey/);
});
for (const name of ['Set up public identity', 'Set up your public identity', 'I already have a private key', 'Private key (nsec or hex)', 'Use this key for this session']) {
  test('missing control fails at fixed substep with bounded readiness category: ' + name, async () => {
    const f = importFixture({ never: name }); await assert.rejects(f.run());
    assert.equal(f.attempts[0].status, 'failed'); assert.equal(f.attempts[0].caught_category, 'readiness-timeout');
  });
  test('duplicate controls fail closed before their action: ' + name, async () => {
    const f = importFixture({ duplicate: name }); await assert.rejects(f.run());
    assert.equal(f.attempts[0].caught_category, 'duplicate-control');
    assert.equal(f.calls.some(row => row[0] === 'click' && row[1] === name), false);
  });
}
for (const [index, substep] of keyImportSubsteps.entries()) test('actual driver preserves exact failure boundary without raw values: ' + substep, async () => {
  const f = importFixture({ failAt: substep }); await assert.rejects(f.run());
  assert.deepEqual(f.attempts[0], { attempt: 1, substep, completed_substeps: index, status: 'failed', caught_category: 'unexpected' });
  assert.doesNotMatch(JSON.stringify(f.attempts), /sentinel|synthetic|http:|secretHex|nsec/);
});
test('error categories inspect only fixed codes/classes and never copy raw messages', () => {
  for (const [error, expected] of [[{ code: 'UPLOAD_UI_READINESS_TIMEOUT' }, 'readiness-timeout'], [{ code: 'UPLOAD_UI_DUPLICATE_CONTROL' }, 'duplicate-control'], [{ name: 'TimeoutError' }, 'browser-timeout'], [{ code: 'ERR_ASSERTION' }, 'assertion'], [{ name: 'AssertionError' }, 'assertion'], [{ message: 'http://private.invalid/nsec-secret', code: 'private' }, 'unexpected']]) assert.equal(keyImportFailureCategory(error), expected);
  assert.deepEqual([...keyImportCategories], ['none', 'readiness-timeout', 'duplicate-control', 'browser-timeout', 'assertion', 'unexpected']);
});
test('throwing error accessors stay generic and preserve the original failure', async () => {
  const failure = Object.defineProperty(Error('private sentinel'), 'code', { get() { throw Error('accessor sentinel'); } });
  assert.equal(keyImportFailureCategory(failure), 'unexpected');
  const f = importFixture({ failAt: 'import-form', failure });
  await assert.rejects(f.run(), error => error === failure);
  assert.equal(f.attempts[0].caught_category, 'unexpected');
});
test('progress permits exactly two imports and cannot continue a terminal or out-of-order attempt', async () => {
  const attempts = []; const run = createKeyImportStep(attempts, () => {});
  await assert.rejects(run('settings-route', async () => {})); assert.equal(attempts[0].completed_substeps, 0);
  await assert.rejects(run('open-settings', async () => { throw Error('sentinel'); }));
  await assert.rejects(run('open-settings', async () => {}));
  assert.throws(() => createKeyImportStep(attempts, () => {}));
  const complete = await serializedImportTrace(); assert.throws(() => createKeyImportStep(complete, () => {}));
});
test('every pre-action, inter-step and final saved snapshot uses the current fixed progress contract', async () => {
  const rows = [], snapshots = [];
  for (let i = 0; i < 2; i++) await importFixture({ attemptRows: rows, snapshotSink: snapshots }).run();
  assert.equal(snapshots.length, 56);
  for (const attemptRows of snapshots) for (const row of attemptRows) {
    if (row.status === 'passed') {
      assert.equal(row.completed_substeps, keyImportSubsteps.length); assert.equal(row.substep, keyImportSubsteps.at(-1));
    } else assert.equal(row.substep, keyImportSubsteps[row.completed_substeps]);
  }
  assert.deepEqual(snapshots[1][0], { attempt: 1, substep: 'settings-route', completed_substeps: 1, status: 'running', caught_category: 'none' });
  assert(snapshots.at(-1).every(row => row.status === 'passed'));
});
for (const failSaveAt of [[1], [2], [28], [2, 3]]) test('save failure restores the failed current step and preserves first persistence error: ' + failSaveAt, async () => {
  const f = importFixture({ failSaveAt });
  await assert.rejects(f.run(), error => error === f.saveErrors[0]);
  const index = Math.floor((failSaveAt[0] - 1) / 2);
  assert.deepEqual(f.attempts[0], { attempt: 1, substep: keyImportSubsteps[index], completed_substeps: index, status: 'failed', caught_category: 'unexpected' });
  assert(f.snapshots.length >= failSaveAt[0] + 1);
});
test('a failed diagnostic save cannot replace the first action failure', async () => {
  const f = importFixture({ failAt: 'import-form', failSaveAt: [12] });
  await assert.rejects(f.run(), error => error === f.failure);
  assert.equal(f.saveErrors.length, 1); assert.equal(f.attempts[0].substep, 'import-form');
  assert.equal(f.attempts[0].completed_substeps, 5); assert.equal(f.attempts[0].status, 'failed');
});
test('two serialized genuine-driver fake attempts retain closed fields only', async () => { const rows = await serializedImportTrace(); assert.deepEqual(rows.map(row => row.attempt), [1, 2]); assert(rows.every(row => row.status === 'passed')); });
test('fixture default and actual UI require local connection rather than self-enrollment; labels/ref and account-context remain production-bound', () => {
  const read = file => fs.readFileSync(new URL('../../' + file, import.meta.url), 'utf8');
  const provisioner = read('backend/examples/browser_fixture_provisioner.rs');
  const native = provisioner.match(/let native = json!\(([^\n]+)\);/)[1]; assert.doesNotMatch(native, /enrollment/);
  assert.match(provisioner, /repo\.set_account_admission\(id\(public\)\?, true\)/);
  const policy = fs.readFileSync(new URL('../../../manga-protocol-lab/server/src/enrollment.rs', import.meta.url), 'utf8'); assert.match(policy, /impl Default for Policy[\s\S]*?enabled: false/);
  const component = historicalContributionSource(new URL('../../platform-integrations/local-library/IdentitySetup.svelte', import.meta.url));
  assert.match(component, /\{#if enrollmentOpen\}[\s\S]*?onclick=\{join\}>[\s\S]*?'Join this instance'[\s\S]*?\{:else\}/);
  assert.match(component, /onclick=\{connectSession\}>Use this key for this session/);
  assert.match(component, /for="identity-private-key-\{id\}"[^>]*>Private key \(nsec or hex\)/);
  assert.match(component, /<Input bind:ref=\{importInput\} id="identity-private-key-\{id\}" type="password"/);
  const input = read('frontend/src/lib/components/ui/input/input.svelte'); assert.match(input, /ref = \$bindable\(null\)/); assert.match(input, /bind:this=\{ref\}/); assert.match(input, /\{\.\.\.restProps\}/);
  const runner = read('scripts/public-upload-recovery-browser.mjs'); assert.match(runner, /uniqueUploadControl\(locator, until\)/); assert.match(runner, /connectPublicUploadKey\(\{ page, unique, button, until, assertUploadRoute, secretHex, importStep: createKeyImportStep\(result\.key_import, save\) \}\)/);
  assert.match(runner, /reader\.get\('\/api\/v1\/nostr\/account-context'\)[\s\S]*?accountRead\.body\.nostr\.pubkey, owner[\s\S]*?nativeSessionAuthority, false/);
});
// This additional composition seam is pinned to the actual latest 301 component.
// The explicit override lets the older d6-based harness worktree inspect those
// bytes without changing any application source. The integrated tree defaults
// to the reviewed reader-UX component (including explicit local recovery download).
// Exact current bytes are requalified by identity setup security tests; future edits must requalify it.
function latestIdentityComposition() {
  const source = historicalContributionSource(process.env.WEEBDEX_IDENTITY_REVIEW_SOURCE ?? new URL('../../platform-integrations/local-library/IdentitySetup.svelte', import.meta.url));
  assert.equal(createHash('sha256').update(source).digest('hex'), 'dd54050b5b0ff4b18ca914f41c086c6c612e22509bed14e2c3897990f0aea81b');
  const backup = source.slice(source.indexOf("{:else if stage === 'backup'"), source.indexOf("{:else if stage === 'review'"));
  const backupSessionControl = /onclick=\{connectSession\}>Use this key for this session/.test(backup);
  assert.equal(backupSessionControl, true);
  assert.match(source, /review: enrollmentOpen \? 'Join this instance' : 'Instance approval needed'/);
  return { backupSessionControl };
}
test('latest 301 backup button cannot satisfy a failed recipient review', async () => {
  const f = importFixture({ ...latestIdentityComposition(), reviewFails: true });
  await assert.rejects(f.run());
  assert.equal(f.snapshot().selected, false);
  assert.deepEqual(f.attempts[0], { attempt: 1, substep: 'recipient-review', completed_substeps: 8, status: 'failed', caught_category: 'assertion' });
  assert.equal(f.calls.some(row => row[0] === 'click' && row[1] === 'Use this key for this session'), false);
});
test('latest 301 delayed successful review reaches its actual closed heading before session connection', async () => {
  const f = importFixture(latestIdentityComposition()); await f.run();
  assert.equal(f.snapshot().selected, true);
  const headings = f.calls.map((row, index) => [row, index]).filter(([row]) => row[0] === 'count' && row[1] === 'Instance approval needed');
  assert(headings.length > 1, 'The actual reviewed heading was initially absent');
  const connect = f.calls.findIndex(row => row[0] === 'click' && row[1] === 'Use this key for this session');
  assert(headings.every(([, index]) => index < connect));
  assert.equal(f.attempts[0].status, 'passed');
});
for (const duplicate of [false, true]) test('missing or duplicate actual review heading cannot select the key, duplicate=' + duplicate, async () => {
  const f = importFixture({ ...latestIdentityComposition(), ...(duplicate ? { duplicate: 'Instance approval needed' } : { never: 'Instance approval needed' }) });
  await assert.rejects(f.run()); assert.equal(f.snapshot().selected, false);
  assert.equal(f.attempts[0].substep, 'recipient-review');
  assert.equal(f.attempts[0].caught_category, duplicate ? 'assertion' : 'readiness-timeout');
});
if (process.env.WEEBDEX_KEY_IMPORT_DRIVER_BASELINE_SOURCE) {
  const baselineFile = process.env.WEEBDEX_KEY_IMPORT_DRIVER_BASELINE_SOURCE;
  const baselineSource = fs.readFileSync(baselineFile, 'utf8');
  assert.equal(createHash('sha256').update(baselineSource).digest('hex'), '2607cbd87ea2969cb030ea068e342045427f68483a3338be3039977c3faff84e');
  const baseline = await import(pathToFileURL(baselineFile).href);
  test('source-extracted d6 driver wrongly completes failed review with latest 301 backup button', async () => {
    const f = importFixture({ ...latestIdentityComposition(), reviewFails: true });
    await baseline.connectPublicUploadKey(f.args);
    assert.equal(f.snapshot().selected, true);
    assert.equal(f.attempts[0].status, 'passed');
    assert.equal(f.calls.some(row => row[0] === 'count' && row[1] === 'Instance approval needed'), false);
  });
}
if (process.env.WEEBDEX_PUBLIC_UPLOAD_BASELINE_SOURCE) {
  const baseline = fs.readFileSync(process.env.WEEBDEX_PUBLIC_UPLOAD_BASELINE_SOURCE, 'utf8');
  const start = baseline.indexOf('  async function connectKey() {'), end = baseline.indexOf('\n  async function english()', start); assert.ok(start > 0 && end > start);
  const body = baseline.slice(start + '  async function connectKey() {'.length, end).trim(); assert.ok(body.endsWith('}'));
  const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
  for (const delayed of [false, true]) test('frozen 5c9 source negative control rejects actual closed/delayed fixture, delayed=' + delayed, async () => {
    const f = importFixture({ delayed });
    const unique = async locator => { assert.equal(await locator.count(), 1); return locator; };
    const button = async (scope, name) => { const value = await unique(scope.getByRole('button', { name, exact: true })); await f.args.until(() => value.isEnabled(), Boolean); return value; };
    const args = { ...f.args, unique, button, assert, URL }; delete args.importStep;
    await assert.rejects(new AsyncFunction(...Object.keys(args), body.slice(0, -1))(...Object.values(args)));
    assert.deepEqual(f.calls.at(-1), ['count', delayed ? 'Open Settings' : 'Join this instance']);
    assert.equal(f.snapshot().selected, false);
    assert.equal(f.calls.some(row => row[0] === 'click' && row[1] === 'Use this key for this session'), false);
  });
}
