// Finite source contracts only. No browser, fixture, installation or CI starts.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';

const root = new URL('../../../', import.meta.url);
const read = name => fs.readFileSync(new URL(name, root), 'utf8');
const sha = value => createHash('sha256').update(value).digest('hex');
const original = read('.github/workflows/hosted-browser-e2e.yml');
const workflow = read('.github/workflows/hosted-startup-e2e.yml');
const originalSha = '29c61963198b921a2e22419cce67d38a4ebd9c868012701bfff2b0bb87dff994';
const startupCondition = "      github.event_name == 'push' && github.event.repository.private == false && github.repository == '9hnm24bzvm-wq/nostr-manga' && github.ref == 'refs/heads/ci/hosted-startup-e2e'";
const header = `name: Hosted startup CSP acceptance

# Existing native Startup phase from reviewed source 8533f5139aaaa23ae16bc94552269f65b43f7283.
# This exact CI branch cannot trigger the separate public-upload workflow.
on:
  push:
    branches: [ci/hosted-startup-e2e]

permissions:
  contents: read

# Serialize with the existing application workflow; never cancel its run.
concurrency:
  group: hosted-browser-e2e-\${{ github.repository }}
  cancel-in-progress: false

jobs:
`;

function replaceOnce(value, before, after) {
  assert.equal(value.split(before).length - 1, 1, 'Expected one reviewed source boundary');
  return value.replace(before, after);
}

function expectedWrapper(source) {
  assert.equal(sha(source), originalSha, 'Unreviewed parent workflow');
  let job = source.slice(source.indexOf('  browser:\n'), source.indexOf('\n  native-upload-diagnostic:'));
  job = replaceOnce(job, '  browser:\n', '  startup:\n');
  const condition = job.split('\n').find(line => line.startsWith('      github.event.repository.private'));
  job = replaceOnce(job, condition, startupCondition);
  const selector = "SELECTED_BATCH: ${{ inputs.batch || 'public-upload' }}";
  assert.equal(job.split(selector).length - 1, 2);
  job = job.replaceAll(selector, 'SELECTED_BATCH: startup');
  // Native-only preparation cannot enter the gate-on QA build. Its optional
  // encrypted failure upload is deliberately not copied into this wrapper.
  const seal = job.indexOf('      - name: Seal current QA preparation failure diagnostics\n');
  const upload = job.indexOf('      - name: Upload allowlisted sanitized evidence only\n');
  assert(seal > 0 && upload > seal);
  job = job.slice(0, seal) + job.slice(upload);
  job = replaceOnce(job,
    'name: hosted-browser-evidence-${{ github.run_id }}-${{ github.run_attempt }}',
    'name: hosted-startup-evidence-${{ github.run_id }}-${{ github.run_attempt }}');
  return header + job;
}

function admit(value) {
  assert.equal(value, expectedWrapper(original), 'Unreviewed startup wrapper');
}

test('startup wrapper is an exact narrow derivative and preserves the old workflow', () => {
  assert.equal(sha(original), originalSha);
  admit(workflow);
});

test('only the exact new public CI branch can trigger one non-cancelling startup job', () => {
  assert.match(workflow, /on:\n  push:\n    branches: \[ci\/hosted-startup-e2e\]\n/);
  assert(workflow.includes(startupCondition));
  assert.doesNotMatch(workflow, /workflow_dispatch|pull_request|schedule:|workflow_call|workflow_run/);
  assert.equal(workflow.split('\n  startup:\n').length - 1, 1);
  assert.match(workflow, /permissions:\n  contents: read\n/);
  assert.match(workflow, /group: hosted-browser-e2e-\$\{\{ github.repository \}\}\n  cancel-in-progress: false/);
  assert.doesNotMatch(original, /ci\/hosted-startup-e2e/);
});

test('preparation and browser execution select only the existing native startup phase', () => {
  assert.equal(workflow.split('SELECTED_BATCH: startup').length - 1, 2);
  assert.doesNotMatch(workflow, /inputs\.batch|SELECTED_BATCH:.*public-upload/);
  assert.match(workflow, /--batch "\$SELECTED_BATCH" --budget-seconds 3600/);
  const stages = ['prepare-hosted-browser.sh', 'run-hosted-feed-regression.py', 'run-hosted-browser.py', 'validate-ci-evidence.py'];
  const positions = stages.map(stage => workflow.indexOf(stage));
  assert(positions.every(position => position > 0));
  assert.deepEqual(positions, [...positions].sort((a, b) => a - b));
  const runner = read('weebdex-platform/scripts/ci/run-hosted-browser.py');
  assert(runner.includes("batches = {'startup': []} if args.batch == 'startup'"));
  assert(runner.includes("return list(phases) if name in PUBLIC_GATE_BATCHES else ['startup', *phases]"));
  const preparation = read('weebdex-platform/scripts/ci/prepare-hosted-browser.py');
  assert(preparation.includes("PUBLIC_MANGADEX_CATALOGUE='0', WEEBDEX_QA_UPLOAD_REFUSALS='0'"));
  assert(preparation.includes("if os.environ.get('SELECTED_BATCH', 'all') in ['all', 'public-upload', 'public-signed-features']:"));
});

test('hard caps, native prerequisite and complete startup fixture reserve remain bounded', () => {
  const caps = [...workflow.matchAll(/timeout-minutes: (\d+)/g)].map(match => Number(match[1]));
  assert.deepEqual(caps, [140, 2, 90, 12, 30]);
  assert(2 + 90 + 12 + 30 < 140);
  // Existing readiness + startup + bookkeeping + cleanup, not a shortened
  // deadline or a claim that browser execution has already succeeded.
  const reserve = 300 + 150 + 35 + 600;
  assert.equal(reserve, 1085);
  assert(reserve < 30 * 60);
  assert.match(workflow, /CARGO_BUILD_JOBS: 2/);
  assert.match(workflow, /NODE_OPTIONS: --max-old-space-size=2048/);
});

test('official action pins and exact platform versions are unchanged', () => {
  const actions = [...workflow.matchAll(/uses: ([^\n]+)/g)].map(match => match[1]);
  assert.deepEqual(actions, [
    'actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1',
    'actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4.4.0',
    'actions/setup-python@e797f83bcb11b83ae66e0230d6156d7c80228e7c # v6.0.0',
    'actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2',
  ]);
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
  assert.match(workflow, /node-version: 24\.19\.0/);
  assert.match(workflow, /python-version: '3\.12\.15'/);
  assert.match(workflow, /persist-credentials: false/);
  assert.doesNotMatch(workflow.replace('private/paid/self-hosted', ''), /self-hosted|secrets\.|contents: write|continue-on-error|--no-sandbox/);
});

test('only three validated sanitized reports can be uploaded for one day', () => {
  assert.match(workflow, /id: evidence-validation\n        if: always\(\)/);
  assert.match(workflow, /if: always\(\) && steps\.evidence-validation\.outcome == 'success'/);
  const paths = [...workflow.matchAll(/^            (weebdex-platform\/ci-sanitized-evidence\/[^\n]+)$/gm)].map(match => match[1]);
  assert.deepEqual(paths, ['preparation.json', 'summary.json', 'phases.json'].map(name => 'weebdex-platform/ci-sanitized-evidence/' + name));
  assert.match(workflow, /retention-days: 1\n          include-hidden-files: false/);
  assert.doesNotMatch(workflow, /ci-encrypted-evidence|qa-preparation-seal|screenshots|\.png|test-runtime\//);
});

test('all reused source seams retain exact reviewed bytes and normal browser sandbox', () => {
  const pins = {
    'scripts/ci/prepare-hosted-browser.sh': 'a4f1669e1f64fc08be9a8cfd9d3b51069f7ffb41a95d15f499fad2e9a4c41233',
    'scripts/ci/prepare-hosted-browser.py': '3784c7babdc3ac5cb74affd231691c90af661d81b5f702dbbe9500a4ab59db4f',
    'scripts/ci/run-hosted-browser.py': '06f996dcdbcd3b0687aaa83d88a39a82b19ac3a475ab4c3cb213937631932f70',
    'scripts/ci/run-hosted-feed-regression.py': '36277f258fa2801abed15069be0002889be68873174333d4e588239b6119d131',
    'scripts/ci/validate-ci-evidence.py': 'e2d01a31f15cc578e5bb6e3b8a4dbb2273f6448f12764bb6fd675c2cf3ea2628',
    'scripts/ci/browser-runtime.mjs': '061be4487096eda05f3411d2a584cd5971cd62629173ae748a38c0e0e9cd279d',
    'scripts/browser-acceptance.mjs': '91db64c842e8c3093568935f6ee6a01dc20fdd23a448618d7916b5e5c5e7cb3e',
  };
  for (const [name, expected] of Object.entries(pins)) assert.equal(sha(read('weebdex-platform/' + name)), expected, name);
  const acceptance = read('weebdex-platform/scripts/browser-acceptance.mjs');
  assert.match(acceptance, /chromiumSandbox: true/);
  assert.doesNotMatch(acceptance, /--no-sandbox/);
  assert(acceptance.includes('homepage bootstraps under exact generated CSP'));
  assert(acceptance.includes('test-evidence/frontend/bootstrap-csp.json'));
});

test('unauthorized selector, branch, budget, pin, permission and evidence changes fail closed', () => {
  const mutations = [
    ['branches: [ci/hosted-startup-e2e]', 'branches: [main]'],
    ["github.event_name == 'push'", "github.event_name != 'push'"],
    ["github.event.repository.private == false", "github.event.repository.private == true"],
    ["github.repository == '9hnm24bzvm-wq/nostr-manga'", 'true'],
    ['refs/heads/ci/hosted-startup-e2e', 'refs/heads/main'],
    ['SELECTED_BATCH: startup', 'SELECTED_BATCH: all'],
    ['timeout-minutes: 140', 'timeout-minutes: 141'],
    ['timeout-minutes: 90', 'timeout-minutes: 91'],
    ['timeout-minutes: 12', 'timeout-minutes: 13'],
    ['timeout-minutes: 30', 'timeout-minutes: 31'],
    ['--budget-seconds 3600', '--budget-seconds 14400'],
    ['ubuntu-22.04', 'ubuntu-latest'],
    ['contents: read', 'contents: write'],
    ['persist-credentials: false', 'persist-credentials: true'],
    ['cancel-in-progress: false', 'cancel-in-progress: true'],
    ['retention-days: 1', 'retention-days: 90'],
    ['include-hidden-files: false', 'include-hidden-files: true'],
    ['ci-sanitized-evidence/phases.json', 'test-runtime/**'],
  ];
  for (const [before, after] of mutations) {
    assert(workflow.includes(before));
    assert.throws(() => admit(workflow.replace(before, after)), /Unreviewed startup wrapper/);
  }
  assert.throws(() => admit(workflow + '\n  extra-job: {}\n'), /Unreviewed startup wrapper/);
  assert.throws(() => expectedWrapper(original + '\n'), /Unreviewed parent workflow/);
});
