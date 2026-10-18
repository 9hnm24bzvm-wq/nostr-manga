import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { createHash, generateKeyPairSync } from 'node:crypto';
import { MAX_ARTIFACT, SealedLogRefused, openEnvelope, recipientFingerprint,
  sealEnvelope, sealRecordedLog, validateEnvelope, validateRecordedEnvelope } from '../../scripts/ci/sealed-sandbox-log.mjs';

// Synthetic, process-memory-only keys. No private key is exported, logged or written.
const recipient = generateKeyPairSync('rsa', { modulusLength: 3072 });
const other = generateKeyPairSync('rsa', { modulusLength: 3072 });
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const context = () => ({ scope: 'sandbox-preflight-worker-log', repository: '9hnm24bzvm-wq/nostr-manga',
  run_id: '123456789', run_attempt: '1', source_commit: 'a'.repeat(40),
  public_record_sha256: 'b'.repeat(64), recipient_spki_sha256: recipientFingerprint(recipient.publicKey) });
const encoded = value => Buffer.from(JSON.stringify(value) + '\n');
const refused = fn => assert.throws(fn, error => error instanceof SealedLogRefused
  && error.message === 'Sealed sandbox log refused');
const mutate = (bytes, fn) => { const value = JSON.parse(bytes); fn(value); return encoded(value); };
const plain = Buffer.from('Synthetic startup only: https://fixture.invalid/path?token=synthetic-private-value\n');

test('exact binary plaintext roundtrips only after authentication; no plaintext appears in envelope', () => {
  const input = Buffer.concat([plain, Buffer.from([0, 255, 128])]);
  const sealed = sealEnvelope(input, recipient.publicKey, context());
  assert.deepEqual(openEnvelope(sealed, recipient.privateKey, context()), input);
  assert.equal(validateEnvelope(sealed, context()), true);
  assert.equal(sealed.includes(Buffer.from('synthetic-private-value')), false);
  assert.equal(sealed.includes(Buffer.from('PRIVATE KEY')), false);
});
test('fresh encryption differs for the same plaintext/context', () => {
  const a = sealEnvelope(plain, recipient.publicKey, context());
  const b = sealEnvelope(plain, recipient.publicKey, context());
  assert.notDeepEqual(a, b);
  assert.notEqual(JSON.parse(a).nonce, JSON.parse(b).nonce);
  assert.notEqual(JSON.parse(a).wrapped_key, JSON.parse(b).wrapped_key);
});
test('every independently expected context component is mandatory and exact', () => {
  const sealed = sealEnvelope(plain, recipient.publicKey, context());
  const changes = { scope: 'other', repository: 'other/repo', run_id: '123456790', run_attempt: '2',
    source_commit: 'c'.repeat(40), public_record_sha256: 'd'.repeat(64),
    recipient_spki_sha256: recipientFingerprint(other.publicKey) };
  for (const [key, value] of Object.entries(changes)) {
    refused(() => openEnvelope(sealed, recipient.privateKey, { ...context(), [key]: value }));
    refused(() => sealEnvelope(plain, recipient.publicKey, { ...context(), [key]: undefined }));
  }
});
test('editing embedded and expected valid run/source/public hash together still fails authentication', () => {
  const sealed = sealEnvelope(plain, recipient.publicKey, context());
  for (const [key, value] of Object.entries({ run_id: '2', run_attempt: '2',
    source_commit: 'd'.repeat(40), public_record_sha256: 'e'.repeat(64) })) {
    const expected = { ...context(), [key]: value };
    const changed = mutate(sealed, e => { e.context[key] = value; });
    assert.equal(validateEnvelope(changed, expected), true); // Structural validation is not authentication.
    refused(() => openEnvelope(changed, recipient.privateKey, expected));
  }
});
test('wrapped key, nonce, ciphertext and tag modifications never return partial plaintext', () => {
  const sealed = sealEnvelope(plain, recipient.publicKey, context());
  for (const key of ['wrapped_key', 'nonce', 'ciphertext', 'tag']) {
    const changed = mutate(sealed, e => { const bytes = Buffer.from(e[key], 'base64'); bytes[0] ^= 1; e[key] = bytes.toString('base64'); });
    refused(() => openEnvelope(changed, recipient.privateKey, context()));
  }
});
test('wrong recipient/private key, non-RSA and wrong RSA size fail closed', () => {
  const sealed = sealEnvelope(plain, recipient.publicKey, context());
  refused(() => openEnvelope(sealed, other.privateKey, context()));
  refused(() => sealEnvelope(plain, other.publicKey, context()));
  const weak = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const ec = generateKeyPairSync('ec', { namedCurve: 'prime256v1' });
  for (const pair of [weak, ec]) refused(() => recipientFingerprint(pair.publicKey));
  refused(() => sealEnvelope(plain, recipient.privateKey, context()));
});
test('unknown envelope fields/algorithms/schema and malformed canonical encodings refuse', () => {
  const sealed = sealEnvelope(plain, recipient.publicKey, context());
  const cases = [mutate(sealed, e => { e.extra = 'https://secret.invalid'; }),
    mutate(sealed, e => { e.algorithm = 'RSA1_5'; }), mutate(sealed, e => { e.schema = 2; }),
    mutate(sealed, e => { e.nonce = 'AAAA'; }), mutate(sealed, e => { e.tag += '='; }),
    mutate(sealed, e => { e.context.extra = true; }),
    mutate(sealed, e => { e.context = Object.fromEntries(Object.entries(e.context).reverse()); }),
    Buffer.from(sealed.toString().replace('"schema":1', '"schema":1,"schema":1')),
    Buffer.concat([sealed, Buffer.from(' ')]), Buffer.from([255]),
    mutate(sealed, e => { e.ciphertext = 'A==='; })];
  for (const value of cases) refused(() => openEnvelope(value, recipient.privateKey, context()));
});
test('context accessors/prototype/symbols/unknown keys and hostile coercion are refused', () => {
  let touched = 0;
  const bad = { ...context() };
  Object.defineProperty(bad, 'run_id', { enumerable: true, get() { touched++; throw Error('private'); } });
  refused(() => sealEnvelope(plain, recipient.publicKey, bad)); assert.equal(touched, 0);
  for (const c of [{ ...context(), extra: true }, { ...context(), [Symbol('private')]: 1 },
    Object.assign(Object.create({ private: true }), context()), { ...context(), run_id: { toString() { touched++; } } }])
    refused(() => sealEnvelope(plain, recipient.publicKey, c));
  assert.equal(touched, 0);
});
test('strict scalar types and decimal/hash ranges refuse without coercion', () => {
  for (const [key, values] of Object.entries({ run_id: ['0', '01', '1\n', 1, '1'.repeat(21)],
    run_attempt: ['0', '01', '1000000', 1], source_commit: ['A'.repeat(40), 'a'.repeat(39)],
    public_record_sha256: ['B'.repeat(64), 'b'.repeat(63)], recipient_spki_sha256: [null] }))
    for (const value of values) refused(() => sealEnvelope(plain, recipient.publicKey, { ...context(), [key]: value }));
  refused(() => sealEnvelope('not bytes', recipient.publicKey, context()));
});
test('the FULL base64 JSON artifact is bounded; maximum representable body passes and +1 refuses', () => {
  const overhead = sealEnvelope(Buffer.alloc(0), recipient.publicKey, context()).length;
  const maximum = 3 * Math.floor((MAX_ARTIFACT - overhead) / 4);
  const input = Buffer.alloc(maximum, 17), sealed = sealEnvelope(input, recipient.publicKey, context());
  assert.ok(sealed.length <= MAX_ARTIFACT && sealed.length >= MAX_ARTIFACT - 3);
  assert.deepEqual(openEnvelope(sealed, recipient.privateKey, context()), input);
  refused(() => sealEnvelope(Buffer.alloc(maximum + 1), recipient.publicKey, context()));
  refused(() => sealEnvelope(Buffer.alloc(MAX_ARTIFACT + 1), recipient.publicKey, context()));
  refused(() => openEnvelope(Buffer.alloc(MAX_ARTIFACT + 1), recipient.privateKey, context()));
});

function fixture() {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'sealed-synthetic-')));
  const privateDir = path.join(root, '.build/sandbox-preflight');
  fs.mkdirSync(privateDir, { recursive: true, mode: 0o700 });
  fs.mkdirSync(path.join(root, '.build/sandbox-preflight-recipient'), { recursive: true });
  fs.mkdirSync(path.join(root, 'ci-evidence'));
  const record = Buffer.from('{"synthetic_record":true}\n');
  fs.writeFileSync(path.join(root, 'ci-evidence/sandbox-preflight.json'), record);
  fs.writeFileSync(path.join(privateDir, 'worker.log'), plain);
  const receipt = { schema: 1, public_record_sha256: sha(record), log_sha256: sha(plain), log_bytes: plain.length };
  fs.writeFileSync(path.join(privateDir, 'seal-ready.json'), '{"log_bytes": ' + receipt.log_bytes
    + ', "log_sha256": "' + receipt.log_sha256 + '", "public_record_sha256": "'
    + receipt.public_record_sha256 + '", "schema": 1}\n');
  fs.writeFileSync(path.join(root, '.build/sandbox-preflight-recipient/public.pem'), recipient.publicKey.export({ format: 'pem', type: 'spki' }));
  const env = { GITHUB_REPOSITORY: context().repository, GITHUB_RUN_ID: context().run_id,
    GITHUB_RUN_ATTEMPT: context().run_attempt, GITHUB_SHA: context().source_commit };
  const args = { root, env, expectedEvidenceSha256: sha(record), expectedRecipientSha256: context().recipient_spki_sha256 };
  return { root, privateDir, record, receipt, args };
}
// Retain owned synthetic fixture directories: no test cleanup/deletion authority is needed.
test('recorded-log path binds exact closed producer bytes and exclusively creates a private envelope', () => {
  const f = fixture(), result = sealRecordedLog(f.args);
  const output = path.join(f.privateDir, 'sealed-worker-log.json'), bytes = fs.readFileSync(output);
  assert.equal(result.sha256, sha(bytes)); assert.equal(result.bytes, bytes.length);
  assert.equal(fs.statSync(output).mode & 0o777, 0o600);
  assert.deepEqual(openEnvelope(bytes, recipient.privateKey, { ...context(), public_record_sha256: sha(f.record) }), plain);
  assert.equal(validateRecordedEnvelope({ ...f.args, expectedSealedSha256: result.sha256 }), true);
  refused(() => sealRecordedLog(f.args)); assert.deepEqual(fs.readFileSync(output), bytes);
});
test('upload validation needs the current exact sealed hash, full context and unchanged public bytes', () => {
  const f = fixture(), result = sealRecordedLog(f.args);
  refused(() => validateRecordedEnvelope({ ...f.args, expectedSealedSha256: '0'.repeat(64) }));
  refused(() => validateRecordedEnvelope({ ...f.args, expectedSealedSha256: result.sha256,
    env: { ...f.args.env, GITHUB_RUN_ATTEMPT: '2' } }));
  const output = path.join(f.privateDir, 'sealed-worker-log.json'), original = fs.readFileSync(output);
  fs.appendFileSync(output, ' ');
  refused(() => validateRecordedEnvelope({ ...f.args, expectedSealedSha256: result.sha256 }));
  fs.writeFileSync(output, original);
  fs.appendFileSync(path.join(f.root, 'ci-evidence/sandbox-preflight.json'), ' ');
  refused(() => validateRecordedEnvelope({ ...f.args, expectedSealedSha256: result.sha256 }));
});
test('stale public hash, changed log, bad receipt and wrong approved recipient produce no artifact', () => {
  const changes = [f => { f.args.expectedEvidenceSha256 = '0'.repeat(64); },
    f => fs.appendFileSync(path.join(f.privateDir, 'worker.log'), 'changed'),
    f => { f.receipt.extra = 'private'; fs.writeFileSync(path.join(f.privateDir, 'seal-ready.json'), JSON.stringify(f.receipt)); },
    f => { f.receipt.log_bytes++; fs.writeFileSync(path.join(f.privateDir, 'seal-ready.json'), JSON.stringify(f.receipt)); },
    f => fs.appendFileSync(path.join(f.root, '.build/sandbox-preflight-recipient/public.pem'), 'synthetic trailing material'),
    f => { f.args.expectedRecipientSha256 = recipientFingerprint(other.publicKey); }];
  for (const change of changes) { const f = fixture(); change(f); refused(() => sealRecordedLog(f.args));
    assert.equal(fs.existsSync(path.join(f.privateDir, 'sealed-worker-log.json')), false); }
});
test('symlinked or hardlinked log input and existing output are never adopted or overwritten', () => {
  for (const kind of ['symlink', 'hardlink', 'existing']) {
    const f = fixture(), output = path.join(f.privateDir, 'sealed-worker-log.json');
    if (kind === 'existing') fs.writeFileSync(output, 'previous generation');
    else {
      const target = path.join(f.privateDir, 'other.log'); fs.renameSync(path.join(f.privateDir, 'worker.log'), target);
      if (kind === 'symlink') fs.symlinkSync(target, path.join(f.privateDir, 'worker.log'));
      else fs.linkSync(target, path.join(f.privateDir, 'worker.log'));
    }
    refused(() => sealRecordedLog(f.args));
    if (kind === 'existing') assert.equal(fs.readFileSync(output, 'utf8'), 'previous generation');
    else assert.equal(fs.existsSync(output), false);
  }
});
