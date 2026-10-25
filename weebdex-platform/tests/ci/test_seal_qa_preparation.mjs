// Synthetic files and the existing PUBLIC recipient only. No key generation,
// private-key access, decryption, build, browser, service or hosted action.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash, createPublicKey } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { MAX_ARTIFACT, MAX_PLAINTEXT, MAX_LOG_TAIL, MAX_LOG_SOURCE, QA_SCOPE, RECIPIENT,
  recipientFingerprint, sealRecordedQaFailure, validateRecordedQaEnvelope,
  validateEnvelope } from '../../scripts/ci/seal-qa-preparation.mjs';

const helper = fileURLToPath(new URL('../../scripts/ci/seal-qa-preparation.mjs', import.meta.url));
const pem = fs.readFileSync(new URL('../../../.ci/qa-preparation-recipient-public.pem', import.meta.url));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sort = value => Array.isArray(value) ? value.map(sort) : value !== null && typeof value === 'object'
  ? Object.fromEntries(Object.keys(value).sort().map(key => [key, sort(value[key])])) : value;
const canonical = value => Buffer.from(JSON.stringify(sort(value)) + '\n');
const labels = ['public-upload-frontend-build', 'public-upload-artifact-record'];
function fixture(action) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'qa-seal-finite-'));
  const root = path.join(directory, 'weebdex-platform');
  const captureDir = path.join(root, '.build/hosted-browser/qa-preparation-failure');
  fs.mkdirSync(captureDir, { recursive: true });
  fs.mkdirSync(path.join(directory, '.ci'));
  fs.writeFileSync(path.join(directory, '.ci/qa-preparation-recipient-public.pem'), pem);
  const env = { GITHUB_REPOSITORY: '9hnm24bzvm-wq/nostr-manga', GITHUB_RUN_ID: '123456',
    GITHUB_RUN_ATTEMPT: '1', GITHUB_SHA: 'a'.repeat(40) };
  const prep = { schema: 1, status: 'preparation_failed', browser_phases_started: false,
    stage: 'public-upload-qa-build', source_files_checked: 676, frontend_preserved: true,
    artifacts_uploaded: false, error_class: 'RuntimeError' };
  fs.mkdirSync(path.join(root, 'ci-evidence'));
  fs.mkdirSync(path.join(root, 'ci-sanitized-evidence'));
  const raw = Buffer.from(JSON.stringify(prep, null, 2) + '\n');
  fs.writeFileSync(path.join(root, 'ci-evidence/preparation.json'), raw);
  fs.writeFileSync(path.join(root, 'ci-sanitized-evidence/preparation.json'), canonical(prep));
  const boundaries = { schema: 1, children: Object.fromEntries(labels.map(label => [label, { entered: false }])) };
  const trace = Buffer.from('Synthetic QA failure; no locals or environment\n');
  fs.writeFileSync(path.join(captureDir, 'traceback.txt'), trace);
  const capture = { schema: 1, status: 'qa_preparation_failed', stage: prep.stage,
    repository: env.GITHUB_REPOSITORY, run_id: env.GITHUB_RUN_ID, run_attempt: env.GITHUB_RUN_ATTEMPT,
    source_commit: env.GITHUB_SHA, preparation_sha256: hash(raw), source_snapshot_sha256: 'b'.repeat(64),
    boundaries_sha256: hash(canonical(boundaries)),
    traceback: { total_bytes: trace.length, retained_bytes: trace.length, truncated: false, sha256: hash(trace) } };
  const options = { root, env, expectedPreparationSha256: hash(canonical(prep)) };
  function refresh() {
    fs.writeFileSync(path.join(captureDir, 'boundaries.json'), canonical(boundaries));
    capture.boundaries_sha256 = hash(canonical(boundaries));
    fs.writeFileSync(path.join(captureDir, 'capture.json'), canonical(capture));
  }
  function child(label = labels[0], overrides = {}, log = Buffer.from('Synthetic build error\n')) {
    boundaries.children[label].entered = true;
    const receipt = { command: label === labels[0]
      ? ['node', path.join(root, 'scripts/build-frontend.mjs')]
      : [path.join(root, '.build/hosted-python/bin/python'), path.join(root, 'scripts/record-frontend-artifacts.py'), 'hosted-public-upload-build/build-result.json'],
      cwd: root, deadline_seconds: label === labels[0] ? 900 : 60, elapsed_seconds: 0.125,
      process_exit_code: 1, deadline_exceeded: false, terminal_receipt: true,
      owned_process_group: 12345, cleanup_signals: [], surviving_owned_process_ids: [],
      cleanup_verified: true, prior_or_unrelated_processes_touched: false, ...overrides };
    let bytes = JSON.stringify(receipt, null, 2) + '\n';
    for (const key of ['deadline_seconds', 'elapsed_seconds']) if (Number.isInteger(receipt[key]))
      bytes = bytes.replace('"' + key + '": ' + receipt[key], '"' + key + '": ' + receipt[key] + '.0');
    fs.writeFileSync(path.join(root, '.build/hosted-browser', label + '-owned.json'), bytes);
    fs.writeFileSync(path.join(root, '.build/hosted-browser', label + '.log'), log);
    refresh();
    return receipt;
  }
  refresh();
  try { return action({ directory, root, captureDir, env, prep, boundaries, capture, options, refresh, child }); }
  finally { fs.rmSync(directory, { recursive: true, force: true }); }
}
const output = root => path.join(root, 'ci-encrypted-evidence/qa-preparation-failure.json');
function seal(f) {
  const result = sealRecordedQaFailure(f.options), bytes = fs.readFileSync(output(f.root));
  assert.equal(hash(bytes), result.sha256);
  assert.ok(bytes.length <= MAX_ARTIFACT);
  const envelope = JSON.parse(bytes);
  assert.equal(envelope.context.scope, QA_SCOPE);
  assert.equal(envelope.context.public_record_sha256, f.options.expectedPreparationSha256);
  assert.equal(envelope.context.recipient_spki_sha256, RECIPIENT);
  assert.equal(validateRecordedQaEnvelope({ ...f.options, expectedSealedSha256: result.sha256 }), true);
  assert.ok(!bytes.includes(Buffer.from('Synthetic build error')));
  assert.equal(fs.statSync(output(f.root)).mode & 0o777, 0o600);
  return { result, bytes, envelope };
}

test('existing public recipient and fixed QA domain/bounds are exact', () => {
  assert.equal(recipientFingerprint(createPublicKey(pem)), RECIPIENT);
  assert.equal(MAX_PLAINTEXT, 2 * 1024 * 1024);
  assert.equal(MAX_ARTIFACT, 4 * 1024 * 1024);
  assert.equal(MAX_LOG_TAIL, 512 * 1024);
  assert.equal(MAX_LOG_SOURCE, 64 * 1024 * 1024);
  assert.notEqual(QA_SCOPE, 'sandbox-preflight-worker-log');
});
test('explicit no-child failure can seal; no fake missing-child cleanup inferred', () => fixture(f => seal(f)));
test('clean failed compilation and successful child before admission failure are both retained', () => {
  for (const code of [0, 1, 2]) fixture(f => { f.child(labels[0], { process_exit_code: code }); seal(f); });
});
test('terminal deadline/signals with known stopped group are admissible honestly', () => fixture(f => {
  f.child(labels[0], { process_exit_code: -15, deadline_exceeded: true,
    cleanup_signals: ['SIGTERM to newly owned process group'] }); seal(f);
}));
test('missing unsafe malformed and unknown owned receipts refuse', () => {
  for (const change of [{ terminal_receipt: false }, { cleanup_verified: false },
    { surviving_owned_process_ids: [111] }, { prior_or_unrelated_processes_touched: true },
    { cleanup_signals: ['private arbitrary signal'] }, { process_exit_code: null },
    { deadline_exceeded: 'true' }, { command: ['cat', '/arbitrary'] }, { extra: 'private' }])
    fixture(f => { f.child(labels[0], change); assert.throws(() => sealRecordedQaFailure(f.options)); });
  fixture(f => { f.boundaries.children[labels[0]].entered = true; f.refresh(); assert.throws(() => sealRecordedQaFailure(f.options)); });
});
test('entered-false stale files including dangling links refuse', () => {
  for (const kind of ['log', 'receipt', 'dangling']) fixture(f => {
    const file = path.join(f.root, '.build/hosted-browser', labels[0] + (kind === 'receipt' ? '-owned.json' : '.log'));
    if (kind === 'dangling') fs.symlinkSync(path.join(f.directory, 'missing'), file);
    else fs.writeFileSync(file, 'stale');
    assert.throws(() => sealRecordedQaFailure(f.options));
  });
});
test('stale run/source/attempt and modified boundary/preparation binding refuse', () => {
  for (const [key, value] of [['run_id', '654321'], ['run_attempt', '2'], ['source_commit', 'c'.repeat(40)],
    ['repository', 'other/repo'], ['boundaries_sha256', '0'.repeat(64)], ['preparation_sha256', '0'.repeat(64)]])
    fixture(f => { f.capture[key] = value; if (key === 'boundaries_sha256') {
      fs.writeFileSync(path.join(f.captureDir, 'capture.json'), canonical(f.capture));
    } else f.refresh(); assert.throws(() => sealRecordedQaFailure(f.options)); });
});
test('canonical duplicate-key and unknown-field capture data refuse', () => {
  for (const kind of ['duplicate', 'unknown', 'pretty']) fixture(f => {
    const p = path.join(f.captureDir, 'capture.json');
    if (kind === 'duplicate') fs.writeFileSync(p, fs.readFileSync(p, 'utf8').replace('"schema":1', '"schema":1,"schema":1'));
    else if (kind === 'unknown') { f.capture.env = { PRIVATE: 'excluded' }; f.refresh(); }
    else fs.writeFileSync(p, JSON.stringify(f.capture, null, 2));
    assert.throws(() => sealRecordedQaFailure(f.options));
  });
});
test('public successful/nonprofile/mismatching preparations cannot be sealed', () => {
  for (const change of [{ status: 'prepared' }, { stage: 'native-dependencies' },
    { browser_phases_started: true }, { error_class: 'arbitrary private error' }]) fixture(f => {
    Object.assign(f.prep, change);
    const raw = Buffer.from(JSON.stringify(f.prep, null, 2) + '\n');
    fs.writeFileSync(path.join(f.root, 'ci-evidence/preparation.json'), raw);
    fs.writeFileSync(path.join(f.root, 'ci-sanitized-evidence/preparation.json'), canonical(f.prep));
    f.capture.preparation_sha256 = hash(raw); f.capture.stage = f.prep.stage; f.options.expectedPreparationSha256 = hash(canonical(f.prep)); f.refresh();
    assert.throws(() => sealRecordedQaFailure(f.options));
  });
});
test('trace tail length/hash/truncation and fixed input links refuse when inconsistent', () => {
  for (const kind of ['hash', 'length', 'truncated', 'symlink']) fixture(f => {
    if (kind === 'hash') f.capture.traceback.sha256 = 'f'.repeat(64);
    else if (kind === 'length') f.capture.traceback.retained_bytes++;
    else if (kind === 'truncated') f.capture.traceback.truncated = true;
    else { const p = path.join(f.captureDir, 'traceback.txt'); fs.renameSync(p, p + '.real'); fs.symlinkSync(p + '.real', p); }
    f.refresh(); assert.throws(() => sealRecordedQaFailure(f.options));
  });
});
test('two truncated binary log tails remain inside serialized/ciphertext caps', () => fixture(f => {
  const bytes = Buffer.alloc(MAX_LOG_TAIL + 1024, 0xff);
  f.child(labels[0], {}, bytes); f.child(labels[1], { process_exit_code: 0 }, bytes);
  const { envelope } = seal(f);
  const plaintextLength = Buffer.from(envelope.ciphertext, 'base64').length;
  assert.ok(plaintextLength > MAX_LOG_TAIL * 2 && plaintextLength <= MAX_PLAINTEXT);
}));
test('oversized source log and public PEM with junk or links refuse', () => {
  fixture(f => {
    f.child(); const target = path.join(f.root, '.build/hosted-browser', labels[0] + '.log');
    const open = fs.openSync, stat = fs.fstatSync, close = fs.closeSync, files = new Map(); let observed = false;
    try {
      fs.openSync = (file, ...args) => { const fd = open(file, ...args); files.set(fd, file); return fd; };
      fs.fstatSync = (fd, ...args) => {
        const value = stat(fd, ...args);
        if (files.get(fd) !== target) return value;
        observed = true; const oversized = Object.create(value); oversized.size = BigInt(MAX_LOG_SOURCE + 1); return oversized;
      };
      fs.closeSync = fd => { files.delete(fd); return close(fd); };
      assert.throws(() => sealRecordedQaFailure(f.options));
      assert.equal(observed, true); // No 64MiB fixture is created or read.
    } finally { fs.openSync = open; fs.fstatSync = stat; fs.closeSync = close; }
  });
  for (const kind of ['junk', 'link']) fixture(f => {
    const p = path.join(f.directory, '.ci/qa-preparation-recipient-public.pem');
    if (kind === 'junk') fs.appendFileSync(p, '\nJUNK');
    else { fs.renameSync(p, p + '.real'); fs.symlinkSync(p + '.real', p); }
    assert.throws(() => sealRecordedQaFailure(f.options));
  });
});
test('fresh output cannot overwrite or bless a stale envelope', () => fixture(f => {
  const { result, bytes, envelope } = seal(f);
  assert.throws(() => sealRecordedQaFailure(f.options));
  assert.deepEqual(fs.readFileSync(output(f.root)), bytes);
  assert.throws(() => validateRecordedQaEnvelope({ ...f.options, expectedSealedSha256: '0'.repeat(64) }));
  assert.throws(() => validateEnvelope(bytes, { ...envelope.context, scope: 'sandbox-preflight-worker-log' }));
  assert.throws(() => validateRecordedQaEnvelope({ ...f.options, env: { ...f.env, GITHUB_RUN_ID: '999' }, expectedSealedSha256: result.sha256 }));
  const changed = Buffer.concat([bytes, Buffer.from(' ')]); fs.writeFileSync(output(f.root), changed);
  assert.throws(() => validateRecordedQaEnvelope({ ...f.options, expectedSealedSha256: hash(changed) }));
}));
test('CLI refuses unknown arguments and never prints their private text', () => {
  const result = spawnSync(process.execPath, [helper, '--arbitrary-file', 'PRIVATE SENTINEL'], { encoding: 'utf8', timeout: 2000 });
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr.trim(), 'Sealed QA preparation refused');
});
test('strong credential markers refuse retained log and trace bytes without redaction', () => {
  const markers = ['-----BEGIN PRIVATE KEY-----', 'nsec1' + 'q'.repeat(58),
    'ghp_' + 'A'.repeat(36), 'github_pat_' + 'A'.repeat(40), 'AKIA' + 'A'.repeat(16),
    'Bearer ' + 'a'.repeat(30), 'Nostr ' + 'a'.repeat(30), 'https://user:password@example.invalid/path',
    'OPENAI_API_KEY=' + 'x'.repeat(30)];
  for (const marker of markers) for (const target of ['log', 'trace']) fixture(f => {
    if (target === 'log') f.child(labels[0], {}, Buffer.from(marker));
    else {
      const bytes = Buffer.from(marker); fs.writeFileSync(path.join(f.captureDir, 'traceback.txt'), bytes);
      f.capture.traceback = { total_bytes: bytes.length, retained_bytes: bytes.length, truncated: false, sha256: hash(bytes) }; f.refresh();
    }
    assert.throws(() => sealRecordedQaFailure(f.options), /^Error: Sealed QA preparation refused$/);
    assert.equal(fs.existsSync(output(f.root)), false);
  });
  fixture(f => { f.child(labels[0], {}, Buffer.from('Reviewed function private_key is unavailable; no values printed.')); seal(f); });
});
test('proven crypto operation and stable-read blocks retain exact source bytes', () => {
  const source = fs.readFileSync(helper, 'utf8');
  const block = (start, end) => source.slice(source.indexOf(start), source.indexOf(end));
  assert.equal(hash(Buffer.from(block('export function sealEnvelope', 'export function openEnvelope').replace('plaintext.length > MAX_PLAINTEXT', 'plaintext.length > MAX_ARTIFACT'))), '1d769498f5f498ebd6db5c63bcc5f33054375ff78c409f70bc80d4ccef7a8921');
  assert.equal(hash(Buffer.from(block('export function openEnvelope', 'function stableRead'))), '9dda48ecc3505aaef440809dbd3a3249b027db1f55e640601e519877d241ac45');
  // Exclude the new blank separator AFTER the unchanged function body.
  assert.equal(hash(Buffer.from(block('function stableRead', 'const LABELS').trimEnd() + '\n')), '021fda2ea1af1c7661fb933ef41ca47a14726626c9c4baa170a184655379670d');
});
test('actual FIFOs at fixed inputs refuse without blocking and hardlinks refuse', () => {
  for (const kind of ['trace', 'owned', 'log', 'pem']) fixture(f => {
    if (kind === 'owned' || kind === 'log') f.child();
    const file = kind === 'trace' ? path.join(f.captureDir, 'traceback.txt')
      : kind === 'pem' ? path.join(f.directory, '.ci/qa-preparation-recipient-public.pem')
        : path.join(f.root, '.build/hosted-browser', labels[0] + (kind === 'owned' ? '-owned.json' : '.log'));
    fs.unlinkSync(file);
    const made = spawnSync('mkfifo', [file], { encoding: 'utf8', timeout: 1000 });
    assert.equal(made.status, 0, 'Existing system mkfifo is required; no fallback or install');
    const code = `import {sealRecordedQaFailure} from ${JSON.stringify(new URL('../../scripts/ci/seal-qa-preparation.mjs',import.meta.url).href)};try{sealRecordedQaFailure(${JSON.stringify(f.options)});process.exitCode=2}catch{process.exitCode=1}`;
    const result = spawnSync(process.execPath, ['--input-type=module', '-e', code], { timeout: 2000, encoding: 'utf8' });
    assert.equal(result.status, 1); assert.equal(result.signal, null); assert.equal(result.stdout, ''); assert.equal(result.stderr, '');
  });
  fixture(f => {
    const file = path.join(f.captureDir, 'traceback.txt'); fs.linkSync(file, file + '.second');
    assert.throws(() => sealRecordedQaFailure(f.options));
  });
});
