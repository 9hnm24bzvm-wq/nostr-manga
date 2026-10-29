import {restoreReaderUxPredecessor} from './reviewed-six-persona-source-successor.mjs';
import { restoreUploadFinalization } from './upload-finalization-source-association.mjs';
import { restoreCancellationObserver } from './upload-cancel-observer-association.mjs';
// Finite source and injected filesystem tests. No package import or browser.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import test from 'node:test';
import { runtimeProfile } from '../../scripts/ci/browser-runtime.mjs';
import { restoreChrome157Evidence } from './chrome157-source-association.mjs';

const root = new URL('../../../', import.meta.url);
const read = file => fs.readFileSync(new URL(file, root), 'utf8');
const sha = value => createHash('sha256').update(value).digest('hex');
const runtimeUrl = new URL('../../scripts/ci/browser-runtime.mjs', import.meta.url);
const runtimeSource = fs.readFileSync(runtimeUrl, 'utf8');
const workflow = read('.github/workflows/hosted-browser-chrome157.yml');

test('profile selection is closed and defaults to the unchanged bundled runtime', () => {
  assert.equal(runtimeProfile(), 'bundled');
  for (const value of ['bundled', 'cft-157']) assert.equal(runtimeProfile(value), value);
  for (const value of ['', null, false, {}, 'latest', 'cft-156', '/tmp/browser']) assert.throws(() => runtimeProfile(value));
});

async function isolatedRuntime(record, environment) {
  let imports = 0, verifications = 0;
  const context = vm.createContext({ process: { argv: [], env: environment } });
  const dependencies = {
    'node:fs': { default: { existsSync: () => record !== undefined, realpathSync: name => name,
      lstatSync: () => ({ isFile: () => true }), readFileSync: () => Buffer.from(JSON.stringify(record)) } },
    'node:path': { default: path }, 'node:crypto': { createHash },
    'node:module': { createRequire: () => () => { imports++; throw Error('Package must not load'); } },
    'node:url': { fileURLToPath },
    'node:child_process': { execFileSync: () => { verifications++; throw Error('Synthetic integrity refusal'); } },
  };
  const module = new vm.SourceTextModule(runtimeSource, { context, initializeImportMeta(meta) { meta.url = runtimeUrl.href; } });
  await module.link(name => {
    const values = dependencies[name]; assert(values, name);
    return new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context });
  });
  await module.evaluate();
  return { load: module.namespace.loadBrowserRuntime, imports: () => imports, verifications: () => verifications };
}

for (const [label, record, environment, reason] of [
  ['missing explicit runtime', undefined, { BROWSER_RUNTIME_PROFILE: 'cft-157' }, /unavailable/],
  ['unknown recorded runtime', { profile: 'latest' }, {}, /Unreviewed/],
  ['qualified record with bundled request', { profile: 'cft-157' }, { BROWSER_RUNTIME_PROFILE: 'bundled' }, /changed/],
  ['legacy record with qualified request', {}, { BROWSER_RUNTIME_PROFILE: 'cft-157' }, /changed/],
  ['qualified integrity failure', { profile: 'cft-157' }, {}, /Synthetic integrity refusal/],
]) test(label + ' fails without package import or fallback', async () => {
  const runtime = await isolatedRuntime(record, environment);
  assert.throws(() => runtime.load(), reason);
  assert.equal(runtime.imports(), 0);
  assert.equal(runtime.verifications(), label === 'qualified integrity failure' ? 1 : 0);
});

test('dedicated wrapper is an exact fixed-profile derivative of unchanged actual-app job', () => {
  const source = read('.github/workflows/hosted-browser-e2e.yml');
  assert.equal(sha(source), '29c61963198b921a2e22419cce67d38a4ebd9c868012701bfff2b0bb87dff994');
  let job = source.slice(source.indexOf('  browser:\n'), source.indexOf('\n  native-upload-diagnostic:'));
  const condition = job.split('\n').find(line => line.startsWith('      github.event.repository.private'));
  job = job.replace(condition, "      github.event_name == 'push' && github.event.repository.private == false && github.repository == '9hnm24bzvm-wq/nostr-manga' && github.ref == 'refs/heads/ci/hosted-browser-chrome157'");
  job = job.replace("      CI: 'true'", "      CI: 'true'\n      BROWSER_RUNTIME_PROFILE: cft-157");
  job = job.replaceAll("SELECTED_BATCH: ${{ inputs.batch || 'public-upload' }}", 'SELECTED_BATCH: public-upload');
  job = job.replace('name: hosted-browser-evidence-', 'name: hosted-chrome157-evidence-');
  const header = `name: Qualified Chrome 157 application acceptance

# Explicit opt-in only. Existing application/startup branches cannot match.
on:
  push:
    branches: [ci/hosted-browser-chrome157]
permissions:
  contents: read
concurrency:
  group: hosted-browser-e2e-\${{ github.repository }}
  cancel-in-progress: false
jobs:
`;
  assert.equal(workflow, header + job);
  assert.equal(workflow.split('SELECTED_BATCH: public-upload').length - 1, 2);
  assert.doesNotMatch(workflow, /workflow_dispatch|continue-on-error|--no-sandbox|secrets\.|contents: write/);
  assert.equal(sha(read('.github/workflows/hosted-startup-e2e.yml')), '790b23f84cdf27a98aebe0a22114ef92437437a30831db1a09c65755d08f2f6b');
});

test('reused qualified acquisition, driver and native acceptance have exact unchanged bytes', () => {
  const pins = {
    '.ci/native-upload-counterfactual/package-lock.json': 'e1da6bd8b8a67849c5e55d257336524a6b9fe4c1764c112efb4e8817ea4f3f35',
    'weebdex-platform/scripts/ci/acquire-native-upload-browsers.mjs': '205501db557e41c507c88169a34ac947e30ced6d0dbf10ade1258c7de39dcf38',
    'weebdex-platform/scripts/ci/extract-native-upload-browsers.py': 'd5e0db783a2eacbaf242a8db3414f81e5d6f645cf63d440b6d7bd1c34d868979',
    'weebdex-platform/scripts/browser-acceptance.mjs': '7238ae50b559c54adacb61c95d17fb852ffe8b1ea9fb7a35a6fead0bd00eed8a',
    'weebdex-platform/scripts/public-upload-recovery-browser.mjs': '88f4ecc94ecd31cd4cb6c033e4be22d398c39f538e6b021b798b9baa00d2d95a',
  };
  for (const [file, expected] of Object.entries(pins)) assert.equal(sha(file.endsWith('/public-upload-recovery-browser.mjs') ? restoreUploadFinalization(restoreReaderUxPredecessor(file,read(file))) : read(file)), expected, file);
  const freeze = JSON.parse(read('.ci/native-upload-counterfactual/source-freeze-current.json'));
  const observer = freeze.files.find(row => row.path.endsWith('/public-upload-recovery-observer.mjs'));
  assert(observer); assert.equal(sha(restoreCancellationObserver(read(observer.path))), observer.sha256);
});

test('qualified evidence changes only one exact runtime admission expression', () => {
  const source = read('weebdex-platform/scripts/ci/validate-ci-evidence.py');
  const restored = restoreChrome157Evidence(source);
  assert.equal(sha(restored), 'e2d01a31f15cc578e5bb6e3b8a4dbb2273f6448f12764bb6fd675c2cf3ea2628');
  for (const changed of [source + '\n', source.replace('cft-157.0.8103.6', 'latest'), source.replace('len(value)', '0')]) {
    assert.notEqual(sha(restoreChrome157Evidence(changed)), sha(restored));
  }
});
