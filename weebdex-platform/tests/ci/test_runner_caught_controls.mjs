// Independent classifier controls and two short owned Node children. No browser or network.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { installRunnerLifecycle } from '../../scripts/runner-lifecycle.mjs';

const marker = 'caught-control-secret-marker';
const key = 'ad'.repeat(32);
const url = `https://fixture-secret.invalid/private-path?token=${marker}&key=${key}`;
let cases = 0;
const safe = value => {
  const json = JSON.stringify(value);
  for (const token of [marker, key, url, 'fixture-secret.invalid', '/private-path']) assert(!json.includes(token));
};
const fixture = (steps = []) => {
  const listeners = {}, report = { steps };
  const stage = installRunnerLifecycle(report, () => {}, { on: (name, callback) => { listeners[name] = callback; } });
  return { stage, report, listeners };
};

// A vocabulary hint, changed case, or incomplete banner is insufficient for
// sandbox attribution. A thrown string is not coerced into an Error message.
for (const message of [
  'sandbox', 'sandbox denied EPERM', 'no usable sandbox!', 'No usable sandbox',
  'Failed to move to new namespace', 'SUID sandbox helper',
  'Running as root without --no-sandbox', 'browser sandbox initialization failed',
]) {
  const { stage, report } = fixture();
  stage('launching-browser'); stage.caught(Object.assign(Error(`${message} ${url}`), { name: url, code: key, stack: url }));
  assert.equal(report.runner_lifecycle.caught_category, 'browser-launch-failed');
  safe(report.runner_lifecycle); cases++;
}
for (const error of ['No usable sandbox!', new String('No usable sandbox!'), { message: ['No usable sandbox!'] }]) {
  const { stage, report } = fixture(); stage('launching-browser'); stage.caught(error);
  assert.equal(report.runner_lifecycle.caught_category, 'browser-launch-failed');
  safe(report.runner_lifecycle); cases++;
}

// Exact complete banner text can select a finite label, but only at launch.
for (const message of [
  'No usable sandbox!', 'Failed to move to new namespace:',
  'SUID sandbox helper binary', 'Running as root without --no-sandbox is not supported',
]) {
  for (const [site, expected] of [
    ['launching-browser', 'browser-sandbox-refused'], ['creating-context', 'context-setup-failed'],
    ['public-upload-fixture', 'journey-setup-failed'], ['public-upload-signer', 'journey-setup-failed'],
    ['public-upload-observers', 'journey-setup-failed'], ['running-checks', 'journey-failed'],
    ['closing-context', 'harness-failed'], ['closing-browser', 'harness-failed'],
    ['closing-proxy', 'harness-failed'], ['finalizing-report', 'harness-failed'], ['complete', 'harness-failed'],
  ]) {
    const { stage, report } = fixture([{ status: 'failed' }]);
    stage(site); stage.caught(Error(`${message} ${url}`)); stage('complete');
    assert.equal(report.runner_lifecycle.caught_stage, site);
    assert.equal(report.runner_lifecycle.caught_category, expected);
    safe(report.runner_lifecycle); cases++;
  }
}

// A revoked Proxy and coercion hooks must not replace an existing failure or
// cause the recorder to emit a message, stack, URL or dynamically supplied key.
const revoked = Proxy.revocable({}, {}); revoked.revoke();
let coercions = 0;
for (const error of [
  revoked.proxy,
  new Proxy({}, { get() { throw Error(url); } }),
  { message: { toString() { coercions++; throw Error(url); } } },
  { toString() { coercions++; throw Error(url); } },
]) {
  const { stage, report } = fixture(); stage('launching-browser');
  assert.doesNotThrow(() => stage.caught(error));
  assert(['browser-launch-failed', 'harness-failed'].includes(report.runner_lifecycle.caught_category));
  assert.equal(report.runner_lifecycle.caught_stage, 'launching-browser');
  safe(report.runner_lifecycle); cases++;
}
assert.equal(coercions, 0);

// Invalid stages never enter evidence. The first observed catch survives all
// subsequent transitions, another caught failure and a separate passive fatal.
for (const unknown of [url, 'public-upload-' + marker, '', null, {}, ['launching-browser']]) {
  const { stage, report, listeners } = fixture(); stage('launching-browser');
  assert.throws(() => stage(unknown), error => error.message === 'Unknown fixed runner stage');
  assert.equal(report.runner_lifecycle.stage, 'launching-browser');
  stage.caught(Error(`No usable sandbox! ${url}`)); stage('closing-browser');
  stage.caught(Object.assign(Error(url), { code: 'EPERM' }));
  listeners.uncaughtExceptionMonitor(Object.assign(Error(url), { code: key }), 'uncaughtException');
  listeners.exit(1); stage('complete');
  assert.equal(report.runner_lifecycle.caught_stage, 'launching-browser');
  assert.equal(report.runner_lifecycle.caught_category, 'browser-sandbox-refused');
  assert.equal(report.runner_lifecycle.fatal_stage, 'closing-browser');
  assert.equal(report.runner_lifecycle.fatal_code, 'unknown');
  assert.equal(report.runner_lifecycle.exit_code, 1);
  safe(report.runner_lifecycle); cases++;
}
// Fatal-code classification must read an untrusted property exactly once. A
// time-varying getter must not smuggle a second, unallowlisted URL/key into JSON.
{
  const { report, listeners } = fixture();
  let reads = 0;
  listeners.uncaughtExceptionMonitor({ get code() { return ++reads === 1 ? 'EPIPE' : url; } }, 'uncaughtException');
  assert.equal(reads, 1, 'Fatal-code allowlisting must use one guarded snapshot');
  assert.equal(report.runner_lifecycle.fatal_code, 'EPIPE');
  safe(report.runner_lifecycle); cases++;
}

// A throwing code getter must preserve Node's original fatal exit. Never print
// or retain the private child stderr; only validate its fixed lifecycle fields.
{
  const moduleUrl = new URL('../../scripts/runner-lifecycle.mjs', import.meta.url).href;
  const run = installed => {
    const setup = installed ? `installRunnerLifecycle(report,()=>{});process.on('exit',()=>process.stdout.write(JSON.stringify(report.runner_lifecycle)));` : '';
    const script = `import {installRunnerLifecycle} from ${JSON.stringify(moduleUrl)};const report={};${setup}const error=Error('synthetic original');Object.defineProperty(error,'code',{get(){throw Error('synthetic getter failure');}});setImmediate(()=>{throw error;});`;
    return spawnSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8', timeout: 3000 });
  };
  const control = run(false), observed = run(true);
  assert.equal(control.status, 1); assert.equal(control.signal, null);
  assert.equal(observed.status, control.status, 'Passive observation must not change the fatal exit');
  assert.equal(observed.signal, null);
  const lifecycle = JSON.parse(observed.stdout);
  assert.equal(lifecycle.fatal_kind, 'uncaught-exception');
  assert.equal(lifecycle.fatal_code, 'unknown');
  assert.equal(lifecycle.exit_observed, true); assert.equal(lifecycle.exit_code, 1);
  safe(lifecycle); cases++;
}
console.log(JSON.stringify({ status: 'passed', cases, boundary: 'Independent classifier controls and two <=3s owned Node children; no browser, service, network or package install.' }));
