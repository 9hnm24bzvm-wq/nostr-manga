// Pure deterministic seams. No browser, server, network, mail or database.
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { observeSeedTitleReadiness, readSeedTitleState } from '../../scripts/startup-readiness-probe.mjs';

const state = (visible = false) => ({ document_ready: true, body_meaningful: visible,
  fixture_heading_visible: visible, fixture_route: true });
const test = async (name, action) => { await action(); process.stdout.write('PASS ' + name + '\n'); };

await test('DOM sampler exports only four fixed booleans', () => {
  const result = vm.runInNewContext('(' + readSeedTitleState.toString() + ')()', {
    document: { readyState: 'complete', body: { innerText: 'private@example.invalid recovery-token' },
      querySelectorAll: () => [{ textContent: 'Fixture Adventure', getClientRects: () => [{}] }] },
    location: { pathname: '/title/SeedManga1', search: '?token=never-publish-this' },
  });
  assert.deepEqual(JSON.parse(JSON.stringify(result)), state(true));
  assert.equal(JSON.stringify(result).includes('private'), false);
});
await test('late ready observation retains the original failed checkpoint', async () => {
  let reads = 0, waitCalls = 0;
  const page = { evaluate: async () => state(reads++ > 0), waitForFunction: async (_, arg, options) => {
    waitCalls++; assert.equal(arg, undefined); assert.equal(options.polling, 100); assert.ok(options.timeout > 0 && options.timeout <= 6750);
  } };
  const result = await observeSeedTitleReadiness(page, false);
  assert.equal(result.original_body_meaningful, false);
  assert.equal(result.initial.body_meaningful, false); assert.equal(result.late.fixture_heading_visible, true);
  assert.equal(result.wait_outcome, 'ready'); assert.equal(waitCalls, 1);
});
await test('already meaningful checkpoint does not extend the wait', async () => {
  const result = await observeSeedTitleReadiness({ evaluate: async () => state(true),
    waitForFunction: () => { throw Error('must not wait'); } }, true);
  assert.equal(result.wait_outcome, 'not-needed');
});
await test('deadline and probe failures are finite categories without raw errors', async () => {
  for (const name of ['TimeoutError', 'Error']) {
    const result = await observeSeedTitleReadiness({ evaluate: async () => state(false),
      waitForFunction: async () => { const error = Error('secret reset token'); error.name = name; throw error; } }, false);
    assert.equal(result.wait_outcome, name === 'TimeoutError' ? 'deadline' : 'probe-error');
    assert.equal(JSON.stringify(result).includes('secret'), false);
  }
});
await test('failed sampler remains an unresolved finite probe', async () => {
  const result = await observeSeedTitleReadiness({ evaluate: async () => { throw Error('private URL'); } }, false);
  assert.equal(result.wait_outcome, 'probe-error'); assert.equal(result.original_body_meaningful, false);
  assert.equal(JSON.stringify(result).includes('private'), false);
});
await test('whole probe bounds a stalled initial sampler and observes late rejection', async () => {
  let reject;
  const pending = new Promise((_, fail) => { reject = fail; });
  const started = Date.now();
  const result = await observeSeedTitleReadiness({ evaluate: () => pending }, false, { timeoutMs: 10 });
  assert.equal(result.wait_outcome, 'deadline'); assert.ok(Date.now() - started < 1000);
  reject(Error('late private sampler error')); await new Promise(resolve => setTimeout(resolve, 20));
  assert.equal(JSON.stringify(result).includes('private'), false);
});
await test('late sampler resolution cannot mutate an already returned report', async () => {
  let resolve;
  const pending = new Promise(done => { resolve = done; });
  const result = await observeSeedTitleReadiness({ evaluate: () => pending }, false, { timeoutMs: 10 });
  const frozen = JSON.stringify(result);
  resolve(state(true)); await new Promise(done => setTimeout(done, 20));
  assert.equal(JSON.stringify(result), frozen); assert.equal(result.initial, undefined);
});
await test('deadline leaves no time for an additional renderer operation', async () => {
  let calls = 0;
  const result = await observeSeedTitleReadiness({ evaluate: async () => {
    calls++; await new Promise(done => setTimeout(done, 25)); return state(true);
  }, waitForFunction: async () => { calls++; } }, false, { timeoutMs: 5 });
  assert.equal(result.wait_outcome, 'deadline'); assert.equal(calls, 1);
  await new Promise(done => setTimeout(done, 30)); assert.equal(calls, 1);
});
await test('check persists running and terminal passed state and returns original value', async () => {
  const report = {}, saved = [], check = createJourneyCheck(report, () => saved.push(JSON.parse(JSON.stringify(report))));
  const sentinel = {};
  assert.equal(await check('fixed-check', async () => sentinel), sentinel);
  assert.deepEqual(saved.map(row => row.journey_checks[0].status), ['running', 'passed']);
});
await test('check rethrows identical failure without copying exception data', async () => {
  const report = {}, saved = [], check = createJourneyCheck(report, () => saved.push(JSON.parse(JSON.stringify(report))));
  const error = Error('password=never-record');
  await assert.rejects(check('fixed-check', async () => { throw error; }), actual => actual === error);
  assert.deepEqual(saved.map(row => row.journey_checks[0].status), ['running', 'failed']);
  assert.equal(JSON.stringify(report).includes('password'), false);
});
