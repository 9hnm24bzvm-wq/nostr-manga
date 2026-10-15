#!/usr/bin/env node
// Real pinned curve/hash/Bech32 implementations and production signer modules.
// Synthetic keys only. No browser, real account, clipboard, storage or network.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

assert(vm.SourceTextModule, 'Run with node --experimental-vm-modules');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const source = path.join(root, 'platform-integrations/local-library');
const noble = path.join(root, '.build/local-library-noble-2.0.1/node_modules/@noble');
const base = path.join(root, '.build/local-signing-base-2.0.0/node_modules/@scure/base');
const curves = await import(pathToFileURL(path.join(noble, 'curves/secp256k1.js')));
const hashes = await import(pathToFileURL(path.join(noble, 'hashes/sha2.js')));
const encoding = await import(pathToFileURL(path.join(base, 'index.js')));
for (const [directory, version] of [[path.join(noble, 'curves'), '2.0.1'], [path.join(noble, 'hashes'), '2.0.1'], [base, '2.0.0']]) assert.equal(JSON.parse(fs.readFileSync(path.join(directory, 'package.json'))).version, version);
const one = '0'.repeat(63) + '1', two = '0'.repeat(63) + '2';
const scalarOrder = 'fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141';
const fromHex = value => Uint8Array.from(Buffer.from(value, 'hex'));
const encodeSecret = value => encoding.bech32.encode('nsec', encoding.bech32.toWords(fromHex(value)));
function writable(value) {
  const listeners = new Set();
  return { subscribe(fn) { listeners.add(fn); fn(value); return () => listeners.delete(fn); }, set(next) { value = next; for (const fn of listeners) fn(value); }, update(fn) { this.set(fn(value)); } };
}
const get = store => { let value; store.subscribe(next => { value = next; })(); return value; };

async function fixture({ random, secureRandom = true } = {}) {
  const calls = { signatures: 0, network: 0, keys: [], randomBuffers: [] };
  const never = label => () => { throw Error('Forbidden boundary: ' + label); };
  const externalKey = fromHex(two), externalOwner = Buffer.from(curves.schnorr.getPublicKey(externalKey)).toString('hex');
  const external = { async getPublicKey() { return externalOwner; }, async signEvent() { throw Error('External signature was not requested in this fixture'); } };
  const context = vm.createContext({ Uint8Array, ArrayBuffer, TextEncoder, TextDecoder, structuredClone, URL, Date, JSON, performance,
    setTimeout, clearTimeout, queueMicrotask, AbortSignal, btoa, atob,
    crypto: secureRandom ? { getRandomValues(buffer) { calls.randomBuffers.push(buffer); return random ? random(buffer, calls.randomBuffers.length) : crypto.getRandomValues(buffer); } } : {},
    window: { nostr: external, location: { origin: 'https://example.invalid' } },
    console: { log: never('logging'), error: never('error logging'), warn: never('warning logging') },
    fetch: () => { calls.network++; throw Error('Network forbidden'); },
    navigator: { clipboard: { writeText: never('clipboard') } },
    localStorage: { getItem: never('localStorage'), setItem: never('localStorage') },
    sessionStorage: { getItem: never('sessionStorage'), setItem: never('sessionStorage') },
    indexedDB: { open: never('IndexedDB') },
    Worker: class { constructor() { throw Error('Worker forbidden'); } },
  });
  const modules = new Map();
  function synthetic(id, values) {
    const module = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context, identifier: id });
    modules.set(id, module); return module;
  }
  synthetic('svelte/store', { writable, get });
  synthetic('$env/static/public', { PUBLIC_API_URL: '/api' });
  synthetic('@noble/curves/secp256k1.js', { ...curves, schnorr: { ...curves.schnorr,
    getPublicKey(key) { calls.keys.push(key); return curves.schnorr.getPublicKey(key); },
    sign(...args) { calls.signatures++; return curves.schnorr.sign(...args); },
  } });
  synthetic('@noble/hashes/sha2.js', hashes);
  synthetic('@scure/base', encoding);
  synthetic('library', Object.fromEntries(['stores', 'currentDeviceSnapshot', 'apiRead', 'refreshLibrary', 'verifyCatalogueScope', 'message'].map(name => [name, never(name)])));
  synthetic('catalog', { scopedCatalogCandidates: never('catalog') });
  synthetic('device-store', { deviceWriteLock: never('device-write') });
  synthetic('protocol/publishing.js', Object.fromEntries(['queueSnapshot', 'deliverJob', 'cancelJob', 'recoverPublic'].map(name => [name, never(name)])));
  synthetic('protocol/backup.js', { backupBlob: never('backup') });
  function moduleFor(file) {
    if (modules.has(file)) return modules.get(file);
    let code = fs.readFileSync(file, 'utf8');
    if (file.endsWith('.ts')) code = stripTypeScriptTypes(code, { mode: 'transform' });
    const module = new vm.SourceTextModule(code, { context, identifier: file }); modules.set(file, module); return module;
  }
  function resolve(specifier, importer) {
    if (modules.has(specifier)) return modules.get(specifier);
    const target = path.resolve(path.dirname(importer.identifier), specifier);
    const relative = path.relative(source, target);
    if (modules.has(relative)) return modules.get(relative);
    return moduleFor(fs.existsSync(target) ? target : target + '.ts');
  }
  const local = moduleFor(path.join(source, 'local-signing-key.ts'));
  await local.link(resolve); await local.evaluate();
  return { keys: local.namespace, publication: modules.get(path.join(source, 'publication.ts')).namespace,
    nostr: modules.get(path.join(source, 'protocol/nostr.js')).namespace, calls, get };
}
const event = owner => ({ pubkey: owner, kind: 0, created_at: 1234567890, tags: [['p', 'public-review']], content: '{"name":"Synthetic reader"}' });

test('generation rejection-samples WebCrypto bytes and produces validated npub without selecting an identity', async () => {
  const f = await fixture({ random(buffer, count) { buffer.fill(0); if (count > 1) buffer[31] = 1; return buffer; } });
  const draft = f.keys.generateLocalKeyDraft();
  assert.equal(f.calls.randomBuffers.length, 2);
  assert.equal(draft.publicKey, Buffer.from(curves.schnorr.getPublicKey(fromHex(one))).toString('hex'));
  const decoded = encoding.bech32.decode(draft.npub);
  assert.equal(decoded.prefix, 'npub'); assert.equal(Buffer.from(encoding.bech32.fromWords(decoded.words)).toString('hex'), draft.publicKey);
  assert.equal(get(f.publication.signerIdentity).owner, null);
  assert.equal(draft.exportSecret() === encodeSecret(one), true);
  draft.discard(); assert.equal(f.calls.keys[0].every(value => value === 0), true);
  assert.throws(() => draft.exportSecret(), /no longer available/);
  await assert.rejects(draft.activate(), /no longer available/);
});

test('missing secure randomness and persistently invalid scalars fail closed', async () => {
  const missing = await fixture({ secureRandom: false });
  assert.throws(() => missing.keys.generateLocalKeyDraft(), /Secure browser randomness/);
  const zero = await fixture({ random: buffer => buffer.fill(0) });
  assert.throws(() => zero.keys.generateLocalKeyDraft(), /Secure key generation failed/);
  assert.equal(zero.calls.randomBuffers.length, 128); assert.equal(zero.calls.keys.length, 0);
});

test('hex import strictly admits only scalars between one and the curve order', async () => {
  const f = await fixture();
  for (const value of [one, (BigInt('0x' + scalarOrder) - 1n).toString(16), two.toUpperCase()]) {
    const draft = f.keys.importLocalKeyDraft(' ' + value + '\n'); assert.match(draft.npub, /^npub1/); draft.discard();
  }
  for (const value of ['0'.repeat(64), scalarOrder, 'f'.repeat(64), '1'.repeat(63), '1'.repeat(65), 'z'.repeat(64), '', 'nostr:' + encodeSecret(one)]) assert.throws(() => f.keys.importLocalKeyDraft(value), /valid nsec private key/);
  assert.equal(f.calls.network, 0); assert.equal(f.calls.signatures, 0);
});

test('nsec import checks prefix, Bech32 checksum, padding, length, case and scalar', async () => {
  const f = await fixture(), valid = encodeSecret(one);
  const draft = f.keys.importLocalKeyDraft(valid); assert.equal(draft.exportSecret() === valid, true); draft.discard();
  const invalid = [valid.slice(0, -1) + (valid.endsWith('q') ? 'p' : 'q'), valid.slice(0, 1).toUpperCase() + valid.slice(1),
    encoding.bech32.encode('npub', encoding.bech32.toWords(fromHex(one))),
    encoding.bech32.encode('nprofile', encoding.bech32.toWords(fromHex(one))),
    encoding.bech32.encode('nsec', encoding.bech32.toWords(new Uint8Array(31))),
    encoding.bech32m.encode('nsec', encoding.bech32m.toWords(fromHex(one))), encodeSecret('0'.repeat(64)), encodeSecret(scalarOrder)];
  for (const value of invalid) {
    assert.throws(() => f.keys.importLocalKeyDraft(value), error => error.message.includes('valid nsec private key') && !error.message.includes(value));
  }
  const uppercase = f.keys.importLocalKeyDraft(valid.toUpperCase()); assert.equal(uppercase.exportSecret() === valid, true); uppercase.discard();
});

test('pending draft has no signing authority and discard preserves an existing selected identity', async () => {
  const f = await fixture(); await f.publication.connectSigner(); const before = get(f.publication.signerIdentity);
  const draft = f.keys.importLocalKeyDraft(one); draft.discard();
  assert.equal(get(f.publication.signerIdentity), before);
  assert.equal(f.publication.selectedSigner().owner, before.owner); assert.equal(f.calls.signatures, 0);
});

test('activation installs only public identity in the common registry and transfers key ownership', async () => {
  const f = await fixture(), draft = f.keys.importLocalKeyDraft(one); await draft.activate();
  const identity = get(f.publication.signerIdentity);
  assert.deepEqual(Object.keys(identity).sort(), ['generation', 'owner', 'source']);
  assert.equal(identity.owner, draft.publicKey); assert.equal(identity.source, 'local');
  assert.equal(JSON.stringify(identity).includes(encodeSecret(one)), false);
  assert.throws(() => draft.exportSecret(), /no longer available/);
  draft.discard(); assert.equal(f.publication.selectedSigner().owner, draft.publicKey);
  await assert.rejects(draft.activate(), /no longer available/);
});

test('real NIP-01 canonical hash and Schnorr signature pass existing signExact and verifyEvent', async () => {
  const f = await fixture(), draft = f.keys.importLocalKeyDraft(one); await draft.activate();
  const selected = f.publication.selectedSigner(), template = event(selected.owner);
  const signed = await f.nostr.signExact(selected.signer, selected.owner, template);
  assert.equal(f.nostr.verifyEvent(signed), true); assert.equal(f.calls.signatures, 1);
  assert.equal(signed.id, Buffer.from(hashes.sha256(new TextEncoder().encode(JSON.stringify([0, selected.owner, template.created_at, template.kind, template.tags, template.content])))).toString('hex'));
  assert.equal(f.nostr.verifyEvent({ ...signed, content: signed.content + ' ' }), false);
  assert.equal(f.nostr.verifyEvent({ ...signed, sig: '0'.repeat(128) }), false);
  assert.equal(JSON.stringify(signed).includes(encodeSecret(one)), false);
  assert.equal(f.calls.network, 0);
});

test('invalid or oversized templates are rejected before accessing private-key signing', async () => {
  const f = await fixture(), draft = f.keys.importLocalKeyDraft(one); await draft.activate(); const selected = f.publication.selectedSigner();
  for (const template of [{ ...event(selected.owner), pubkey: 'f'.repeat(64) }, { ...event(selected.owner), kind: -1 }, { ...event(selected.owner), content: 'x'.repeat(16384) }, { ...event(selected.owner), tags: [['bad', 1]] }, { ...event(selected.owner), unknown: true }]) await assert.rejects(selected.signer.signEvent(template));
  assert.equal(f.calls.signatures, 0);
});

test('Lock zeros selected mutable secret and invalidates all old signer handles', async () => {
  const f = await fixture(), draft = f.keys.importLocalKeyDraft(one); await draft.activate(); const selected = f.publication.selectedSigner();
  f.publication.disconnectSigner();
  assert.equal(f.calls.keys[0].every(value => value === 0), true);
  assert.equal(get(f.publication.signerIdentity).owner, null); assert.equal(get(f.publication.signerIdentity).source, null);
  assert.throws(selected.check, /identity changed/); await assert.rejects(selected.signer.signEvent(event(selected.owner)), /identity changed/);
  assert.equal(f.calls.signatures, 0);
});

test('replacing local identity or connecting an extension clears old bytes and fences deferred signing', async () => {
  const f = await fixture(), first = f.keys.importLocalKeyDraft(one); await first.activate(); const previous = f.publication.selectedSigner();
  const pending = f.nostr.signExact(previous.signer, previous.owner, event(previous.owner));
  const next = f.keys.importLocalKeyDraft(two); await next.activate(); await assert.rejects(pending, /identity changed/);
  assert.equal(f.calls.keys[0].every(value => value === 0), true); assert.equal(f.calls.signatures, 0);
  await f.publication.connectSigner(); assert.equal(get(f.publication.signerIdentity).source, 'external');
  assert.equal(f.calls.keys[1].every(value => value === 0), true);
});

test('stale draft and action lifetime cannot export, activate or create a deferred signature', async () => {
  const f = await fixture(); let alive = true; const current = () => { if (!alive) throw Error('UI review closed'); };
  const pendingDraft = f.keys.importLocalKeyDraft(one, current); alive = false;
  assert.throws(() => pendingDraft.exportSecret(), /UI review closed/); await assert.rejects(pendingDraft.activate(current), /UI review closed/); pendingDraft.discard();
  alive = true; const draft = f.keys.importLocalKeyDraft(two, current); await draft.activate(current);
  const identity = f.publication.selectedSigner();
  const reviewed = { signEvent(template) { current(); return identity.signer.signEvent(template); } };
  const pending = f.nostr.signExact(reviewed, identity.owner, event(identity.owner)); alive = false;
  await assert.rejects(pending, /UI review closed/); assert.equal(f.calls.signatures, 0); assert.equal(f.calls.network, 0);
});

test('public npub encoding validates the actual curve point and capability metadata is truthful', async () => {
  const f = await fixture(); assert.throws(() => f.keys.encodePublicKey('f'.repeat(64))); assert.throws(() => f.keys.encodePublicKey('1'.repeat(63)));
  const backend = fs.readFileSync(path.join(root, 'backend/src/protocol/adapters.rs'), 'utf8');
  assert.match(backend, /"external_signer_required":false/); assert.match(backend, /"supported_signer_modes":\["local-session","nip07"\]/);
  const module = fs.readFileSync(path.join(source, 'local-signing-key.ts'), 'utf8');
  assert.doesNotMatch(module, /fetch\(|localStorage|sessionStorage|indexedDB|console\.|navigator\.clipboard|postMessage\(/);
});
