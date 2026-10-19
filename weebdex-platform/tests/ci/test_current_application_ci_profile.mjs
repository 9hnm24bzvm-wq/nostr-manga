import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';

const root = new URL('../../../', import.meta.url);
const workflow = fs.readFileSync(new URL('.github/workflows/hosted-browser-e2e.yml', root), 'utf8');
const hash = value => createHash('sha256').update(value).digest('hex');
const currentDigest = '4a62a482ec907923bb830a41f3524e21f7f23ac6868aedac76caed483feffa3e';
const priorCiDigest = '876f174a31c346e17efdbec045e5f00377c2cf3fae966eac5445b4858754f6a1';
const sealedQaBlocks = ['        id: preparation\n', "      - name: Seal current QA preparation failure diagnostics\n        id: qa-preparation-seal\n        if: always() && steps.preparation.outcome == 'failure' && steps.evidence-validation.outcome == 'success' && hashFiles('weebdex-platform/.build/hosted-browser/qa-preparation-failure/capture.json') != ''\n        timeout-minutes: 2\n        run: |\n          set -euo pipefail\n          PREP_SHA=\"$(python3 -c 'import hashlib; from pathlib import Path; print(hashlib.sha256((Path(\"weebdex-platform/ci-sanitized-evidence\") / \"preparation.json\").read_bytes()).hexdigest())')\"\n          node weebdex-platform/scripts/ci/seal-qa-preparation.mjs --expected-preparation-sha256 \"$PREP_SHA\"\n          SEALED_SHA=\"$(python3 -c 'import hashlib; from pathlib import Path; print(hashlib.sha256(Path(\"weebdex-platform/ci-encrypted-evidence/qa-preparation-failure.json\").read_bytes()).hexdigest())')\"\n          node weebdex-platform/scripts/ci/seal-qa-preparation.mjs --validate --expected-preparation-sha256 \"$PREP_SHA\" --expected-sealed-sha256 \"$SEALED_SHA\"\n          printf 'sealed_sha256=%s\\n' \"$SEALED_SHA\" >> \"$GITHUB_OUTPUT\"\n      - name: Upload only the validated encrypted QA preparation failure\n        if: always() && steps.qa-preparation-seal.outcome == 'success' && steps.qa-preparation-seal.outputs.sealed_sha256 != ''\n        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2\n        with:\n          name: hosted-qa-preparation-sealed-${{ github.run_id }}-${{ github.run_attempt }}\n          path: weebdex-platform/ci-encrypted-evidence/qa-preparation-failure.json\n          if-no-files-found: error\n          retention-days: 1\n          include-hidden-files: false\n"];
const recoveryDigest = 'df7481090327214e528ec4d8025d5d05212cbdf7f035294677451e74a59c2c27';
const setupPython = "      - uses: actions/setup-python@e797f83bcb11b83ae66e0230d6156d7c80228e7c # v6.0.0\n        with:\n          python-version: '3.12.15'\n          architecture: x64\n          check-latest: false\n";
const changes = [
  ['default: public-upload', 'default: all', 1],
  ["inputs.batch || 'public-upload'", "inputs.batch || 'all'", 2],
  ['timeout-minutes: 140', 'timeout-minutes: 345', 1],
  ['timeout-minutes: 30', 'timeout-minutes: 245', 1],
  ['--budget-seconds 3600', '--budget-seconds 14400', 1],
  ['runs-on: ubuntu-22.04', 'runs-on: ubuntu-24.04', 1],
  [setupPython, '', 1],
];
function restored(value) {
  assert.equal(hash(value), currentDigest, 'Unreviewed application workflow bytes');
  for (const block of sealedQaBlocks) {
    assert.equal(value.split(block).length - 1, 1);
    value = value.replace(block, '');
  }
  assert.equal(hash(value), priorCiDigest);
  for (const [current, previous, count] of changes) {
    assert.equal(value.split(current).length - 1, count);
    value = value.replaceAll(current, previous);
  }
  assert.equal(hash(value), recoveryDigest);
  return value;
}
test('current application profile is an exact bounded successor of immutable recovery workflow', () => {
  assert.equal(hash(workflow), currentDigest);
  restored(workflow);
  for (const [before, after] of [["inputs.batch || 'public-upload'", "inputs.batch || 'all'"], ['--budget-seconds 3600', '--budget-seconds 3601'], ['python-version: \'3.12.15\'', 'python-version: \'3.12\''], ['ubuntu-22.04', 'ubuntu-latest']]) {
    assert.throws(() => restored(workflow.replace(before, after)), /Unreviewed/);
  }
});
test('one admitted branch push selects precisely public-upload in both preparation and execution', () => {
  assert.match(workflow, /branches: \[ci\/hosted-browser-e2e, ci\/browser-failure-diagnostics\]/);
  assert.equal(workflow.split("SELECTED_BATCH: ${{ inputs.batch || 'public-upload' }}").length - 1, 2);
  assert.match(workflow, /github\.event\.repository\.private == false/);
  assert.doesNotMatch(workflow, /ci\/browser-sandbox-preflight/);
});
test('known supported host and exact Python ABI are explicitly provisioned before preparation', () => {
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
  assert.equal(workflow.split(setupPython).length - 1, 1);
  assert(workflow.indexOf(setupPython) < workflow.indexOf('prepare-hosted-browser.sh'));
  assert.match(workflow, /node-version: 24\.19\.0/);
  const preparation = fs.readFileSync(new URL('weebdex-platform/scripts/ci/prepare-hosted-browser.sh', root), 'utf8');
  assert.match(preparation, /sys\.version_info\[:2\] == \(3,12\)/);
  assert.match(preparation, /--require-hashes --no-deps/);
});
test('hard limits admit the complete single-fixture reservation without widening phase deadlines', () => {
  assert.match(workflow, /timeout-minutes: 140/);
  assert.match(workflow, /timeout-minutes: 90/);
  assert.match(workflow, /timeout-minutes: 12/);
  assert.match(workflow, /timeout-minutes: 30/);
  assert.match(workflow, /--budget-seconds 3600/);
  const phaseReserve = 600 + 35 + 300 + 600;
  assert.equal(phaseReserve, 1535);
  assert(phaseReserve < 30 * 60);
  assert(90 + 12 + 30 < 140);
});
test('native gate and sandboxed network-fenced application launcher remain prerequisites', () => {
  assert(workflow.indexOf('prepare-hosted-browser.sh') < workflow.indexOf('run-hosted-feed-regression.py'));
  assert(workflow.indexOf('run-hosted-feed-regression.py') < workflow.indexOf('Run genuine browser batches'));
  const acceptance = fs.readFileSync(new URL('weebdex-platform/scripts/browser-acceptance.mjs', root), 'utf8');
  assert.match(acceptance, /chromiumSandbox: true/);
  assert.match(acceptance, /Outside exact local fixture/);
  assert.doesNotMatch(acceptance, /--no-sandbox/);
  const runner = fs.readFileSync(new URL('weebdex-platform/scripts/ci/run-hosted-browser.py', root), 'utf8');
  assert.match(runner, /'public-upload': \[PUBLIC_UPLOAD_PHASE\]/);
  assert.match(runner, /set\(ALL_PHASES\) <= passed/);
});
test('three sanitized reports and one failure-only encrypted QA artifact have exact one-day allowlists', () => {
  assert.match(workflow, /contents: read/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /steps\.evidence-validation\.outcome == 'success'/);
  assert.match(workflow, /retention-days: 1/);
  const paths = [...workflow.matchAll(/weebdex-platform\/ci-sanitized-evidence\/[a-z]+\.json/g)].map(row => row[0]);
  assert.deepEqual(paths, ['preparation', 'summary', 'phases'].map(name => `weebdex-platform/ci-sanitized-evidence/${name}.json`));
  assert.doesNotMatch(workflow, /secrets\.|worker\.log|sealed-worker|test-runtime\//);
  assert.equal(workflow.split('path: weebdex-platform/ci-encrypted-evidence/qa-preparation-failure.json').length - 1, 1);
  assert.match(workflow, /steps\.preparation\.outcome == 'failure'/);
  assert.match(workflow, /steps\.qa-preparation-seal\.outcome == 'success'/);
  assert.match(workflow, /--validate --expected-preparation-sha256/);
  assert.match(workflow, /--expected-sealed-sha256/);
  assert.doesNotMatch(workflow, /path:.*(?:traceback|boundaries|capture)\.(?:txt|json)/);
});
