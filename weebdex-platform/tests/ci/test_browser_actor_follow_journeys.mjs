#!/usr/bin/env node
// Finite harness contract/seams only. Inner browser actions are explicit
// stand-ins in wiring cases; none establishes selectors or runtime acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { actorFollowPhase, actorFollowDiagnosticChecks, actorFollowCaseChecks, runActorFollowJourneys, watchActorFollowRequests } from '../../scripts/browser-actor-follow-journeys.mjs';
import { ui11To16JourneyPhases, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';

const caseChecks = {
  'initial-readiness': ['prerequisite', 'initial-session', 'status-pending', 'status-settled'],
  'read-retry': ['prerequisite', 'delivery-failed', 'retry-pending', 'retry-settled'],
  'unblock-loading': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
  'unblock-error': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
  'pending-ack': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
  'pending-lost': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
  'pending-disposed': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
};
const aggregates = ['actor-follow-account', ...['group', 'user'].flatMap(kind => Object.keys(caseChecks).map(name => `actor-follow-${kind}-${name}`))];
const expected = ['actor-follow-account', ...['group', 'user'].flatMap(kind => Object.entries(caseChecks).flatMap(([name, suffixes]) => {
  const id = `actor-follow-${kind}-${name}`; return [id, ...suffixes.map(suffix => id + '-' + suffix)];
}))];

test('actor-follow enrollment is fixed, complete, ordered and frozen', () => {
  assert.equal(actorFollowPhase, 'ui14-actor-follow');
  assert.deepEqual(actorFollowDiagnosticChecks, { [actorFollowPhase]: expected });
  assert.ok(Object.isFrozen(actorFollowDiagnosticChecks)); assert.ok(Object.isFrozen(actorFollowDiagnosticChecks[actorFollowPhase]));
  assert.equal(new Set(expected).size, 71);
  assert.deepEqual(actorFollowCaseChecks, caseChecks); assert.ok(Object.isFrozen(actorFollowCaseChecks));
  for (const checks of Object.values(actorFollowCaseChecks)) assert.ok(Object.isFrozen(checks));
  assert.equal(aggregates.length, 15);
  assert.equal(ui11To16JourneyPhases.filter(phase => phase === actorFollowPhase).length, 1);
  assert.deepEqual(ui11To16DiagnosticChecks[actorFollowPhase], expected);
});

test('supplemental source plan binds exact new source without changing historical baseline scope', () => {
  const root = new URL('../../', import.meta.url);
  const plan = JSON.parse(fs.readFileSync(new URL('test-evidence/frontend/browser-actor-follow-plan.json', root), 'utf8'));
  const historical = JSON.parse(fs.readFileSync(new URL(plan.historical_browser_plan, root), 'utf8'));
  assert.equal(plan.base_commit, '56f9cb248334eff951be571a567697b73f710fab');
  assert.equal(plan.observed_correction_base, '94c00b8b0688b121499743ed213e465eec69bc37');
  assert.equal(plan.status, 'source-prepared-browser-unrun');
  assert.deepEqual(plan.supplemental_phases, [actorFollowPhase]); assert.deepEqual(plan.checks, expected);
  assert.equal(new Set(['startup', ...Object.values(historical.phases).flatMap(group => group.names ?? [])]).size, 38);
  assert.equal(plan.historical_phases_retained, 38); assert.equal(plan.total_required_unique_phases, 39);
  for (const [file, digest] of Object.entries(plan.source_sha256)) assert.equal(createHash('sha256').update(fs.readFileSync(new URL(file, root))).digest('hex'), digest, file);
  for (const required of ['scripts/browser-actor-follow-journeys.mjs', 'scripts/browser-actor-follow-controls.mjs', 'scripts/browser-ui11-16-journeys.mjs', 'scripts/ci/run-hosted-browser.py', 'scripts/ci/journey-diagnostics.py', 'scripts/ci/validate-ci-evidence.py']) assert.ok(Object.hasOwn(plan.source_sha256, required), required);
});

test('request counter observes exact browser target methods/settings only and removes its listener', () => {
  const listeners = new Map(), origin = 'http://127.0.0.1:31415', endpoint = '/api/user/follows/groups/SeedGroup1';
  const page = { on(name, callback) { assert.equal(name, 'request'); listeners.set(name, callback); }, off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); } };
  const watched = watchActorFollowRequests(page, origin, endpoint);
  const emit = (path, method, base = origin) => listeners.get('request')({ url: () => base + path, method: () => method });
  for (const method of ['GET', 'POST', 'DELETE']) emit(endpoint, method);
  emit('/api/settings', 'PUT'); emit('/api/settings', 'GET'); emit(endpoint, 'PUT'); emit(endpoint + '/other', 'POST'); emit('/api/auth/check', 'GET'); emit(endpoint, 'POST', 'http://localhost:31415'); emit(endpoint, 'POST', 'http://127.0.0.1:31416');
  assert.deepEqual(watched.counts, { GET: 1, POST: 1, DELETE: 1, PUT: 1 });
  watched.close(); assert.equal(listeners.size, 0);
});

for (const failAt of [undefined, ...aggregates]) test('actual outer phase wiring retains checkpoint failures: ' + (failAt ?? 'complete'), async () => {
  const report = {}, snapshots = [], steps = [], error = Error('private synthetic detail must not enter finite diagnostics');
  const record = createJourneyCheck(report, () => snapshots.push(structuredClone(report)));
  await runActorFollowJourneys({ phase: actorFollowPhase, ids: { group: 'synthetic-group', seeder: 'synthetic-user' },
    insist(value, message) { assert.ok(value, message); },
    async step(name, action) { const row = { name }; steps.push(row); try { await action(); row.status = 'passed'; } catch (cause) { row.status = 'failed'; row.error = cause; } },
    async check(id, action) { assert.equal(typeof action, 'function'); return record(id, async () => { if (id === failAt) throw error; }); },
  });
  assert.deepEqual(steps.map(row => row.name), aggregates);
  assert.deepEqual(report.journey_checks, aggregates.map(check_id => ({ check_id, status: check_id === failAt ? 'failed' : 'passed' })));
  assert.equal(steps.filter(row => row.status === 'failed').length, failAt ? 1 : 0);
  if (failAt) assert.equal(steps.find(row => row.name === failAt).error, error);
  assert.doesNotMatch(JSON.stringify(snapshots), /private synthetic|synthetic-group|synthetic-user|127\.0\.0\.1|password|@/);
});

test('actual account prerequisite rejects unverified, missing, wrong-status and self principals', async () => {
  for (const actual of [{ status: 401 }, { status: 200, body: { identity: { id: 'actor', is_verified: false } } }, { status: 200, body: { identity: { id: 'target', is_verified: true } } }]) {
    let failure;
    await runActorFollowJourneys({ phase: actorFollowPhase, ids: { group: 'group', seeder: 'target' }, api: async () => actual, signup: async () => {}, insist(value, message) { assert.ok(value, message); }, async step(id, action) { if (id === 'actor-follow-account') try { await action(); } catch (error) { failure = error; } }, check: async (_id, action) => action() });
    assert.match(String(failure?.message), /real verified non-self account/);
  }
});

// Execute selected actual assertion/cleanup bodies against a small browser/API
// stand-in. These negative controls reject invented/wrong native outcomes; they
// are not real browser or server responses and cannot establish E2E acceptance.
function negativeHarness(selected, fault) {
  const origin = 'http://127.0.0.1:31415', endpoint = '/api/user/follows/groups/group', listeners = new Map(), gates = [];
  let followed = false, mode = 'anonymous', error; const report = {}, record = createJourneyCheck(report);
  const emit = method => listeners.get('request')?.({ url: () => origin + endpoint, method: () => method });
  const button = name => ({
    async waitFor() {}, async count() { return mode === 'anonymous' ? 0 : name === 'Follow' ? 1 : 0; },
    async isEnabled() { return true; }, async isDisabled() { return fault !== 'actionable-pending'; },
  });
  const header = { filter({ hasText } = {}) { if (!hasText) return this; const name = ['Follow', 'Unfollow', 'Block', 'Unblock', 'Retry follow status'].find(value => hasText.test(value)); return { ...button(name), async click() { followed = true; emit('POST'); } }; }, locator() { return this; }, async waitFor() {}, async count() { return 1; } };
  const page = { on(name, callback) { listeners.set(name, callback); }, off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); }, locator() { return header; }, url: () => origin + '/group/group' };
  const h = { phase: actorFollowPhase, page, origin, ids: { group: 'group', seeder: 'user' },
    insist(value, message) { assert.ok(value, message); },
    async step(id, action) { if (id === selected) try { await action(); } catch (cause) { error = cause; } },
    check: record, requestPath: () => true,
    async stable(predicate, message) { assert.ok(await predicate(), message); },
    async until(read, predicate, message) { const value = await read(); assert.ok(await predicate(value), message); return value; },
    async open() { mode = selected.endsWith('initial-readiness') ? 'anonymous' : 'loading'; if (mode === 'loading') emit('GET'); }, async realm() {},
    async api(path, options = {}) {
      if (path === '/api/auth/check') return { status: 200, body: { identity: { id: 'actor', is_verified: true }, preferences: { version: 1, groups: [], uploaders: [] } } };
      if (path === '/api/settings') return { status: 200, body: { groups: (options.data.groups ?? []).map(id => ({ id })), uploaders: [] } };
      if (path === endpoint) {
        if (options.method) { followed = options.method === 'POST'; return { status: 204 }; }
        return { status: fault === 'uncommitted-write' && mode !== 'anonymous' ? 404 : followed ? 204 : 404 };
      }
      throw Error('Unexpected synthetic seam API path');
    },
    async hold(_pattern, _predicate) {
      const isAuth = selected.endsWith('initial-readiness') && gates.length === 0;
      const gate = { closed: 0, nativeStatus: isAuth ? (fault === 'bad-auth-status' ? 401 : 200) : fault === 'bad-read-status' ? 500 : followed ? 204 : 404, disposition: 'delivered',
        async wait() {}, async release() { if (isAuth) { mode = 'loading'; emit('GET'); } }, async fail() { gate.disposition = 'failed-delivery-after-real-response'; }, async close() { gate.closed++; } };
      gates.push(gate); return gate;
    },
  };
  return { h, gates, listeners, report, error: () => error };
}

for (const [suffix, fault, message] of [
  ['initial-readiness', 'bad-auth-status', /genuine native200/],
  ['initial-readiness', 'bad-read-status', /Existing native follow/],
  ['read-retry', 'bad-read-status', /delivery loss after a genuine native404/],
  ['pending-ack', 'uncommitted-write', /not actually committed/],
  ['pending-ack', 'actionable-pending', /left its control actionable/],
]) test(`actual ${suffix} rejects ${fault} and closes all owned gates/listeners`, async () => {
  const seam = negativeHarness('actor-follow-group-' + suffix, fault);
  await runActorFollowJourneys(seam.h);
  assert.match(String(seam.error()?.message), message);
  const rows = seam.report.journey_checks;
  assert.equal(rows[0].status, 'failed'); assert.equal(rows.at(-1).status, 'failed');
  assert.ok(rows.slice(1, -1).every(row => row.status === 'passed'));
  assert.ok(rows.every(row => expected.includes(row.check_id) && Object.keys(row).sort().join(',') === 'check_id,status'));
  const indices = rows.map(row => expected.indexOf(row.check_id)); assert.deepEqual(indices, [...indices].sort((a, b) => a - b));
  assert.ok(seam.gates.length > 0); assert.ok(seam.gates.every(gate => gate.closed === 1)); assert.equal(seam.listeners.size, 0);
});
