#!/usr/bin/env node
// Production public-action/outbox and library publication paths, real signatures,
// synthetic keys and finite in-memory fixture boundaries. No real network/account.
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture } from './library-management-fixture.mjs';
const encoding = await import(new URL('../../.build/local-signing-base-2.0.0/node_modules/@scure/base/index.js', import.meta.url));

async function localFixture() {
  const f = await fixture({ moduleAdapters: { '@scure/base': encoding } });
  const local = await f.load('local-signing-key.ts');
  const draft = local.importLocalKeyDraft('0'.repeat(63) + '1', f.assertCurrent);
  await draft.activate(f.assertCurrent);
  return { f, draft };
}

test('reviewed public identity action uses the active local key and preserves exact verified intent', { timeout: 5000 }, async () => {
  const { f, draft } = await localFixture();
  try {
    const identity = await f.load('protocol-identity.ts');
    const selected = f.publication.selectedSigner();
    const authored = await f.nostr.signExact(selected.signer, selected.owner, { kind: 0, created_at: Math.floor(Date.now() / 1000), tags: [], content: '{}' });
    const review = identity.reviewWithdrawal(f.review.recipients[0], authored);
    await assert.rejects(identity.sendReviewed(review, true, f.assertCurrent), /Synthetic transport boundary/);
    const rows = await identity.publicActionRecords();
    assert.equal(rows.length, 1); assert.equal(rows[0].event.pubkey, draft.publicKey);
    assert.equal(rows[0].event.kind, 5); assert.equal(f.nostr.verifyEvent(rows[0].event), true);
    assert.equal(rows[0].event.content, review.content); assert.equal(f.calls.posts, 1);
    assert.deepEqual(f.calls.kinds, [], 'external signer is never invoked');
  } finally { f.publication.disconnectSigner(); await f.dispose(); }
});

test('reviewed library publication uses the same local identity and immutable signed job', { timeout: 5000 }, async () => {
  const { f, draft } = await localFixture();
  try {
    const selected = f.publication.selectedSigner();
    const review = { ...f.review, owner: selected.owner, generation: selected.generation };
    const id = await f.publication.publishReviewed(review, true, f.assertCurrent);
    const jobs = await f.publication.publicationJobs();
    assert.equal(jobs.length, 1); assert.equal(jobs[0].event.pubkey, draft.publicKey);
    assert.equal(jobs[0].id, id); assert.equal(jobs[0].recipients[0].state, 'retryable');
    assert.equal(f.nostr.verifyEvent(jobs[0].event), true); assert.equal(f.calls.posts, 1);
    assert.deepEqual(f.calls.kinds, [], 'external signer is never invoked');
  } finally { f.publication.disconnectSigner(); await f.dispose(); }
});

test('closing a reviewed action before deferred signing creates no local signature or durable action', { timeout: 5000 }, async () => {
  const { f } = await localFixture();
  try {
    const identity = await f.load('protocol-identity.ts');
    const review = identity.reviewProfile(f.review.recipients[0], { name: 'Synthetic reader', display_name: '', about: '' });
    const pending = identity.sendReviewed(review, true, f.assertCurrent); f.close();
    await assert.rejects(pending, /Management closed/);
    assert.equal((await identity.publicActionRecords()).length, 0); assert.equal(f.calls.posts, 0);
  } finally { f.publication.disconnectSigner(); await f.dispose(); }
});
