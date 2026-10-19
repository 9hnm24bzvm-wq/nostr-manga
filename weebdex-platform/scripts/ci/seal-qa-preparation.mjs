// Source preparation only. This module never starts a browser or generates a keypair.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { constants, createCipheriv, createDecipheriv, createHash, KeyObject,
  createPublicKey, privateDecrypt, publicEncrypt, randomBytes } from 'node:crypto';
import { TextDecoder } from 'node:util';

export const MAX_ARTIFACT = 4 * 1024 * 1024;
export const MAX_PLAINTEXT = 2 * 1024 * 1024;
export const MAX_LOG_SOURCE = 64 * 1024 * 1024;
export const MAX_LOG_TAIL = 512 * 1024;
export const QA_SCOPE = 'hosted-public-upload-qa-preparation-failure';
export const RECIPIENT = '06656641d644eb4fe046a53c085a62a9a0d853d0f712f1a92295337f479cf9e7';
const ALGORITHM = 'RSA-OAEP-SHA256-3072+A256GCM';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const CONTEXT_KEYS = ['scope', 'repository', 'run_id', 'run_attempt', 'source_commit',
  'public_record_sha256', 'recipient_spki_sha256'];
const ENVELOPE_KEYS = ['schema', 'algorithm', 'context', 'wrapped_key', 'nonce', 'ciphertext', 'tag'];
const HEX = /^[0-9a-f]{64}$/;
const utf8 = new TextDecoder('utf-8', { fatal: true });
export class SealedLogRefused extends Error {
  constructor() { super('Sealed QA preparation refused'); }
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
  if (c.scope !== QA_SCOPE
      || c.repository !== '9hnm24bzvm-wq/nostr-manga'
      || typeof c.run_id !== 'string' || !/^[1-9][0-9]{0,19}$/.test(c.run_id)
      || typeof c.run_attempt !== 'string' || !/^[1-9][0-9]{0,5}$/.test(c.run_attempt)
      || typeof c.source_commit !== 'string' || !/^[0-9a-f]{40}$/.test(c.source_commit)
      || typeof c.public_record_sha256 !== 'string' || !HEX.test(c.public_record_sha256)
      || c.recipient_spki_sha256 !== RECIPIENT) refuse();
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
  const ciphertext = b64(e.ciphertext);
  if (ciphertext.length > MAX_PLAINTEXT) refuse();
  return { context, aad: encode(context), wrapped: b64(e.wrapped_key, 384),
    nonce: b64(e.nonce, 12), ciphertext, tag: b64(e.tag, 16) };
}
export function validateEnvelope(bytes, expectedContext) {
  try { inspect(bytes, expectedContext); return true; }
  catch { refuse(); }
}
export function sealEnvelope(plaintext, publicKey, context) {
  let key;
  try {
    if (!Buffer.isBuffer(plaintext) || plaintext.length > MAX_PLAINTEXT) refuse();
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

const LABELS = ['public-upload-frontend-build', 'public-upload-artifact-record'];
const STAGES = ['public-upload-profile', 'public-upload-profile-preflight',
  'public-upload-profile-native-off', 'public-upload-profile-native-evidence',
  'public-upload-qa-build', 'public-upload-qa-source', 'public-upload-qa-receipt',
  'public-upload-qa-graph', 'public-upload-qa-record', 'public-upload-qa-final-source',
  'public-upload-qa-final-graph', 'public-upload-qa-seal', 'public-upload-profile-restore'];
const ERRORS = ['RuntimeError', 'TimeoutError', 'InterruptedError', 'FileNotFoundError',
  'PermissionError', 'IsADirectoryError', 'NotADirectoryError', 'OSError', 'JSONDecodeError',
  'KeyError', 'TypeError', 'ValueError', 'ProcessLookupError', 'ChildProcessError'];
const PREP_KEYS = ['artifacts_uploaded', 'browser_phases_started', 'error_class',
  'frontend_preserved', 'schema', 'source_files_checked', 'stage', 'status'];
const CAPTURE_KEYS = ['schema', 'status', 'stage', 'repository', 'run_id', 'run_attempt',
  'source_commit', 'preparation_sha256', 'source_snapshot_sha256', 'boundaries_sha256', 'traceback'];
const OWNED_KEYS = ['command', 'cwd', 'deadline_seconds', 'elapsed_seconds', 'process_exit_code',
  'deadline_exceeded', 'terminal_receipt', 'owned_process_group', 'cleanup_signals',
  'surviving_owned_process_ids', 'cleanup_verified', 'prior_or_unrelated_processes_touched'];
const SIGNALS = ['SIGTERM to newly owned process group', 'SIGKILL to newly owned process group',
  'SIGKILL to remaining newly owned group members'];
const RECORD = 'ci-sanitized-evidence/preparation.json';
const CAPTURE_DIR = '.build/hosted-browser/qa-preparation-failure';
const OUTPUT = 'ci-encrypted-evidence/qa-preparation-failure.json';
function noCredentialMarkers(bytes) {
  // Narrow refusal, never redaction or a general-purpose PII sanitizer. These
  // QA logs predate fixture identities and exclude dependency/env/browser logs.
  const value = bytes.toString('latin1');
  if (/-----BEGIN (?:RSA |EC |DSA |OPENSSH |ENCRYPTED )?PRIVATE KEY-----/.test(value)
      || /\bnsec1[023456789acdefghjklmnpqrstuvwxyz]{20,100}\b/i.test(value)
      || /\b(?:gh[pousr]_[A-Za-z0-9]{20,255}|github_pat_[A-Za-z0-9_]{20,255})\b/.test(value)
      || /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/.test(value)
      || /\b(?:Bearer|Nostr)\s+[A-Za-z0-9._~+\/-]{20,}/i.test(value)
      || /\b[A-Za-z][A-Za-z0-9+.-]{1,15}:\/\/[^\s/?#]+@/.test(value)
      || /(?:^|\n)\s*(?:GITHUB_TOKEN|OPENAI_API_KEY|AWS_SECRET_ACCESS_KEY|NOSTR_PRIVATE_KEY)\s*[:=]\s*\S{8,}/.test(value)) refuse();
}
const ordered = value => Array.isArray(value) ? value.map(ordered)
  : value !== null && typeof value === 'object'
    ? Object.fromEntries(Object.keys(value).sort().map(key => [key, ordered(value[key])])) : value;
function parseCanonical(bytes) {
  const value = JSON.parse(utf8.decode(bytes));
  if (!encode(ordered(value)).equals(bytes)) refuse();
  return value;
}
function recordedContext(env, expectedPreparationSha256) {
  return contextValue({ scope: QA_SCOPE, repository: env.GITHUB_REPOSITORY,
    run_id: env.GITHUB_RUN_ID, run_attempt: env.GITHUB_RUN_ATTEMPT,
    source_commit: env.GITHUB_SHA, public_record_sha256: expectedPreparationSha256,
    recipient_spki_sha256: RECIPIENT });
}
function publicPreparation(root, expectedPreparationSha256) {
  if (typeof expectedPreparationSha256 !== 'string' || !HEX.test(expectedPreparationSha256)) refuse();
  const bytes = stableRead(path.join(root, RECORD), 65536);
  if (sha256(bytes) !== expectedPreparationSha256) refuse();
  const value = dataObject(parseCanonical(bytes), PREP_KEYS);
  if (value.schema !== 1 || value.status !== 'preparation_failed'
      || value.browser_phases_started !== false || value.artifacts_uploaded !== false
      || value.frontend_preserved !== true || value.source_files_checked !== 676
      || !STAGES.includes(value.stage) || !ERRORS.includes(value.error_class)) refuse();
  const raw = stableRead(path.join(root, 'ci-evidence/preparation.json'), 65536);
  const rawParsed = JSON.parse(utf8.decode(raw));
  const rawValue = dataObject(rawParsed, PREP_KEYS);
  // The existing owner writes pretty JSON. Exact representation rejects duplicate keys.
  if (!Buffer.from(JSON.stringify(rawParsed, null, 2) + '\n').equals(raw)
      || !encode(ordered(rawValue)).equals(bytes)) refuse();
  return { value, rawSha256: sha256(raw) };
}
function ownedReceipt(bytes, root, label) {
  const receipt = dataObject(JSON.parse(utf8.decode(bytes)), OWNED_KEYS);
  // Python's producer emits these two fields as floats, even for whole values.
  let exact = JSON.stringify(receipt, null, 2) + '\n';
  for (const key of ['deadline_seconds', 'elapsed_seconds']) {
    if (Number.isInteger(receipt[key])) exact = exact.replace('"' + key + '": ' + receipt[key], '"' + key + '": ' + receipt[key] + '.0');
  }
  if (!Buffer.from(exact).equals(bytes) || receipt.cwd !== root
      || !Array.isArray(receipt.command)
      || JSON.stringify(receipt.command) !== JSON.stringify(label === LABELS[0]
        ? ['node', path.join(root, 'scripts/build-frontend.mjs')]
        : [path.join(root, '.build/hosted-python/bin/python'), path.join(root, 'scripts/record-frontend-artifacts.py'), 'hosted-public-upload-build/build-result.json'])
      || typeof receipt.deadline_seconds !== 'number' || !Number.isFinite(receipt.deadline_seconds)
      || receipt.deadline_seconds <= 0 || receipt.deadline_seconds > (label === LABELS[0] ? 900 : 60)
      || typeof receipt.elapsed_seconds !== 'number' || !Number.isFinite(receipt.elapsed_seconds) || receipt.elapsed_seconds < 0 || receipt.elapsed_seconds > 86400
      || !Number.isInteger(receipt.process_exit_code) || receipt.process_exit_code < -128 || receipt.process_exit_code > 255
      || typeof receipt.deadline_exceeded !== 'boolean' || receipt.terminal_receipt !== true
      || !Number.isSafeInteger(receipt.owned_process_group) || receipt.owned_process_group <= 0
      || !Array.isArray(receipt.cleanup_signals) || receipt.cleanup_signals.length > 3
      || receipt.cleanup_signals.some(signal => !SIGNALS.includes(signal))
      || !Array.isArray(receipt.surviving_owned_process_ids) || receipt.surviving_owned_process_ids.length
      || receipt.cleanup_verified !== true || receipt.prior_or_unrelated_processes_touched !== false) refuse();
  return receipt;
}
function checkedRoot(root) {
  if (typeof root !== 'string' || !path.isAbsolute(root) || fs.realpathSync(root) !== root) refuse();
}
function collect(root, context) {
  const prep = publicPreparation(root, context.public_record_sha256);
  const directory = path.join(root, CAPTURE_DIR);
  if (fs.realpathSync(directory) !== directory) refuse();
  const capture = dataObject(parseCanonical(stableRead(path.join(directory, 'capture.json'), 65536)), CAPTURE_KEYS);
  if (capture.schema !== 1 || capture.status !== 'qa_preparation_failed'
      || capture.stage !== prep.value.stage || capture.repository !== context.repository
      || capture.run_id !== context.run_id || capture.run_attempt !== context.run_attempt
      || capture.source_commit !== context.source_commit || capture.preparation_sha256 !== prep.rawSha256
      || typeof capture.source_snapshot_sha256 !== 'string' || !HEX.test(capture.source_snapshot_sha256)
      || typeof capture.boundaries_sha256 !== 'string' || !HEX.test(capture.boundaries_sha256)) refuse();
  const boundaryBytes = stableRead(path.join(directory, 'boundaries.json'), 65536);
  if (sha256(boundaryBytes) !== capture.boundaries_sha256) refuse();
  const boundaries = dataObject(parseCanonical(boundaryBytes), ['schema', 'children']);
  if (boundaries.schema !== 1) refuse();
  const children = dataObject(boundaries.children, LABELS), records = {};
  const trace = dataObject(capture.traceback, ['total_bytes', 'retained_bytes', 'truncated', 'sha256']);
  const traceback = stableRead(path.join(directory, 'traceback.txt'), 65536);
  if (!Number.isSafeInteger(trace.total_bytes) || !Number.isSafeInteger(trace.retained_bytes)
      || trace.retained_bytes < 0 || trace.total_bytes < trace.retained_bytes
      || trace.retained_bytes !== traceback.length || typeof trace.truncated !== 'boolean'
      || trace.truncated !== (trace.total_bytes > trace.retained_bytes) || trace.sha256 !== sha256(traceback)) refuse();
  noCredentialMarkers(traceback);
  for (const label of LABELS) {
    const state = dataObject(children[label], ['entered']);
    if (typeof state.entered !== 'boolean') refuse();
    const logFile = path.join(root, '.build/hosted-browser', label + '.log');
    const receiptFile = path.join(root, '.build/hosted-browser', label + '-owned.json');
    if (!state.entered) {
      if (fs.lstatSync(logFile, { throwIfNoEntry: false }) !== undefined
          || fs.lstatSync(receiptFile, { throwIfNoEntry: false }) !== undefined) refuse();
      records[label] = { entered: false, log: null, owned: null };
      continue;
    }
    const receiptBytes = stableRead(receiptFile, 16384);
    const receipt = ownedReceipt(receiptBytes, root, label);
    const log = stableRead(logFile, MAX_LOG_SOURCE);
    try {
      noCredentialMarkers(log); // Includes a marker crossing the retained-tail boundary.
      const tail = log.subarray(Math.max(0, log.length - MAX_LOG_TAIL));
      records[label] = { entered: true, owned: receipt,
        log: { original_bytes: log.length, original_sha256: sha256(log), retained_bytes: tail.length,
          retained_sha256: sha256(tail), truncated: tail.length !== log.length, base64: tail.toString('base64') } };
    } finally { log.fill(0); }
  }
  // The existing receipt is a process-group census, never a whole-OS/birth proof.
  const plaintext = encode({ schema: 1, scope: QA_SCOPE, context,
    preparation: prep.value, capture, boundaries, children: records,
    traceback: { ...trace, base64: traceback.toString('base64') },
    cleanup_scope: 'existing-owned-process-group-census' });
  traceback.fill(0);
  if (plaintext.length > MAX_PLAINTEXT) { plaintext.fill(0); refuse(); }
  return plaintext;
}
function recipient(root) {
  const bytes = stableRead(path.resolve(root, '../.ci/qa-preparation-recipient-public.pem'), 16384);
  if (!bytes.subarray(0, 27).equals(Buffer.from('-----BEGIN PUBLIC KEY-----\n'))) refuse();
  const key = createPublicKey(bytes);
  if (!Buffer.from(key.export({ format: 'pem', type: 'spki' })).equals(bytes)
      || recipientFingerprint(key) !== RECIPIENT) refuse();
  return key;
}
export function sealRecordedQaFailure({ root, env, expectedPreparationSha256 }) {
  let plaintext;
  try {
    checkedRoot(root);
    const context = recordedContext(env, expectedPreparationSha256);
    plaintext = collect(root, context);
    const envelope = sealEnvelope(plaintext, recipient(root), context);
    const directory = path.join(root, 'ci-encrypted-evidence');
    fs.mkdirSync(directory, { mode: 0o700 }); // Refuse every stale output generation.
    const output = path.join(root, OUTPUT);
    const fd = fs.openSync(output, fs.constants.O_WRONLY | fs.constants.O_CREAT
      | fs.constants.O_EXCL | fs.constants.O_NOFOLLOW, 0o600);
    try { fs.writeFileSync(fd, envelope); fs.fsyncSync(fd); }
    finally { fs.closeSync(fd); }
    const written = stableRead(output, MAX_ARTIFACT);
    if (!written.equals(envelope)) refuse();
    validateEnvelope(written, context);
    return { sha256: sha256(written), bytes: written.length };
  } catch { refuse(); }
  finally { plaintext?.fill(0); }
}
export function validateRecordedQaEnvelope({ root, env, expectedPreparationSha256, expectedSealedSha256 }) {
  try {
    checkedRoot(root);
    const context = recordedContext(env, expectedPreparationSha256);
    publicPreparation(root, expectedPreparationSha256);
    recipient(root);
    if (typeof expectedSealedSha256 !== 'string' || !HEX.test(expectedSealedSha256)) refuse();
    const bytes = stableRead(path.join(root, OUTPUT), MAX_ARTIFACT);
    if (sha256(bytes) !== expectedSealedSha256) refuse();
    return validateEnvelope(bytes, context);
  } catch { refuse(); }
}
function main() {
  const args = process.argv.slice(2), validate = args[0] === '--validate';
  if (validate) args.shift();
  if (args.length !== (validate ? 4 : 2) || args[0] !== '--expected-preparation-sha256') refuse();
  const options = { root: ROOT, env: process.env, expectedPreparationSha256: args[1] };
  if (validate) {
    if (args[2] !== '--expected-sealed-sha256') refuse();
    validateRecordedQaEnvelope({ ...options, expectedSealedSha256: args[3] });
    return;
  }
  const result = sealRecordedQaFailure(options);
  console.log('sealed_sha256=' + result.sha256);
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); }
  catch { console.error('Sealed QA preparation refused'); process.exitCode = 1; }
}
