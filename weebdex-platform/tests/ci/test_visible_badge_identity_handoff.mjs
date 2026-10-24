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
