// Finite source and injected-object controls only; no browser/network/install.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash, createPublicKey } from 'node:crypto';
import test from 'node:test';
import { classify, supportedHost, probe } from '../../scripts/ci/sandbox-preflight.mjs';

function fixture(fail = '') {
  const calls = [], snapshots = [];
  const failAt = key => { if (fail === key) throw Error(key === 'launch' ? 'No usable sandbox! PRIVATE-DETAIL' : 'PRIVATE-DETAIL'); };
  const page = { async goto(url, options) { calls.push(['goto', url, options]); failAt('goto'); }, async evaluate(expression) { calls.push(['evaluate', typeof expression]); return fail === 'text' ? 'wrong' : 'sandbox-preflight-ok'; } };
  const context = { async route(pattern, callback) { calls.push(['route', pattern]); await callback({ async abort(code) { calls.push(['abort', code]); } }); }, async newPage() { failAt('page'); return page; }, async close() { calls.push(['context-close']); failAt('context-close'); } };
  const browser = { async newContext(options) { calls.push(['context', options]); failAt('context'); return context; }, async close() { calls.push(['browser-close']); failAt('browser-close'); } };
  const runtime = { executablePath: '/reviewed/synthetic-chromium', chromium: { async launch(options) { calls.push(['launch', options]); failAt('launch'); return browser; } } };
  return { calls, snapshots, runtime, persist: value => snapshots.push(value) };
}
test('supported host is exact Ubuntu22.04 Linuxx64 nonroot', () => {
  const host = 'ID=ubuntu\nVERSION_ID="22.04"\n';
  assert.equal(supportedHost(host, 'linux', 'x64', 1001), true);
  for (const row of [[host, 'linux', 'x64', 0], [host.replace('22.04', '24.04'), 'linux', 'x64', 1001], [host, 'darwin', 'x64', 1001], [host, 'linux', 'arm64', 1001], [host, 'linux', 'x64', true]]) assert.equal(supportedHost(...row), false);
});
test('launch is explicitly sandboxed and data-only with normal context/browser closure', async () => {
  const f = fixture(), value = await probe(f.runtime, f.persist);
  assert.equal(value.completed, true); assert.equal(value.caught_category, 'none');
  assert.deepEqual(f.calls[0], ['launch', { executablePath: '/reviewed/synthetic-chromium', headless: true, chromiumSandbox: true, timeout: 30000 }]);
  assert.deepEqual(f.calls.find(row => row[0] === 'context'), ['context', { serviceWorkers: 'block', acceptDownloads: false }]);
  assert.ok(f.calls.find(row => row[0] === 'goto')[1].startsWith('data:text/html;'));
  assert.deepEqual(f.calls.find(row => row[0] === 'evaluate'), ['evaluate', 'function']);
  assert.deepEqual(f.calls.slice(-2), [['context-close'], ['browser-close']]);
  assert.equal(value.context_closed, true); assert.equal(value.browser_closed, true);
});
for (const [failure, category, stage] of [['launch', 'sandbox-refused', 'launching-browser'], ['context', 'context-failed', 'browser-launched'], ['page', 'page-failed', 'context-created'], ['goto', 'script-failed', 'page-created'], ['text', 'script-failed', 'page-created'], ['context-close', 'cleanup-failed', 'closing-context'], ['browser-close', 'cleanup-failed', 'closing-browser']]) test('first caught failure stays closed: ' + failure, async () => {
  const f = fixture(failure), value = await probe(f.runtime, f.persist);
  assert.equal(value.completed, false); assert.equal(value.stage, 'complete'); assert.equal(value.caught_stage, stage); assert.equal(value.caught_category, category);
  assert.doesNotMatch(JSON.stringify(f.snapshots), /PRIVATE-DETAIL|reviewed\/synthetic|data:text/);
  assert.equal(Object.keys(value).length, 12);
});
test('classifier never leaks raw messages and reads a throwing/changing getter once', () => {
  let reads = 0;
  assert.equal(classify('launching-browser', { get message() { reads++; return reads === 1 ? 'ordinary private error' : 'No usable sandbox!'; } }), 'launch-failed');
  assert.equal(reads, 1);
  assert.equal(classify('launching-browser', { get message() { throw Error('private'); } }), 'launch-failed');
  assert.equal(classify('initializing', Error('No usable sandbox!')), 'worker-failed');
});
test('dedicated workflow never triggers application branches or installs app dependencies', () => {
  const workflow = fs.readFileSync(new URL('../../../.github/workflows/hosted-sandbox-preflight.yml', import.meta.url), 'utf8');
  assert.match(workflow, /branches: \[ci\/browser-sandbox-preflight\]/);
  assert.match(workflow, /runs-on: ubuntu-22\.04/); assert.match(workflow, /timeout-minutes: 20/);
  assert.match(workflow, /persist-credentials: false/); assert.match(workflow, /contents: read/);
  assert.match(workflow, /retention-days: 1/); assert.match(workflow, /ci-evidence\/sandbox-preflight\.json/);
  assert.match(workflow, /run-sandbox-preflight\.py --node "\$\(command -v node\)"/);
  assert.match(workflow, /--cache weebdex-platform\/\.cache\/sandbox-preflight-npm/);
  assert.doesNotMatch(workflow, /mkdir[^\n]*\.build\/sandbox-preflight(?:[\s"']|$)/m);
  assert.match(workflow, /id: probe/);
  assert.match(workflow, /--validate --expected-sha256 "\$\{\{ steps\.probe\.outputs\.evidence_sha256 \}\}"/);
  assert.equal(workflow.split("steps.probe.outputs.evidence_sha256 != ''").length - 1, 3);
  assert.match(workflow, /\^evidence_sha256=\[0-9a-f\]\{64\}\$/);
  const executableWorkflow = workflow.split('\n').filter(line => !line.trimStart().startsWith('#')).join('\n');
  assert.doesNotMatch(executableWorkflow, /workflow_dispatch|prepare-hosted-browser\.sh|cargo |pip |yarn install|self-hosted|sysctl|apparmor|--no-sandbox/);
});
test('application workflow remains exact and does not match the dedicated preflight branch', () => {
  const workflow = fs.readFileSync(new URL('../../../.github/workflows/hosted-browser-e2e.yml', import.meta.url));
  // Two separately approved source profiles; neither is rewritten by this probe.
  const profiles = new Set([
    '19f3cada257be951ee397ef5a10830efb5af0c6033c1978bbdbc3cd8f54951c1', // bounded CI
    'df7481090327214e528ec4d8025d5d05212cbdf7f035294677451e74a59c2c27', // recovery c6
  ]);
  const digest = bytes => createHash('sha256').update(bytes).digest('hex');
  assert.ok(profiles.has(digest(workflow)));
  assert.equal(profiles.has(digest(Buffer.concat([workflow, Buffer.from('\n# unreviewed\n')]))), false);
  assert.doesNotMatch(workflow.toString('utf8'), /ci\/browser-sandbox-preflight/);
});

// These are finite classifier/probe controls, not a real Chromium launch.
test('official rewritten sandbox banner and all original banners stay stage-bound', () => {
  for (const banner of ['Chromium sandboxing failed!', 'No usable sandbox!',
    'Failed to move to new namespace:', 'SUID sandbox helper binary',
    'Running as root without --no-sandbox is not supported']) {
    assert.equal(classify('launching-browser', Error('Fixed wrapper\n' + banner + '\nPRIVATE-SUFFIX')), 'sandbox-refused');
  }
});
test('rewrite classification refuses near-miss and untrusted message shapes', () => {
  for (const message of ['chromium sandboxing failed!', 'Chromium sandboxing failed',
    'Chromium sandbox failed!', 'Chromium sandboxing FAILED!', '', null, 1, true, {},
    { toString() { throw Error('PRIVATE'); } }]) {
    assert.equal(classify('launching-browser', { message }), 'launch-failed');
  }
  let reads = 0;
  assert.equal(classify('launching-browser', { get message() { reads++; return reads === 1 ? 'Chromium sandboxing failed!' : 'PRIVATE'; } }), 'sandbox-refused');
  assert.equal(reads, 1);
  assert.equal(classify('launching-browser', { get message() { throw Error('PRIVATE'); } }), 'launch-failed');
});
test('rewrite classification never attributes a later failure to launch sandbox', () => {
  for (const [stage, expected] of [['initializing', 'worker-failed'], ['browser-launched', 'context-failed'],
    ['context-created', 'page-failed'], ['page-created', 'script-failed'],
    ['closing-context', 'cleanup-failed'], ['closing-browser', 'cleanup-failed'],
    ['complete', 'worker-failed'], ['unknown-private-stage', 'worker-failed']]) {
    assert.equal(classify(stage, Error('Chromium sandboxing failed!')), expected);
  }
});
test('official rewritten launch failure preserves closed catch facts through cleanup', async () => {
  const f = fixture();
  f.runtime.chromium.launch = async options => {
    f.calls.push(['launch', options]);
    throw Error('Failed to launch the browser process.\nBrowser logs:\nChromium sandboxing failed!\nPRIVATE-SUFFIX');
  };
  const value = await probe(f.runtime, f.persist);
  assert.equal(value.stage, 'complete'); assert.equal(value.caught_stage, 'launching-browser');
  assert.equal(value.caught_category, 'sandbox-refused'); assert.equal(value.completed, false);
  for (const field of ['browser_created', 'context_created', 'page_created', 'script_verified', 'context_closed', 'browser_closed']) assert.equal(value[field], false);
  assert.equal(f.calls.length, 1); assert.equal(f.calls[0][1].chromiumSandbox, true);
  assert.equal(Object.keys(value).length, 12);
  assert.doesNotMatch(JSON.stringify(f.snapshots), /PRIVATE-SUFFIX|Failed to launch|Browser logs|Chromium sandboxing/);
});

test('sealed debug export is fresh, recipient-bound and restricted to one encrypted file', () => {
  const workflow = fs.readFileSync(new URL('../../../.github/workflows/hosted-sandbox-preflight.yml', import.meta.url), 'utf8');
  const fingerprint = workflow.match(/^      SEALED_RECIPIENT_SPKI_SHA256: ([0-9a-f]{64})$/m)?.[1];
  assert.ok(fingerprint);
  const pem = fs.readFileSync(new URL('../../../.ci/sandbox-preflight-recipient-public.pem', import.meta.url));
  const key = createPublicKey(pem);
  assert.equal(key.asymmetricKeyType, 'rsa');
  assert.equal(key.asymmetricKeyDetails.modulusLength, 3072);
  assert.equal(key.asymmetricKeyDetails.publicExponent, 65537n);
  assert.deepEqual(Buffer.from(key.export({ type: 'spki', format: 'pem' })), pem);
  assert.equal(createHash('sha256').update(key.export({ type: 'spki', format: 'der' })).digest('hex'), fingerprint);
  assert.doesNotMatch(pem.toString('ascii'), /PRIVATE KEY/);
  assert.match(workflow, /install -m 600 \.ci\/sandbox-preflight-recipient-public\.pem "\$recipient_dir\/public\.pem"/);
  const paths = Array.from(workflow.matchAll(/^          path: (.+)$/gm), match => match[1]);
  assert.deepEqual(paths, ['weebdex-platform/ci-evidence/sandbox-preflight.json', 'weebdex-platform/.build/sandbox-preflight/sealed-worker-log.json']);
  assert.equal(workflow.match(/retention-days: 1/g)?.length, 2);
  assert.equal(workflow.match(/include-hidden-files: true/g)?.length, 1);
  assert.match(workflow, /id: sealed-log/);
  assert.match(workflow, /--expected-recipient-sha256 "\$SEALED_RECIPIENT_SPKI_SHA256"/);
  assert.match(workflow, /--expected-sealed-sha256 "\$\{\{ steps\.sealed-log\.outputs\.sealed_sha256 \}\}"/);
  assert.match(workflow, /steps\.sealed-validation\.outcome == 'success' && steps\.sealed-log\.outputs\.sealed_sha256 != ''/);
  const sealStep = workflow.split('      - name: Seal the bounded private worker log for this invocation\n')[1]?.split('      - name: Validate the current encrypted envelope without opening it\n')[0];
  assert.ok(sealStep);
  assert.match(sealStep, /\^sealed_sha256=\[0-9a-f\]\{64\}\$/);
  assert.doesNotMatch(sealStep, /printf[^\n]*"\$report"\s*$/m);
  assert.doesNotMatch(paths.join('\n'), /worker\.log|owner\.json|seal-ready|private|\.pem|\*/);
  assert.match(workflow, /timeout-minutes: 20/);
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
});
