#!/usr/bin/env node
// Original real signing/storage/transport tests; the shared fixture also binds
// the actual compiled Settings dialog lifetime to these same production modules.
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture } from './library-management-fixture.mjs';
const clean = value => JSON.parse(JSON.stringify(value));
const get = store => { let value; store.subscribe(next => { value = next; })(); return value; };

for (const reason of ['Close', 'Back', 'navigation']) test('dismissed management prevents initial snapshot signing after deferred storage/worker: ' + reason, { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'worker' });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); f.close(); f.release(); await assert.rejects(sending, /Management closed/);
    assert.deepEqual(f.calls.kinds, []); assert.equal(f.calls.posts, 0); assert.deepEqual(clean(await f.protocol.jobs()), []);
  } finally { await f.dispose(); }
});

test('dismissal during external snapshot signature retains exact CAS-checked immutable job before stopping delivery', { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'snapshot' });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); f.close(); f.release(); await assert.rejects(sending, /Management closed/);
    const jobs = await f.protocol.jobs(); assert.equal(jobs.length, 1);
    const job = jobs[0]; assert(f.nostr.verifyEvent(job.event)); assert.equal(job.event.kind, 11999);
    assert.deepEqual(JSON.parse(job.event.content), { manifest: f.graph.root.hash, version: 1 });
    assert.deepEqual(clean(job.recipients.map(({ origin, serviceKey, selected }) => ({ origin, serviceKey, selected }))), clean(f.review.recipients));
    assert.equal(job.recipients[0].state, 'pending'); assert.equal(job.deviceRevision, 0);
    const batch = await f.protocol.jobBatch(job.id, 0); assert.equal(batch.objects[0].hash, f.graph.objects[0].hash);
    assert.deepEqual([...batch.objects[0].data], [...f.graph.objects[0].data]);
    assert.deepEqual(f.calls.kinds, [11999]); assert.equal(f.calls.posts, 0);
  } finally { await f.dispose(); }
});

for (const gateAt of ['proof', 'final-read']) test('dismissal blocks POST after exact job is durable at ' + gateAt, { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); f.close(); f.release(); await assert.rejects(sending, /Management closed/);
    const jobs = await f.protocol.jobs(); assert.equal(jobs.length, 1); assert(f.nostr.verifyEvent(jobs[0].event));
    assert.equal(jobs[0].recipients[0].state, 'retryable'); assert.equal(f.calls.posts, 0);
    assert.deepEqual(f.calls.kinds, [11999, 27235]);
    if (gateAt === 'final-read') assert.equal(f.calls.finalRead, 1);
  } finally { await f.dispose(); }
});

for (const changeDuringSign of ['device', 'recipients']) test('dismissal never bypasses original durable revision CAS: ' + changeDuringSign, { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'snapshot', changeDuringSign });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); await f.change(); f.close(); f.release(); await assert.rejects(sending, /changed.*signing|changed while signing/i);
    assert.deepEqual(clean(await f.protocol.jobs()), []); assert.equal(f.calls.posts, 0);
  } finally { await f.dispose(); }
});

test('late external public-key selection cannot reconnect a dismissed settings view', { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'connect' });
  try {
    const selecting = f.publication.connectSigner(f.assertCurrent);
    await Promise.race([f.entered, selecting.then(() => { throw Error('Selection ended before the deferred key response'); })]); f.close(); f.release(); await assert.rejects(selecting, /Management closed/);
    assert.equal(get(f.publication.signerIdentity).owner, null); assert.equal(f.calls.posts, 0);
  } finally { await f.dispose(); }
});

test('active management still signs and reaches the original transport boundary', { timeout: 5000 }, async () => {
  const f = await fixture();
  try {
    const id = await f.publication.publishReviewed(f.review, true, f.assertCurrent);
    assert.equal((await f.protocol.jobs())[0].id, id); assert.equal(f.calls.posts, 1);
    assert.deepEqual(f.calls.kinds, [11999, 27235]);
  } finally { await f.dispose(); }
});


test('closing recovery preview abandons a late HTTP signing prompt without starting a request', { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'recovery-proof' });
  try {
    const previewing = f.publication.previewRecovery(f.review.recipients[0], f.assertCurrent);
    await Promise.race([f.entered, previewing.then(() => { throw Error('Recovery ended before its signing prompt'); })]);
    f.close(); f.release(); await assert.rejects(previewing, /Management closed/);
    assert.deepEqual(f.calls.kinds, [27235]); assert.equal(f.calls.posts, 0); assert.deepEqual(clean(await f.protocol.jobs()), []);
  } finally { await f.dispose(); }
});

test('additive device schema reopens unchanged v1 saved families and exact original public jobs', { timeout: 5000 }, async () => {
  const f = await fixture(); let reopenedDevice, reopenedProtocol;
  try {
    await f.publication.publishReviewed(f.review, true, f.assertCurrent);
    const list = 'd'.repeat(32), date = '2026-10-14T00:00:00.000Z';
    await f.device.writeBatch([
      { family: 'series', key: 'TitleOne01', id: 'TitleOne01', status: 'reading', notification: true, followedAt: date, updated_at: 1 },
      { family: 'chapters', key: 'Chapter001', id: 'Chapter001', mangaId: 'TitleOne01', completed: false, page: 12, readAt: date, updated_at: 1 },
      { family: 'lists', key: list, id: list, name: 'Saved list', updated_at: 1 },
      { family: 'members', key: list + ':TitleOne01', list, series: 'TitleOne01', position: 2, updated_at: 1 },
    ]);
    await f.protocol.write('series', { id: 'e'.repeat(64), status: 'completed', updated_at: 1 });
    const before = await f.device.snapshot(), jobs = await f.protocol.jobs(), counts = await f.protocol.counts();
    const Device = f.device.constructor, Protocol = f.protocol.constructor, protocolName = f.protocol.db.name;
    f.device.db.close(); f.device.channel?.close(); f.protocol.close();
    reopenedDevice = await Device.open(before.scope, () => {}, () => {});
    reopenedProtocol = await Protocol.open(protocolName);
    const after = await reopenedDevice.snapshot();
    assert.equal(reopenedDevice.db.version, 2); assert.equal(after.version, 1); assert.equal(reopenedProtocol.db.version, 5);
    for (const name of ['publicAliases', 'idMigrationArchives', 'idMigrationIssues', 'idMigrationTombstones']) assert.equal(reopenedDevice.db.objectStoreNames.contains(name), true);
    assert.deepEqual(clean(after.records), clean(before.records)); assert.deepEqual(clean(after.scope), clean(before.scope)); assert.equal(after.revision, before.revision);
    assert.deepEqual(clean(await reopenedProtocol.jobs()), clean(jobs)); assert.deepEqual(clean(await reopenedProtocol.counts()), clean(counts));
    assert(f.nostr.verifyEvent((await reopenedProtocol.jobs())[0].event));
  } finally { reopenedDevice?.db.close(); reopenedDevice?.channel?.close(); reopenedProtocol?.close(); await f.dispose(); }
});
