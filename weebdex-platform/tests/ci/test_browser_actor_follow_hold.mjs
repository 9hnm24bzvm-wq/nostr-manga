#!/usr/bin/env node
// Execute the actual shared hold closure with explicit in-memory request/route
// stand-ins. No browser, network, fixture or native-server success is claimed.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const source = fs.readFileSync(new URL('../../scripts/browser-ui11-16-journeys.mjs', import.meta.url), 'utf8');
const start = source.indexOf('  const hold = async '), end = source.indexOf('\n  const observe =', start);
assert.ok(start > 0 && end > start); assert.equal(source.indexOf('  const hold = async ', start + 1), -1);
async function setup(before, nativeStatus = 204, delivery = 'deliver') {
  let intercept, removed = 0, fetches = 0, fulfills = 0, aborts = 0;
  const context = { async route(_pattern, action) { intercept = action; }, async unroute(_pattern, action) { assert.equal(action, intercept); removed++; } };
  const until = async (read, predicate, message) => {
    for (let turn = 0; turn < 100; turn++) { const value = await read(); if (await predicate(value)) return value; await new Promise(resolve => setImmediate(resolve)); }
    assert.fail(message);
  };
  const hold = vm.runInNewContext(source.slice(start, end) + '\nhold;', { context, until, insist: (value, message) => assert.ok(value, message) });
  const request = {}, response = { status: () => nativeStatus };
  const gate = await hold('synthetic-private-target', value => value === request, before);
  const task = intercept({ request: () => request, async fetch(options) { assert.equal(options.maxRedirects, 0); fetches++; return response; }, async fulfill(value) { assert.equal(value.response, response); fulfills++; if (delivery === 'cancel') throw Error('private cancellation'); }, async abort() { aborts++; } });
  await gate.wait();
  return { gate, task, counts: () => ({ removed, fetches, fulfills, aborts }) };
}

for (const before of [false, true]) test('owned actual hold separates captured native status from delivered status: before=' + before, async () => {
  const { gate, task, counts } = await setup(before);
  assert.equal(gate.disposition, 'pending'); assert.equal(gate.nativeStatus, undefined);
  assert.equal(gate.heldNativeStatus, before ? undefined : 204);
  assert.deepEqual(counts(), { removed: 0, fetches: before ? 0 : 1, fulfills: 0, aborts: 0 });
  await gate.release(); await task;
  assert.equal(gate.heldNativeStatus, 204); assert.equal(gate.nativeStatus, 204); assert.equal(gate.disposition, 'delivered');
  await gate.close(); assert.deepEqual(counts(), { removed: 1, fetches: 1, fulfills: 1, aborts: 0 });
});

test('held native404 delivery loss preserves genuine status and owned cleanup', async () => {
  const { gate, task, counts } = await setup(false, 404);
  assert.equal(gate.heldNativeStatus, 404); assert.equal(gate.nativeStatus, undefined);
  await gate.fail(); await task; await gate.close();
  assert.equal(gate.nativeStatus, 404); assert.equal(gate.disposition, 'failed-delivery-after-real-response');
  assert.deepEqual(counts(), { removed: 1, fetches: 1, fulfills: 0, aborts: 1 });
});

test('cancelled delivery remains distinct from native success, and closes its route', async () => {
  const { gate, task, counts } = await setup(false, 204, 'cancel');
  assert.equal(gate.heldNativeStatus, 204); await gate.release(); await task; await gate.close();
  assert.equal(gate.nativeStatus, 204); assert.equal(gate.disposition, 'browser-cancelled');
  assert.deepEqual(counts(), { removed: 1, fetches: 1, fulfills: 1, aborts: 1 });
});
