// Real short-lived Node processes, no browser/service/network/database.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import vm from 'node:vm';
import { installRunnerLifecycle } from '../../scripts/runner-lifecycle.mjs';

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'runner-lifecycle-seam-'));
const moduleUrl = new URL('../../scripts/runner-lifecycle.mjs', import.meta.url).href;
const test = async (name, action) => { await action(); process.stdout.write('PASS ' + name + '\n'); };
try {
  for (const [label, action, exit, fatal, finalized] of [
    ['normal', "report.passed=true;report.completed_utc='synthetic';stage('complete');", 0, 'none', true],
    ['nonzero', "process.exitCode=7;stage('complete');", 7, 'none', false],
    ['throw-in-close', "stage('closing-browser');setImmediate(()=>{throw Error('private synthetic value');});", 1, 'uncaught-exception', false],
    ['late-rejection', "stage('complete');report.passed=true;report.completed_utc='synthetic';Promise.reject(Error('private synthetic value'));", 1, 'unhandled-rejection', true],
  ]) await test('passive recorder preserves ' + label + ' process exit', () => {
    const file = path.join(root, label + '.json');
    const script = `import fs from 'node:fs';import {installRunnerLifecycle} from ${JSON.stringify(moduleUrl)};const report={steps:[{status:'passed'}]};const stage=installRunnerLifecycle(report,()=>fs.writeFileSync(${JSON.stringify(file)},JSON.stringify(report)));${action}`;
    const child = spawnSync(process.execPath, ['--unhandled-rejections=strict', '--input-type=module', '-e', script], { encoding: 'utf8', timeout: 3000 });
    assert.equal(child.status, exit); assert.equal(child.signal, null);
    const report = JSON.parse(fs.readFileSync(file, 'utf8'));
    assert.equal(report.runner_lifecycle.exit_observed, true); assert.equal(report.runner_lifecycle.exit_code, exit);
    assert.equal(report.runner_lifecycle.fatal_kind, fatal); assert.equal(typeof report.completed_utc === 'string', finalized);
    assert.equal(JSON.stringify(report).includes('private'), false);
    assert.equal(report.runner_lifecycle.fatal_stage, fatal === 'none' ? 'none' : label === 'throw-in-close' ? 'closing-browser' : 'complete');
  });
  await test('recorder only subscribes to passive monitor and exit and tolerates save failure', () => {
    const listeners = {}, report = {};
    const stage = installRunnerLifecycle(report, () => { throw Error('synthetic disk failure'); }, { on: (name, callback) => { listeners[name] = callback; } });
    assert.deepEqual(Object.keys(listeners).sort(), ['exit', 'uncaughtExceptionMonitor']);
    stage('running-checks'); listeners.uncaughtExceptionMonitor(Error('private'), 'unhandledRejection'); listeners.exit(1);
    assert.equal(report.runner_lifecycle.fatal_kind, 'unhandled-rejection');
    assert.throws(() => stage('private unknown stage'), /Unknown fixed runner stage/);
    assert.equal(JSON.stringify(report).includes('private'), false);
  });
  await test('fatal-code recorder copies only the fixed finite allowlist', () => {
    for (const code of ['ECONNRESET','EPIPE','ERR_STREAM_DESTROYED','ERR_SERVER_NOT_RUNNING','private-value',null,{},undefined]) {
      const listeners={},report={};installRunnerLifecycle(report,()=>{}, {on:(key,callback)=>listeners[key]=callback});
      listeners.uncaughtExceptionMonitor(Object.assign(Error('private-value'),{code}),'uncaughtException');
      assert.equal(report.runner_lifecycle.fatal_code,['ECONNRESET','EPIPE','ERR_STREAM_DESTROYED','ERR_SERVER_NOT_RUNNING'].includes(code)?code:'unknown');
      assert.equal(JSON.stringify(report).includes('private-value'),false);
    }
  });
  await test('passive fatal-code extraction reads once and refuses throwing or unstable getters', () => {
    for (const error of [{get code() { throw Error('private'); }}, {get code() { return ++this.reads === 1 ? 'EPIPE' : 'private'; }, reads: 0}]) {
      const listeners = {}, report = {};
      installRunnerLifecycle(report, () => {}, {on: (key, callback) => listeners[key] = callback});
      assert.doesNotThrow(() => listeners.uncaughtExceptionMonitor(error, 'uncaughtException'));
      assert(['EPIPE', 'unknown'].includes(report.runner_lifecycle.fatal_code));
      assert.equal(JSON.stringify(report).includes('private'), false);
      if ('reads' in error) assert.equal(error.reads, 1);
    }
  });
  await test('actual acceptance catch captures the original stage before finally completes', () => {
    const source = fs.readFileSync(new URL('../../scripts/browser-acceptance.mjs', import.meta.url), 'utf8');
    const match = source.match(/\} catch \(error\) \{ ([^\n]+) \}\nfinally \{/);
    assert(match);
    const listeners = {}, report = {steps: []};
    const runnerStage = installRunnerLifecycle(report, () => {}, {on: (key, callback) => listeners[key] = callback});
    vm.runInNewContext(`try { runnerStage('launching-browser'); throw Error('No usable sandbox! private-url'); } catch (error) { ${match[1]} } finally { runnerStage('closing-context'); runnerStage('closing-browser'); runnerStage('closing-proxy'); runnerStage('finalizing-report'); runnerStage('complete'); }`, {report, runnerStage, safeError: () => 'private-only'});
    assert.equal(report.runner_lifecycle.stage, 'complete');
    assert.equal(report.runner_lifecycle.caught_stage, 'launching-browser');
    assert.equal(report.runner_lifecycle.caught_category, 'browser-sandbox-refused');
    assert.equal(report.runner_lifecycle.fatal_kind, 'none');
    assert.equal(JSON.stringify(report.runner_lifecycle).includes('private'), false);
    runnerStage.caught(Error('later failure'));
    assert.equal(report.runner_lifecycle.caught_stage, 'launching-browser');
  });
  await test('caught classifications are stage-bound and never export messages or arbitrary codes', () => {
    for (const [stage, error, steps, expected] of [
      ['initializing', Object.assign(Error('private'), {code: 'EPERM'}), [], 'proxy-bind-permission'],
      ['initializing', Error('No usable sandbox! private'), [], 'proxy-listen-failed'],
      ['launching-browser', Error('private browser launch error'), [], 'browser-launch-failed'],
      ...['No usable sandbox!', 'Failed to move to new namespace:', 'SUID sandbox helper binary', 'Running as root without --no-sandbox is not supported'].map(banner => ['launching-browser', Error(banner + ' private'), [], 'browser-sandbox-refused']),
      ['creating-context', Error('No usable sandbox! private'), [], 'context-setup-failed'],
      ['running-checks', Error('No usable sandbox! private'), [], 'journey-setup-failed'],
      ['running-checks', Error('No usable sandbox! private'), [{status:'failed'}], 'journey-failed'],
      ['closing-browser', Error('No usable sandbox! private'), [], 'harness-failed'],
      ...['public-upload-fixture','public-upload-signer','public-upload-observers'].map(stage => [stage, Error('No usable sandbox! private'), [], 'journey-setup-failed']),
    ]) {
      const report = {steps}; const transition = installRunnerLifecycle(report, () => {}, {on() {}});
      transition(stage); transition.caught(error); transition('complete');
      assert.equal(report.runner_lifecycle.caught_stage, stage);
      assert.equal(report.runner_lifecycle.caught_category, expected);
      assert.equal(JSON.stringify(report.runner_lifecycle).includes('private'), false);
    }
  });
  await test('unreadable or nonstring caught errors stay generic without replacing the original outcome', () => {
    for (const error of [null, undefined, 'private', {message: {private:'value'}, code: 'private'},
      {get message() { throw Error('private'); }}, {get code() { throw Error('private'); }},
      {toString() { throw Error('private'); }}]) {
      for (const stage of ['initializing','launching-browser']) {
        const report = {steps:[]}; const transition = installRunnerLifecycle(report, () => {throw Error('private-save');}, {on() {}});
        transition(stage); assert.doesNotThrow(() => transition.caught(error));
        assert.equal(report.runner_lifecycle.caught_stage, stage);
        assert(['proxy-listen-failed','browser-launch-failed','harness-failed'].includes(report.runner_lifecycle.caught_category));
        assert.equal(JSON.stringify(report.runner_lifecycle).includes('private'), false);
      }
    }
  });
  await test('actual public-upload prefix distinguishes fixture signer and observer setup before any step', async () => {
    const source = fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs', import.meta.url), 'utf8');
    const start = source.indexOf('  const { page, context, fixture, origin, report, step, save, insist, redactions, runnerStage } = h;');
    const end = source.indexOf('  const healthy =', start);
    assert(start > 0 && end > start);
    const body = source.slice(start, end);
    for (const fault of ['fixture', 'signer', 'observers']) {
      const report = {steps: []}, marker = Error('private synthetic preflight refusal');
      const transition = installRunnerLifecycle(report, () => {}, {on() {}});
      const globals = {assert, URL, Buffer,
        async verifiedSchnorr() { if (fault === 'signer') throw marker; return {utils: {randomSecretKey: () => new Uint8Array(32)}, getPublicKey: () => new Uint8Array(32)}; },
        observePublicUpload() { assert.equal(fault, 'observers'); throw marker; },
        createUploadIndependentReader() { assert.fail('Refused observer setup must not create a reader'); },
        uploadDigest() { assert.fail('No HTTP signing occurs in setup'); },
      };
      const prefix = vm.runInNewContext('(async h => {' + body + '})', globals);
      try {
        await prefix({page: {}, context: {}, fixture: {public_upload: {enabled: fault !== 'fixture', fixture_only: true, scenario: 'public-upload-v1'}, mail_origin: 'http://127.0.0.1:1234'}, origin: 'http://127.0.0.1:5678', report, step() {assert.fail('Preflight must not run a step');}, save() {}, insist: assert, redactions: new Set(), runnerStage: transition});
        assert.fail('The actual setup prefix must refuse');
      } catch (error) { transition.caught(error); }
      transition('closing-context'); transition('closing-browser'); transition('complete');
      assert.equal(report.runner_lifecycle.caught_stage, 'public-upload-' + fault);
      assert.equal(report.runner_lifecycle.caught_category, 'journey-setup-failed');
      assert.deepEqual(report.steps, []);
      assert.equal(JSON.stringify(report).includes('private'), false);
    }
    assert.match(source, /try \{\n    runnerStage\('running-checks'\);\n    await run\('provision real bound/);
  });
} finally { fs.rmSync(root, { recursive: true, force: true }); }
