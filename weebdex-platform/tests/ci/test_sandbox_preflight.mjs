// Finite source and injected-object controls only; no browser/network/install.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
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
  assert.equal(Object.keys(value).length, 11);
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
  assert.doesNotMatch(workflow, /mkdir[^\n]*\.build\/sandbox-preflight/);
  assert.match(workflow, /id: probe/);
  assert.match(workflow, /--validate --expected-sha256 "\$\{\{ steps\.probe\.outputs\.evidence_sha256 \}\}"/);
  assert.equal(workflow.split("steps.probe.outputs.evidence_sha256 != ''").length - 1, 2);
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
