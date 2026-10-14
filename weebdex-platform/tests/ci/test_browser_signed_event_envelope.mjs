#!/usr/bin/env node
// Synthetic public fixture only. The committed Rust output was produced by
// compiling the exact original Event declaration and its real to_value operation.
// Signature verification remains independent of stable envelope equality.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { signedEventEnvelope, signedEventDigest } from '../../scripts/browser-signed-event-envelope.mjs';
const read = relative => fs.readFileSync(new URL(relative, import.meta.url), 'utf8');
const nodeEvent = JSON.parse(read('./fixtures/signed-envelope/node-event.json'));
const rustEvent = JSON.parse(read('./fixtures/signed-envelope/rust-event.json'));
const hash = value => createHash('sha256').update(value).digest('hex');
const frontend = process.env.WEEBDEX_FRONTEND_ROOT;
if (!frontend) throw Error('The existing verified exact-lock frontend dependency stage is required');
const stage = path.resolve(frontend, '../local-library-noble-2.0.1');
const manifestBytes = read('../../test-evidence/frontend/noble-restore/package-provenance.json');
assert.equal(hash(manifestBytes), 'f7cd7c2d6d7b14b9eaec4d38b4405267cc9ba291f0808dc3687822e28a3d24f3');
for (const item of JSON.parse(manifestBytes).packages) for (const file of item.files) {
  const source = path.resolve(stage, 'node_modules', item.name, file.path), bytes = fs.readFileSync(source);
  assert.equal(fs.realpathSync(source), source); assert.equal(bytes.length, file.bytes); assert.equal(hash(bytes), file.sha256);
}
const { schnorr } = await import(pathToFileURL(path.join(stage, 'node_modules/@noble/curves/secp256k1.js')));
const verify = event => {
  const id = hash(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]));
  return event.id === id && schnorr.verify(Buffer.from(event.sig, 'hex'), Buffer.from(id, 'hex'), Buffer.from(event.pubkey, 'hex'));
};

test('actual Rust Event declaration and read_event serialization are bound to the fixture', () => {
  const auth = read('../../../manga-protocol-lab/server/src/auth.rs');
  const start = auth.indexOf('#[derive(Clone, Debug, Serialize, Deserialize)]\npub struct Event {');
  const declaration = auth.slice(start, auth.indexOf('\nimpl Event {', start));
  assert(start > 0); assert(read('./fixtures/signed-envelope/rust-event-roundtrip.rs').includes(declaration));
  assert.match(read('../../../manga-protocol-lab/server/src/node.rs'), /Json\(serde_json::to_value\(&\*event\)\.map_err\(\|_\| unavailable\(\)\)\?\)/);
  assert.equal(hash(read('./fixtures/signed-envelope/node-event.json')), '1c2ee882b8f2635b07510cc0c1264d6e28f894912bee646046418e40fa08e0b7');
  assert.equal(hash(read('./fixtures/signed-envelope/rust-event.json')), 'f4ef6f206b844fa3271a5d5173f94d94a8b57f878fb434911451389b2590c56d');
});

test('real Rust to_value output breaks old object hash while preserving every signed field', () => {
  assert.deepEqual(nodeEvent, rustEvent);
  assert.notEqual(hash(JSON.stringify(nodeEvent)), hash(JSON.stringify(rustEvent)));
  assert.equal(signedEventDigest(nodeEvent), signedEventDigest(rustEvent));
  assert.deepEqual(signedEventEnvelope(nodeEvent), [nodeEvent.id, nodeEvent.pubkey, nodeEvent.created_at, nodeEvent.kind, nodeEvent.tags, nodeEvent.content, nodeEvent.sig]);
  assert(verify(nodeEvent)); assert(verify(rustEvent), 'Genuine Schnorr verification is independent of envelope equality');
});

test('every signed field and ordered tag/content byte remains significant', () => {
  const edits = { id: 'a'.repeat(64), pubkey: 'b'.repeat(64), created_at: nodeEvent.created_at + 1,
    kind: nodeEvent.kind + 1, tags: [...nodeEvent.tags].reverse(), content: nodeEvent.content + ' ', sig: '0'.repeat(128) };
  for (const [field, value] of Object.entries(edits)) assert.notEqual(signedEventDigest({ ...nodeEvent, [field]: value }), signedEventDigest(nodeEvent), field);
  const entryOrder = structuredClone(nodeEvent); entryOrder.tags[1] = ['ordered', 'two', 'one'];
  assert.notEqual(signedEventDigest(entryOrder), signedEventDigest(nodeEvent));
  assert.equal(verify({ ...nodeEvent, sig: '0'.repeat(128) }), false, 'A well-shaped equality projection is not signature validation');
});

test('missing, unknown and malformed fields cannot be silently projected away', () => {
  for (const key of Object.keys(nodeEvent)) { const event = { ...nodeEvent }; delete event[key]; assert.throws(() => signedEventDigest(event), /closed seven-field/); }
  for (const extra of [{ extra: true }, { toJSON() { return nodeEvent; } }, { [Symbol('unknown')]: true }]) assert.throws(() => signedEventDigest({ ...nodeEvent, ...extra }), /closed seven-field/);
  for (const bad of [{ kind: -1 }, { kind: 65536 }, { created_at: NaN }, { content: null }, { tags: [['d', 1]] }, { sig: 'invalid' }, { id: { toString: () => nodeEvent.id } }]) assert.throws(() => signedEventDigest({ ...nodeEvent, ...bad }), /closed seven-field/);
  const sparse = new Array(1), extraTag = ['d']; extraTag.unknown = true;
  const getterTag = ['d']; Object.defineProperty(getterTag, 0, { get() { throw Error('must not read a tag accessor'); } });
  for (const tags of [sparse, [sparse], [extraTag], [getterTag]]) assert.throws(() => signedEventDigest({ ...nodeEvent, tags }), /closed seven-field/);
  const inherited = structuredClone(nodeEvent); Object.setPrototypeOf(inherited.tags, { toJSON() { return []; } });
  assert.equal(signedEventDigest(inherited), signedEventDigest(nodeEvent), 'Inherited serialization hooks do not replace ordered validated tags');
});

test('raw request byte ownership is retained separately from stable signed-envelope equality', () => {
  const source = read('../../scripts/browser-signed-journeys.mjs');
  assert.match(source, /exactRequestBytes = digest\(JSON.stringify\(saved.event\)\)/);
  assert.match(source, /row.bodyDigest === exactRequestBytes/);
  assert.match(source, /verify\(value.body\)/);
  assert.match(source, /signedCounts.get\(0\) === signed && posts.length === 2 && posts.every\(row => row.origin === protocol\)/);
});
