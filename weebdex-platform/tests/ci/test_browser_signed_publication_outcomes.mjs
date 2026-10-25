#!/usr/bin/env node
// Execute actual signed outcome/prerequisite assertions with explicit private
// durable-record/service stand-ins. No browser, service, signer or replay.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const start = source.indexOf("const job = await inspect('two-snapshot-accepted',"), end = source.indexOf("\n        await inspect('two-snapshot-projected'", start);
assert(start > 0 && end > start);
const body = source.slice(start, end);
const protocol = 'http://127.0.0.1:4001', second = { origin: 'http://127.0.0.1:4002', service_key: 'second private key' }, fixture = { service_key: 'first private key' };
const makeJob = () => ({ id: 'private exact id', event: { id: 'private exact id', verified: true }, cancelled: false,
  recipients: [{ origin: protocol, serviceKey: fixture.service_key, state: 'accepted' }, { origin: second.origin, serviceKey: second.service_key, state: 'accepted' }] });
async function outcome(job) {
  const report = {}, check = createJourneyCheck(report), calls = [];
  const globals = { protocol, second, fixture, async jobs() { calls.push('read-durable-jobs'); return job ? [job] : []; },
    verify: event => event.verified === true, insist: (value, message) => assert(value, message), inspect: (suffix, operation) => check('signed-recipients-' + suffix, operation) };
  let value, error; try { value = await vm.runInNewContext('(async () => {' + body + ';return job;})()', globals); } catch (cause) { error = cause; }
  assert.deepEqual(calls, ['read-durable-jobs'], 'Diagnostics must not replay or manufacture any operation');
  assert.doesNotMatch(JSON.stringify(report), /private|http:|4001|4002/);
  return { value, error, report };
}

test('actual two-recipient gate requires a retained verified job and each original pinned accepted recipient', async () => {
  const good = await outcome(makeJob()); assert.equal(good.error, undefined); assert.equal(good.report.journey_checks.length, 4);
  for (const [change, expected] of [
    [() => undefined, 'job-retained'],
    [job => { job.cancelled = true; return job; }, 'job-retained'],
    [job => { job.event.verified = false; return job; }, 'job-retained'],
    [job => { job.recipients[0].state = 'retryable'; return job; }, 'primary-receipt'],
    [job => { job.recipients[0].serviceKey = 'changed'; return job; }, 'primary-receipt'],
    [job => { job.recipients[1].state = 'rejected'; return job; }, 'secondary-receipt'],
    [job => { job.recipients[1].serviceKey = 'changed'; return job; }, 'secondary-receipt'],
    [job => { job.recipients[1] = { ...job.recipients[0] }; return job; }, 'primary-receipt'],
  ]) {
    const value = await outcome(change(makeJob())); assert(value.error);
    const failed = value.report.journey_checks.filter(row => row.status === 'failed');
    assert.deepEqual(failed.map(row => row.check_id), ['signed-recipients-two-snapshot-accepted', 'signed-recipients-two-snapshot-' + expected]);
  }
});

test('actual admission prerequisite rejects an absent or altered retained job before changing fixture admission', async () => {
  const callback = source.match(/inspect\('admission-exact-prerequisite', (async \(\) => \{[^\n]+\})\);/)[1];
  for (const firstJob of [undefined, { id: 'different', event: { id: 'exact', verified: true } }, { id: 'exact', event: { id: 'exact', verified: false } }]) {
    await assert.rejects(vm.runInNewContext('(' + callback + ')()', { firstJob, verify: event => event.verified, insist: (value, message) => assert(value, message) }), /retained exact verified/);
  }
});

test('actual peer-policy step requires completed exact publication; success retains both policy and read assertions', async () => {
  const start = source.indexOf('      await step("actual mutual peer revocation'), end = source.indexOf('\n    } else if (phase === "signed-library")', start);
  assert(start > 0 && end > start);
  const fragment = source.slice(start, end);
  for (const completed of [false, true]) {
    const controls = [], reads = [], report = {}, check = createJourneyCheck(report);
    const globals = { lifecycleCoverage: { two_real_recipients: completed }, protocol, second, owner: 'private author', recipients: [{ origin: protocol }, second],
      async step(_label, operation) { return operation(); }, inspect: (suffix, operation) => check('signed-recipients-' + suffix, operation), insist: (value, message) => assert(value, message), verify: event => event?.verified === true,
      async privateControl(action, { allowed }) { assert.equal(action, 'peer-policy'); controls.push(allowed); return { result: { operation: { mutual_recorded_peer_approval: allowed, arbitrary_endpoint_received: false } } }; },
      async originalAt(origin, path) { reads.push({ origin, path }); return path.startsWith('/federation/') ? { status: 403, body: {} } : { status: 200, body: { event: { verified: true } } }; },
    };
    const run = vm.runInNewContext('(async () => {' + fragment + '})()', globals);
    if (completed) { await run; assert.deepEqual(controls, [false, true]); assert.equal(reads.length, 8); }
    else { await assert.rejects(run, /must complete before peer-policy reads/); assert.deepEqual(controls, []); assert.deepEqual(reads, []); }
    assert.doesNotMatch(JSON.stringify(report), /private|http:/);
  }
});
