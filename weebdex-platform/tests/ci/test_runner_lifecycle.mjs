// Real short-lived Node processes, no browser/service/network/database.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { installRunnerLifecycle } from '../../scripts/runner-lifecycle.mjs';

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'runner-lifecycle-seam-'));
const moduleUrl = new URL('../../scripts/runner-lifecycle.mjs', import.meta.url).href;
const test = (name, action) => { action(); process.stdout.write('PASS ' + name + '\n'); };
try {
  for (const [label, action, exit, fatal, finalized] of [
    ['normal', "report.passed=true;report.completed_utc='synthetic';stage('complete');", 0, 'none', true],
    ['nonzero', "process.exitCode=7;stage('complete');", 7, 'none', false],
    ['throw-in-close', "stage('closing-browser');setImmediate(()=>{throw Error('private synthetic value');});", 1, 'uncaught-exception', false],
    ['late-rejection', "stage('complete');report.passed=true;report.completed_utc='synthetic';Promise.reject(Error('private synthetic value'));", 1, 'unhandled-rejection', true],
  ]) test('passive recorder preserves ' + label + ' process exit', () => {
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
  test('recorder only subscribes to passive monitor and exit and tolerates save failure', () => {
    const listeners = {}, report = {};
    const stage = installRunnerLifecycle(report, () => { throw Error('synthetic disk failure'); }, { on: (name, callback) => { listeners[name] = callback; } });
    assert.deepEqual(Object.keys(listeners).sort(), ['exit', 'uncaughtExceptionMonitor']);
    stage('running-checks'); listeners.uncaughtExceptionMonitor(Error('private'), 'unhandledRejection'); listeners.exit(1);
    assert.equal(report.runner_lifecycle.fatal_kind, 'unhandled-rejection');
    assert.throws(() => stage('private unknown stage'), /Unknown fixed runner stage/);
    assert.equal(JSON.stringify(report).includes('private'), false);
  });
} finally { fs.rmSync(root, { recursive: true, force: true }); }
