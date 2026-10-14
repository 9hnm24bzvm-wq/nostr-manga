#!/usr/bin/env node
// Finite harness contract/seams only. Inner browser actions are explicit
// stand-ins in wiring cases; none establishes selectors or runtime acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { actorFollowPhase, actorFollowDiagnosticChecks, actorFollowCaseChecks, runActorFollowJourneys, watchActorFollowRequests } from '../../scripts/browser-actor-follow-journeys.mjs';
import { ui11To16JourneyPhases, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';

const caseChecks = {
  'initial-readiness': ['prerequisite', 'initial-session', 'status-pending', 'status-pending-follow-visible', 'status-pending-follow-disabled', 'status-pending-block-hidden', 'status-pending-no-writes', 'status-settled'],
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
  assert.equal(new Set(expected).size, 79);
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
function negativeHarness(selected, fault, actorHeader = 'actor') {
  const origin = 'http://127.0.0.1:31415', endpoint = '/api/user/follows/groups/group', listeners = new Map(), gates = [];
  let followed = false, mode = 'anonymous', error; const report = {}, record = createJourneyCheck(report);
  const emit = method => listeners.get('request')?.({ url: () => origin + endpoint, method: () => method });
  const button = name => ({
    async waitFor() { if (mode !== 'anonymous' && name === 'Follow' && fault === 'missing-initial-follow') throw Error('Strict Follow is absent'); },
    async count() { return mode === 'anonymous' ? 0 : name === 'Follow' ? fault === 'missing-initial-follow' ? 0 : 1 : name === 'Block' && fault === 'exposed-initial-block' ? 1 : 0; },
    async evaluateAll(action) { return action(await this.count() ? [{ tagName: 'BUTTON', disabled: await this.isDisabled() }] : []); },
    async isEnabled() { return true; }, async isDisabled() { return !['actionable-pending', 'actionable-initial-follow'].includes(fault); },
  });
  const header = { filter({ hasText } = {}) { if (!hasText) return this; const name = ['Follow', 'Unfollow', 'Block', 'Unblock', 'Retry follow status'].find(value => hasText.test(value)); return { ...button(name), async click() { followed = true; emit('POST'); } }; }, locator() { return this; }, async waitFor() {}, async count() { return 1; } };
  const page = { on(name, callback) { listeners.set(name, callback); }, off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); }, locator() { return header; }, url: () => origin + '/group/group' };
  const h = { phase: actorFollowPhase, page, origin, report, ids: { group: 'group', seeder: 'user' },
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
    async hold(_pattern, predicate) {
      const isAuth = selected.endsWith('initial-readiness') && gates.length === 0;
      const gate = { closed: 0, heldNativeStatus: isAuth ? 200 : followed ? 204 : 404, nativeStatus: isAuth ? (fault === 'bad-auth-status' ? 401 : 200) : fault === 'bad-read-status' ? 500 : followed ? 204 : 404, disposition: 'pending',
        async wait() { if (!isAuth) { predicate({ headers: () => actorHeader ? ({ 'x-weebdex-expected-account': actorHeader }) : ({}) }); predicate({ headers: () => ({ 'x-weebdex-expected-account': 'later-other-actor' }) }); if (fault === 'unexpected-initial-write') emit('POST'); } }, async release() { gate.disposition = 'delivered'; if (isAuth) { mode = 'loading'; emit('GET'); } }, async fail() { gate.disposition = 'failed-delivery-after-real-response'; }, async close() { gate.closed++; } };
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

for (const [fault, suffix, message] of [
  ['missing-initial-follow', 'follow-visible', /Strict Follow is absent/],
  ['actionable-initial-follow', 'follow-disabled', /Follow became actionable/],
  ['exposed-initial-block', 'block-hidden', /exposed a new Block/],
  ['unexpected-initial-write', 'no-writes', /wrote follows or account preferences/],
]) test('initial pending failure remains exact and records the owned pre-cleanup view: ' + suffix, async () => {
  const seam = negativeHarness('actor-follow-group-initial-readiness', fault);
  await runActorFollowJourneys(seam.h);
  assert.match(String(seam.error()?.message), message);
  const failed = seam.report.journey_checks.filter(row => row.status === 'failed').map(row => row.check_id);
  assert.deepEqual(failed, ['actor-follow-group-initial-readiness', 'actor-follow-group-initial-readiness-status-pending', 'actor-follow-group-initial-readiness-status-pending-' + suffix]);
  assert.ok(!seam.report.journey_checks.some(row => row.check_id.endsWith('status-settled')));
  const [row] = seam.report.actor_follow_initial_observations;
  assert.equal(seam.report.actor_follow_initial_observations.length, 1);
  assert.equal(row.kind, 'group'); assert.equal(row.route, 'current'); assert.equal(row.held_status, 'native204');
  assert.equal(row.delivery, 'pending'); assert.equal(row.request_actor, 'prerequisite-actor');
  assert.equal(row.follow, fault === 'missing-initial-follow' ? 'absent' : fault === 'actionable-initial-follow' ? 'enabled' : 'disabled');
  assert.equal(row.block, fault === 'exposed-initial-block' ? 'disabled' : 'absent');
  assert.equal(row.post, fault === 'unexpected-initial-write' ? 'one' : 'zero');
  assert.ok(seam.gates.every(gate => gate.closed === 1)); assert.equal(seam.listeners.size, 0);
  assert.doesNotMatch(JSON.stringify(seam.report), /31415|group\/group|Strict Follow|actorId/);
});

test('all31 original assertions and ordered3 polls/5 stability conditions retain exact AST fingerprints', () => {
  assert.ok(process.env.WEEBDEX_FRONTEND_ROOT, 'Use the verified exact-lock dependency root');
  const ts = createRequire(path.join(process.env.WEEBDEX_FRONTEND_ROOT, 'package.json'))('typescript');
  const source = fs.readFileSync(new URL('../../scripts/browser-actor-follow-journeys.mjs', import.meta.url), 'utf8');
  const ast = ts.createSourceFile('actor-follow.mjs', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const calls = { insist: [], until: [], stable: [] }; let holds = 0;
  function walk(node) {
    if (ts.isCallExpression(node)) {
      const name = node.expression.getText(ast);
      if (Object.hasOwn(calls, name)) calls[name].push(node.arguments.map(arg => arg.getText(ast).replace(/\s+/g, ' ')).join(','));
      if (name === 'hold') holds++;
    }
    ts.forEachChild(node, walk);
  }
  walk(ast);
  for (const [name, count, digest] of [
    ['insist', 31, 'd1390f4c77329600af9f2f898723580f006a133b9211a87b8f2153ea39b9b88b'],
    ['until', 3, 'f63a82adb762a930cdde1667aa7af9b93abac7490e187317d1556e5300d10832'],
    ['stable', 5, 'bf70c11f1ee34d4c2e96d95824066bedcfc15fcd1db74ae3ccc3a29dad646a3d'],
  ]) { assert.equal(calls[name].length, count); assert.equal(createHash('sha256').update(JSON.stringify(calls[name])).digest('hex'), digest, name); }
  assert.equal(holds, 7);
});

for (const [header, expectedActor] of [['actor', 'prerequisite-actor'], ['private-other-actor', 'other-actor'], ['', 'anonymous']]) test('snapshot classifies only the first held request actor without publishing headers: ' + expectedActor, async () => {
  const seam = negativeHarness('actor-follow-group-initial-readiness', 'missing-initial-follow', header);
  await runActorFollowJourneys(seam.h);
  assert.match(String(seam.error()?.message), /Strict Follow is absent/);
  assert.equal(seam.report.actor_follow_initial_observations[0].request_actor, expectedActor);
  assert.doesNotMatch(JSON.stringify(seam.report), /private-other-actor|later-other-actor|x-weebdex/);
  assert.ok(seam.gates.every(gate => gate.closed === 1)); assert.equal(seam.listeners.size, 0);
});
