// AUTHORED, NOT EXECUTED. Requires separately admitted finite Node controls.
import test from 'node:test';
import assert from 'node:assert/strict';
import { badgeFixture, turns } from '../fixtures/bounded-reader/badge-fixture.mjs';
for (const mode of ['account', 'device']) for (const count of [0, 1, 50, 51, 100, 101]) test(mode + ' exact coverage at ' + count, async () => {
  const f = await badgeFixture(mode, count); f.records.set('chapter:0', { family: 'chapters', key: 'chapter:0', id: 'chapter:0', mangaId: 'manga:one', completed: true });
  await f.controller.refresh(); const view = f.controller.snapshot;
  if (count > 100) { assert.equal(view.status, 'error'); assert.equal(view.values, undefined); assert.equal(f.calls.length + f.transactions.length, 0); return; }
  assert.equal(view.status, 'ready'); assert.deepEqual(Object.keys(view.values), f.control.ids);
  if (count) { assert.equal(view.values['chapter:0'], true); assert.equal(f.budgets.length, 1); assert.equal(f.budgets[0].rows, count); }
  if (count > 1) assert.equal(view.values['chapter:1'], false);
  assert.equal(mode === 'account' ? f.calls.length : f.transactions.length, Math.ceil(count / 50));
  assert.equal(mode === 'account' ? f.transactions.length : f.calls.length, 0);
  assert.equal(f.views.filter(value => value.values !== undefined).length, 1);
  if (mode === 'account') assert(f.calls.every(call => call.url.searchParams.getAll('id').length <= 50 && call.options.credentials === 'include'));
});
for (const fault of ['partial', 'fail']) test('second native batch ' + fault + ' never publishes first batch', async () => {
  const f = await badgeFixture(); f.control[fault] = 2; await f.controller.refresh();
  assert.equal(f.calls.length, 2); assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.controller.snapshot.values, undefined); assert(f.views.every(view => view.values === undefined));
});
for (const mode of ['account', 'device']) test(mode + ' second batch remains atomic while held', async () => {
  const f = await badgeFixture(mode); f.control.hold = 2; const pending = f.controller.refresh(); await turns();
  assert.equal(f.controller.snapshot.status, 'pending'); assert(f.views.every(view => view.values === undefined));
  f.settleHeld(); await pending; assert.equal(Object.keys(f.controller.snapshot.values).length, 51); assert.equal(f.budgets.length, 1);
});
for (const mode of ['account', 'device']) for (const change of ['account', 'namespace', 'page', 'revision', 'selection', 'ids']) test(mode + ' rejects late ' + change, async () => {
  const f = await badgeFixture(mode); f.control.hold = 2; const pending = f.controller.refresh(); await turns();
  if (change === 'account') f.source.admission.authority.actor = 'account:other';
  else if (change === 'namespace') f.identity.catalogNamespace = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  else if (change === 'page') f.control.page++;
  else if (change === 'revision') { f.source.revision++; f.source.accountRevision++; }
  else if (change === 'selection') f.source.admission.selectionEpoch++;
  else f.control.ids = [...f.control.ids].reverse();
  f.settleHeld(); await pending; assert.equal(f.controller.snapshot.values, undefined); assert(f.views.every(view => view.values === undefined));
});
for (const mode of ['account', 'device']) test(mode + ' one deadline covers both batches', async () => {
  const f = await badgeFixture(mode); f.control.hold = 2; const pending = f.controller.refresh(); await turns();
  f.expireDeadline(); await turns(); f.settleHeld(); await pending; assert.equal(f.controller.snapshot.values, undefined); assert.equal(f.budgets.length, 1);
});
for (const mode of ['account', 'device']) test(mode + ' abort/remount shares unsettled owner', async () => {
  const f = await badgeFixture(mode, 1); f.control.hold = 1; f.control.ignoredAbort = true;
  const pending = f.controller.refresh(); await turns(); f.controller.dispose(); await turns(); f.expireCleanup(); await pending;
  const remount = f.make(); await remount.refresh(); assert.equal(remount.snapshot.values, undefined); assert.equal(f.calls.length + f.transactions.length, 1);
  f.settleHeld(); await turns(); f.control.hold = 0; await remount.refresh(); assert.equal(remount.snapshot.status, 'ready');
});
for (const acknowledged of [true, false]) test('mutation ' + (acknowledged ? 'acknowledgement requeries' : 'uncertainty stays unknown'), async () => {
  const f = await badgeFixture('account', 1); await f.controller.refresh(); const mutation = f.controller.beginMutation();
  assert.equal(f.controller.snapshot.values, undefined); f.source.accountRevision++; f.controller.sync();
  if (acknowledged) mutation.acknowledged(); else mutation.uncertain(); await turns();
  assert.equal(f.controller.snapshot.status, acknowledged ? 'ready' : 'unknown'); assert.equal(f.calls.length, acknowledged ? 2 : 1);
  if (!acknowledged) { await f.controller.refresh(); assert.equal(f.calls.length, 1); assert.equal(f.controller.snapshot.values, undefined); }
});
test('duplicate accepted IDs do not become an abbreviated successful map', async () => {
  const f = await badgeFixture('account', 2); f.control.ids[1] = f.control.ids[0]; await f.controller.refresh(); assert.equal(f.calls.length, 0); assert.equal(f.controller.snapshot.values, undefined);
});
test('old source control requires the real projection, not a substitute reducer', async () => {
  const f = await badgeFixture('account', 1); await f.controller.refresh(); assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.controller.snapshot.values['chapter:0'], true);
});

test('an uncertain mutation cannot be replaced by a second caller', async () => {
  const f = await badgeFixture('account', 1); await f.controller.refresh(); const first = f.controller.beginMutation(); first.uncertain();
  assert.throws(() => f.controller.beginMutation(), /pending or uncertain/); assert.equal(f.calls.length, 1);
});

for (const change of ['dispose', 'account']) test('late acknowledgement after ' + change + ' cannot release the surviving mutation fence', async () => {
  const f = await badgeFixture('account', 1); await f.controller.refresh(); const token = f.controller.beginMutation();
  if (change === 'dispose') f.controller.dispose(); else { f.source.admission.authority.actor = 'account:two'; f.source.admission.authority.epoch++; f.controller.sync(); }
  const remount = f.make(); await remount.refresh(); token.acknowledged(); await turns(); await remount.refresh();
  assert.equal(f.calls.length, 1); assert.equal(remount.snapshot.values, undefined); assert.throws(() => remount.beginMutation(), /pending or uncertain/);
});
test('uncertain mutation survives remount, ordinary revisions, and retries', async () => {
  const f = await badgeFixture('account', 1); await f.controller.refresh(); const token = f.controller.beginMutation(); token.uncertain(); f.controller.dispose();
  const remount = f.make(); f.source.accountRevision++; remount.sync(); await remount.refresh();
  assert.equal(f.calls.length, 1); assert.equal(remount.snapshot.values, undefined); assert.throws(() => remount.beginMutation(), /pending or uncertain/);
});

for (const mode of ['account', 'device']) test(mode + ' changed context discovered during read automatically refreshes without accepting stale badges', async () => {
  const f = await badgeFixture(mode, 1); f.control.hold = 1;
  const pending = f.controller.refresh(); await turns();
  f.control.ids = ['chapter:fresh']; f.source.admission.authority.epoch++;
  // No sync notification: the in-flight budget itself discovers the changed key.
  f.settleHeld(); await pending; await turns();
  assert.equal(f.controller.snapshot.status, 'ready');
  assert.deepEqual(Object.keys(f.controller.snapshot.values), ['chapter:fresh']);
  assert(f.views.filter(view => view.values).every(view => !Object.hasOwn(view.values, 'chapter:0')));
  assert.equal(f.budgets.length, 2); f.controller.dispose();
});
test('same-context read failures remain explicit and do not automatically retry', async () => {
  const f = await badgeFixture('account', 1); f.control.fail = 1;
  await f.controller.refresh(); await turns();
  assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.calls.length, 1);
  assert.equal(f.controller.snapshot.values, undefined); f.controller.dispose();
});
