// Source preparation only. This module never starts a browser or generates a keypair.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { constants, createCipheriv, createDecipheriv, createHash, KeyObject,
  createPublicKey, privateDecrypt, publicEncrypt, randomBytes } from 'node:crypto';
import { TextDecoder } from 'node:util';

export const MAX_ARTIFACT = 4 * 1024 * 1024;
const ALGORITHM = 'RSA-OAEP-SHA256-3072+A256GCM';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const CONTEXT_KEYS = ['scope', 'repository', 'run_id', 'run_attempt', 'source_commit',
  'public_record_sha256', 'recipient_spki_sha256'];
const ENVELOPE_KEYS = ['schema', 'algorithm', 'context', 'wrapped_key', 'nonce', 'ciphertext', 'tag'];
const HEX = /^[0-9a-f]{64}$/;
const utf8 = new TextDecoder('utf-8', { fatal: true });
export class SealedLogRefused extends Error {
  constructor() { super('Sealed sandbox log refused'); }
}
const refuse = () => { throw new SealedLogRefused(); };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
function dataObject(value, keys) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) refuse();
  const descriptors = Object.getOwnPropertyDescriptors(value);
  if (Reflect.ownKeys(descriptors).length !== keys.length
      || keys.some(key => !Object.hasOwn(descriptors, key)
        || !Object.hasOwn(descriptors[key], 'value'))) refuse();
  return Object.fromEntries(keys.map(key => [key, descriptors[key].value]));
}
function contextValue(value) {
  const c = dataObject(value, CONTEXT_KEYS);
  if (c.scope !== 'sandbox-preflight-worker-log'
      || c.repository !== '9hnm24bzvm-wq/nostr-manga'
      || typeof c.run_id !== 'string' || !/^[1-9][0-9]{0,19}$/.test(c.run_id)
      || typeof c.run_attempt !== 'string' || !/^[1-9][0-9]{0,5}$/.test(c.run_attempt)
      || typeof c.source_commit !== 'string' || !/^[0-9a-f]{40}$/.test(c.source_commit)
      || typeof c.public_record_sha256 !== 'string' || !HEX.test(c.public_record_sha256)
      || typeof c.recipient_spki_sha256 !== 'string' || !HEX.test(c.recipient_spki_sha256)) refuse();
  return c;
}
function rsaKey(key, type) {
  // Accept existing KeyObjects only: no secret PEM, passphrase or key path API.
  if (!(key instanceof KeyObject) || key.type !== type
      || key.asymmetricKeyType !== 'rsa' || key.asymmetricKeyDetails?.modulusLength !== 3072
      || key.asymmetricKeyDetails?.publicExponent !== 65537n) refuse();
  return key;
}
export function recipientFingerprint(publicKey) {
  try { return sha256(rsaKey(publicKey, 'public').export({ format: 'der', type: 'spki' })); }
  catch { refuse(); }
}
function b64(value, length = null) {
  if (typeof value !== 'string' || value.length > MAX_ARTIFACT
      || value.length % 4 !== 0 || /[^A-Za-z0-9+/=]/.test(value)) refuse();
  const bytes = Buffer.from(value, 'base64');
  if (bytes.toString('base64') !== value || (length !== null && bytes.length !== length)) refuse();
  return bytes;
}
const encode = value => Buffer.from(JSON.stringify(value) + '\n', 'utf8');
function inspect(bytes, expectedContext) {
  if (!Buffer.isBuffer(bytes) || bytes.length > MAX_ARTIFACT) refuse();
  const parsed = JSON.parse(utf8.decode(bytes));
  const e = dataObject(parsed, ENVELOPE_KEYS);
  const context = contextValue(e.context), expected = contextValue(expectedContext);
  // Exact canonical bytes also reject duplicate keys, alternate encodings and trailing data.
  if (JSON.stringify(context) !== JSON.stringify(expected) || e.schema !== 1
      || e.algorithm !== ALGORITHM || !encode({ ...e, context }).equals(bytes)) refuse();
  return { context, aad: encode(context), wrapped: b64(e.wrapped_key, 384),
    nonce: b64(e.nonce, 12), ciphertext: b64(e.ciphertext), tag: b64(e.tag, 16) };
}
export function validateEnvelope(bytes, expectedContext) {
  try { inspect(bytes, expectedContext); return true; }
  catch { refuse(); }
}
export function sealEnvelope(plaintext, publicKey, context) {
  let key;
  try {
    if (!Buffer.isBuffer(plaintext) || plaintext.length > MAX_ARTIFACT) refuse();
    const c = contextValue(context);
    if (recipientFingerprint(publicKey) !== c.recipient_spki_sha256) refuse();
    const aad = encode(c);
    key = randomBytes(32);
    const nonce = randomBytes(12), cipher = createCipheriv('aes-256-gcm', key, nonce);
    cipher.setAAD(aad);
    const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
    const wrapped = publicEncrypt({ key: publicKey, padding: constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: 'sha256', oaepLabel: aad }, key);
    const bytes = encode({ schema: 1, algorithm: ALGORITHM, context: c,
      wrapped_key: wrapped.toString('base64'), nonce: nonce.toString('base64'),
      ciphertext: ciphertext.toString('base64'), tag: cipher.getAuthTag().toString('base64') });
    if (bytes.length > MAX_ARTIFACT) refuse(); // Refuse the entire artifact; never truncate.
    inspect(bytes, c);
    return bytes;
  } catch { refuse(); }
  finally { key?.fill(0); }
}
export function openEnvelope(bytes, privateKey, expectedContext) {
  let key, partial;
  try {
    const e = inspect(bytes, expectedContext);
    rsaKey(privateKey, 'private');
    if (recipientFingerprint(createPublicKey(privateKey)) !== e.context.recipient_spki_sha256) refuse();
    key = privateDecrypt({ key: privateKey, padding: constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: 'sha256', oaepLabel: e.aad }, e.wrapped);
    if (key.length !== 32) refuse();
    const decipher = createDecipheriv('aes-256-gcm', key, e.nonce);
    decipher.setAAD(e.aad); decipher.setAuthTag(e.tag);
    partial = decipher.update(e.ciphertext);
    const tail = decipher.final(); // No plaintext leaves this function before authentication.
    return Buffer.concat([partial, tail]);
  } catch { refuse(); }
  finally { key?.fill(0); partial?.fill(0); }
}
function stableRead(file, limit) {
  if (fs.realpathSync(file) !== file) refuse();
  // O_NONBLOCK makes FIFO/device admission bounded before the regular-file fstat.
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    if (!before.isFile() || before.size > BigInt(limit) || before.nlink !== 1n) refuse();
    const bytes = Buffer.alloc(Number(before.size) + 1);
    let count = 0, n;
    do { n = fs.readSync(fd, bytes, count, bytes.length - count, null); count += n; }
    while (n && count < bytes.length);
    const after = fs.fstatSync(fd, { bigint: true });
    if (BigInt(count) !== before.size || ['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs']
      .some(key => before[key] !== after[key])) refuse();
    return bytes.subarray(0, count);
  } finally { fs.closeSync(fd); }
}
function readJson(bytes) {
  const value = JSON.parse(utf8.decode(bytes));
  // Producer JSON is canonical sort_keys output, checked by the owner's exact hash.
  if (value === null || typeof value !== 'object' || Array.isArray(value)) refuse();
  return value;
}
function recordedContext(env, expectedEvidenceSha256, expectedRecipientSha256) {
  return contextValue({ scope: 'sandbox-preflight-worker-log',
    repository: env.GITHUB_REPOSITORY, run_id: env.GITHUB_RUN_ID, run_attempt: env.GITHUB_RUN_ATTEMPT,
    source_commit: env.GITHUB_SHA, public_record_sha256: expectedEvidenceSha256,
    recipient_spki_sha256: expectedRecipientSha256 });
}
export function sealRecordedLog({ root, env, expectedEvidenceSha256, expectedRecipientSha256 }) {
  try {
    if (typeof root !== 'string' || !path.isAbsolute(root) || fs.realpathSync(root) !== root
        || typeof expectedEvidenceSha256 !== 'string' || !HEX.test(expectedEvidenceSha256)
        || typeof expectedRecipientSha256 !== 'string' || !HEX.test(expectedRecipientSha256)) refuse();
    const directory = path.join(root, '.build/sandbox-preflight');
    if (fs.realpathSync(directory) !== directory) refuse();
    const publicBytes = stableRead(path.join(root, 'ci-evidence/sandbox-preflight.json'), 65536);
    if (sha256(publicBytes) !== expectedEvidenceSha256) refuse();
    const receiptBytes = stableRead(path.join(directory, 'seal-ready.json'), 65536);
    const receipt = dataObject(readJson(receiptBytes),
      ['schema', 'public_record_sha256', 'log_sha256', 'log_bytes']);
    if (receipt.schema !== 1 || receipt.public_record_sha256 !== expectedEvidenceSha256
        || typeof receipt.log_sha256 !== 'string' || !HEX.test(receipt.log_sha256)
        || !Number.isSafeInteger(receipt.log_bytes) || receipt.log_bytes < 0
        || receipt.log_bytes > MAX_ARTIFACT) refuse();
    const canonicalReceipt = Buffer.from('{"log_bytes": ' + receipt.log_bytes
      + ', "log_sha256": "' + receipt.log_sha256 + '", "public_record_sha256": "'
      + receipt.public_record_sha256 + '", "schema": 1}\n');
    if (!receiptBytes.equals(canonicalReceipt)) refuse(); // Includes duplicate-key refusal.
    const log = stableRead(path.join(directory, 'worker.log'), MAX_ARTIFACT);
    if (log.length !== receipt.log_bytes || sha256(log) !== receipt.log_sha256) refuse();
    const publicPem = stableRead(path.join(root, '.build/sandbox-preflight-recipient/public.pem'), 16384);
    if (!publicPem.subarray(0, 27).equals(Buffer.from('-----BEGIN PUBLIC KEY-----\n'))) refuse();
    const publicKey = createPublicKey(publicPem);
    if (!Buffer.from(publicKey.export({ format: 'pem', type: 'spki' })).equals(publicPem)) refuse();
    const context = recordedContext(env, expectedEvidenceSha256, expectedRecipientSha256);
    const envelope = sealEnvelope(log, publicKey, context);
    const output = path.join(directory, 'sealed-worker-log.json');
    const fd = fs.openSync(output, fs.constants.O_WRONLY | fs.constants.O_CREAT
      | fs.constants.O_EXCL | fs.constants.O_NOFOLLOW, 0o600);
    try { fs.writeFileSync(fd, envelope); fs.fsyncSync(fd); }
    finally { fs.closeSync(fd); }
    const written = stableRead(output, MAX_ARTIFACT);
    if (!written.equals(envelope)) refuse();
    validateEnvelope(written, context);
    return { sha256: sha256(written), bytes: written.length };
  } catch { refuse(); }
}
export function validateRecordedEnvelope({ root, env, expectedEvidenceSha256,
  expectedRecipientSha256, expectedSealedSha256 }) {
  try {
    if (typeof root !== 'string' || !path.isAbsolute(root) || fs.realpathSync(root) !== root
        || typeof expectedSealedSha256 !== 'string' || !HEX.test(expectedSealedSha256)) refuse();
    const context = recordedContext(env, expectedEvidenceSha256, expectedRecipientSha256);
    if (sha256(stableRead(path.join(root, 'ci-evidence/sandbox-preflight.json'), 65536))
        !== context.public_record_sha256) refuse();
    // Hash and structural/context checks use exactly the same bounded byte buffer.
    const bytes = stableRead(path.join(root, '.build/sandbox-preflight/sealed-worker-log.json'), MAX_ARTIFACT);
    if (sha256(bytes) !== expectedSealedSha256) refuse();
    return validateEnvelope(bytes, context);
  } catch { refuse(); }
}
function main() {
  const args = process.argv.slice(2);
  const validate = args[0] === '--validate';
  if (validate) args.shift();
  if (args.length !== (validate ? 6 : 4) || args[0] !== '--expected-evidence-sha256'
      || args[2] !== '--expected-recipient-sha256') refuse();
  const options = { root: ROOT, env: process.env,
    expectedEvidenceSha256: args[1], expectedRecipientSha256: args[3] };
  if (validate) {
    if (args[4] !== '--expected-sealed-sha256') refuse();
    validateRecordedEnvelope({ ...options, expectedSealedSha256: args[5] });
    return;
  }
  const result = sealRecordedLog(options);
  // An eventual workflow may upload only this exact fresh closed generation.
  console.log('sealed_sha256=' + result.sha256);
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); }
  catch { console.error('Sealed sandbox log refused'); process.exitCode = 1; }
}
