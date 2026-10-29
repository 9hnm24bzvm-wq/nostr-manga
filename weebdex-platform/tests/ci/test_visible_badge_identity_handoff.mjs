// AUTHORED, NOT EXECUTED. Actual runtime/transport/ReaderBudget; controlled
// lifecycle and authority observations, not compiled mounted/browser acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import { badgeRuntimeFixture } from '../fixtures/bounded-reader/badge-runtime-fixture.mjs';
import { deferred, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
test('simultaneous title and badge identity use one existing owned request', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred();
  f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const mounting = f.mount(); await turns(); const budget = f.budget(), badge = f.identity(budget); await turns();
  assert.equal(f.calls.length, 1); gate.resolve(); const identity = await badge; await mounting;
  assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(identity.catalogNamespace, f.catalogIdentity.catalogNamespace); assert.equal(budget.requests, 1); assert(budget.bytes > 0); budget.close(); f.dispose();
});
test('cancelled badge waiter does not cancel title identity owner or bypass its slot', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred();
  f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const mounting = f.mount(); await turns(); const budget = f.budget(), badge = f.identity(budget), rejected = assert.rejects(badge);
  budget.cancel(); await rejected; assert.equal(f.calls.length, 1); gate.resolve(); await mounting; assert.equal(f.controller.snapshot.status, 'ready'); budget.close(); f.dispose();
});
test('waiter deadline and page/account fence reject late shared identity', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred(); let current = true;
  f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const mounting = f.mount(); await turns(); const budget = f.budget(() => { if (!current) throw Error('Changed account/page'); });
  const badge = f.identity(budget), rejected = assert.rejects(badge); current = false; gate.resolve(); await rejected; await mounting; budget.close(); f.dispose();
});
test('explicit title reverify does not silently reuse cached identity', async () => {
  const f = await badgeRuntimeFixture(false, true); await f.mount();
  f.responder = () => ({ catalog_namespace: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', service_key: null });
  await f.controller.verifyIdentity(); assert.equal(f.calls.length, 2); assert.equal(f.controller.snapshot.requiresReload, true); f.dispose();
});

test('settled discovery is not cached across later badge pages at the same origin', async () => {
  const f = await badgeRuntimeFixture(false, true); await f.mount();
  f.responder = () => ({ catalog_namespace: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', service_key: null });
  const budget = f.budget(), identity = await f.identity(budget); budget.close();
  assert.equal(f.calls.length, 2); assert.notEqual(identity.catalogNamespace, f.catalogIdentity.catalogNamespace);
  // The badge runtime must compare this fresh identity to its first admission.
  await f.controller.verifyIdentity(); assert.equal(f.controller.snapshot.requiresReload, true); f.dispose();
});

// Regression: a current badge may share discovery owned by a title action that
// is cancelled during account initialization. The badge's independent authority
// remains current; it must get one fresh discovery after the old owner settles.
test('current badge waiter recovers once after cancelled identity initiator settles', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred(); let reads = 0;
  f.responder = async () => { if (++reads === 1) await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const owner = f.budget(), waiter = f.budget();
  const first = f.identity(owner), firstRejected = assert.rejects(first);
  await turns(); const second = f.identity(waiter); await turns();
  owner.cancel(); await turns(); assert.equal(f.calls.length, 1, 'A held cancelled fetch must keep its transport ownership');
  gate.resolve(); await firstRejected;
  const result = await second;
  assert.equal(result.catalogNamespace, f.catalogIdentity.catalogNamespace);
  assert.equal(f.calls.length, 2); assert.equal(waiter.requests, 2); assert(waiter.bytes >= 65536);
  owner.close(); waiter.close(); f.dispose();
});

test('ordinary shared discovery failure does not automatically retry', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred();
  f.responder = async () => { await gate.promise; return new Response('', {status:503}); };
  const owner = f.budget(), waiter = f.budget(), first = f.identity(owner); await turns();
  const second = f.identity(waiter), failures = Promise.all([assert.rejects(first), assert.rejects(second)]);
  gate.resolve(); await failures; assert.equal(f.calls.length, 1);
  owner.close(); waiter.close(); f.dispose();
});

test('a changed badge waiter cannot recover from another consumer cancellation', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred(); let current = true;
  f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const owner = f.budget(), waiter = f.budget(() => { if (!current) throw Error('Badge account or page changed'); });
  const first = f.identity(owner); await turns(); const second = f.identity(waiter);
  const failures = Promise.all([assert.rejects(first), assert.rejects(second)]);
  current = false; owner.cancel(); gate.resolve(); await failures;
  assert.equal(f.calls.length, 1); owner.close(); waiter.close(); f.dispose();
});

test('unsettled cancelled identity owner is never bypassed by current waiter recovery', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred();
  f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const owner = f.budget(), waiter = f.budget(), first = f.identity(owner); await turns();
  const second = f.identity(waiter), failures = Promise.all([assert.rejects(first), assert.rejects(second, /cleanup is unconfirmed/)]);
  owner.cancel(); await turns(); f.expireCleanup(); await failures;
  assert.equal(f.calls.length, 1); assert.equal(waiter.requests, 1);
  gate.resolve(); await turns(); owner.close(); waiter.close(); f.dispose();
});

test('cancelled-owner recovery does not retry a second failed discovery', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred(); let reads = 0;
  f.responder = async () => { if (++reads === 1) { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; } return new Response('', {status:503}); };
  const owner = f.budget(), waiter = f.budget(), first = f.identity(owner); await turns();
  const second = f.identity(waiter), failures = Promise.all([assert.rejects(first), assert.rejects(second, /HTTP 503/)]);
  owner.cancel(); gate.resolve(); await failures;
  assert.equal(f.calls.length, 2); assert.equal(waiter.requests, 2); owner.close(); waiter.close(); f.dispose();
});

test('changed navigation identity key prevents cancelled-owner recovery', async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred();
  f.responder = async () => { await gate.promise; return { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null }; };
  const owner = f.budget(), waiter = f.budget(), first = f.identity(owner); await turns();
  const second = f.identity(waiter), failures = Promise.all([assert.rejects(first), assert.rejects(second)]);
  f.navigationStart(); owner.cancel(); gate.resolve(); await failures;
  assert.equal(f.calls.length, 1); owner.close(); waiter.close(); f.dispose();
});

for (const verdict of ['HTTP 503', 'Invalid catalogue identity response']) test(`owner abort after known ${verdict} verdict cannot relabel it as retryable cancellation`, async () => {
  const f = await badgeRuntimeFixture(false, true), gate = deferred(); let classified = 0;
  f.responder = async () => { await gate.promise; return verdict === 'HTTP 503' ? new Response('', {status:503}) : {}; };
  const owner = f.budget(), waiter = f.budget();
  // Controlled scheduling seam, not a substituted reader: interrupt immediately
  // after the actual client classifies the HTTP/identity error, before rejection
  // reaches shared waiters. This is the known-error/owner-abort race.
  f.onErrorConstructed(message => { if (String(message).includes(verdict)) { classified++; owner.cancel(); } });
  const first = f.identity(owner); await turns(); const second = f.identity(waiter);
  const failures = Promise.all([assert.rejects(first), assert.rejects(second, error => error.message.includes(verdict))]);
  gate.resolve(); await failures;
  assert.equal(classified, 1); assert.equal(owner.signal.aborted, true);
  assert.equal(f.calls.length, 1, 'A known failure must not acquire a second request even when its owner aborts');
  assert.equal(waiter.requests, 1); owner.close(); waiter.close(); f.dispose();
});
