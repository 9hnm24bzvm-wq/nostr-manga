// Reverse only exact reviewed successors. Historical bytes remain immutable.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const hash = value => createHash('sha256').update(value).digest('hex');
const prior = "4a62a482ec907923bb830a41f3524e21f7f23ac6868aedac76caed483feffa3e";
const current = "66a3f9577f49c8f92a9b6cba5ab8bb76497bb5624157d363a5f052f1898990d9";
// Commit 2e506bd845ad775ca776d45acece1047cf27be5e only adds the
// selectable signed-feature batch and updates its descriptive phase count.
const signedFeatureSuccessor = "76d40e0370cdb7c5fb57ae9b4380c39252ff820c1ff2d3dc0a6f0c15fca1d135";
const signedFeatureChanges = [
  ["        description: Bounded batch, or all 42 phases with separate native and public profiles\n", "        description: Bounded batch, or all 40 phases with separate native and public profiles\n"],
  ["        options: [all, startup, ordinary-a, ordinary-b, ui10, signed-a, signed-b, signed-c, signed-d, ui-feed, ui-reader, ui-home, actor-follow, public-upload, public-signed-features]\n", "        options: [all, startup, ordinary-a, ordinary-b, ui10, signed-a, signed-b, signed-c, signed-d, ui-feed, ui-reader, ui-home, actor-follow, public-upload]\n"],
];
export function restoreSignedFeatureWorkflow(input) {
  if (hash(input) === current) return input;
  assert.equal(hash(input), signedFeatureSuccessor, 'Unreviewed signed-feature workflow bytes');
  for (const [successor, previous] of signedFeatureChanges) {
    assert.equal(input.split(successor).length - 1, 1);
    assert.equal(input.split(previous).length - 1, 0);
    input = input.replace(successor, previous);
  }
  assert.equal(hash(input), current);
  return input;
}
const job = "\n  native-upload-diagnostic:\n    # One exact approved publication selects this job instead of the combined suite.\n    if: >-\n      github.event_name == 'push' && github.event.repository.private == false && github.repository == '9hnm24bzvm-wq/nostr-manga' && github.ref == 'refs/heads/ci/browser-failure-diagnostics' && github.event.head_commit.message == 'diagnostic(native-upload): approved bounded run 20261020'\n    runs-on: ubuntu-22.04\n    timeout-minutes: 15\n    env:\n      CI: 'true'\n      PYTHONDONTWRITEBYTECODE: '1'\n      NODE_OPTIONS: --max-old-space-size=512\n      PLAYWRIGHT_BROWSERS_PATH: ${{ github.workspace }}/.ci/native-diagnostic-browsers\n    steps:\n      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1\n        with:\n          persist-credentials: false\n          fetch-depth: 2\n      - name: Admit only the exact approved predecessor and file scope\n        timeout-minutes: 1\n        run: python3 -B weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py preflight\n      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4.4.0\n        with:\n          node-version: 24.19.0\n      - name: Prepare exact official browser tools\n        timeout-minutes: 7\n        run: |\n          mkdir -p weebdex-platform/.build/native-upload-diagnostic\n          if ! (npm ci --ignore-scripts --registry=https://registry.npmjs.org --no-audit --no-fund --omit=optional --prefix .ci/browser && node .ci/browser/node_modules/playwright/cli.js install chromium) >weebdex-platform/.build/native-upload-diagnostic/private-preparation.log 2>&1; then\n            echo 'Dedicated browser preparation failed.'\n            exit 1\n          fi\n      - name: Run six bounded adapter-versus-fetch comparisons with normal Chromium sandbox\n        id: diagnostic\n        timeout-minutes: 5\n        run: python3 -B weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py run\n      - name: Validate one closed sanitized diagnostic report\n        id: diagnostic-validation\n        if: always() && steps.diagnostic.outcome != 'skipped'\n        timeout-minutes: 1\n        run: |\n          python3 -B weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py verify-owner\n          mkdir -p weebdex-platform/ci-native-upload-evidence\n          node weebdex-platform/scripts/ci/native-upload-diagnostic-report.mjs --input weebdex-platform/.build/native-upload-diagnostic/report.json --output weebdex-platform/ci-native-upload-evidence/report.json\n      - name: Upload only the bounded validated diagnostic\n        if: always() && steps.diagnostic-validation.outcome == 'success'\n        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2\n        with:\n          name: native-upload-diagnostic-${{ github.run_id }}-${{ github.run_attempt }}\n          path: weebdex-platform/ci-native-upload-evidence/report.json\n          if-no-files-found: error\n          retention-days: 1\n          include-hidden-files: false\n";
const skip = " && !(github.event_name == 'push' && github.ref == 'refs/heads/ci/browser-failure-diagnostics' && github.event.head_commit.message == 'diagnostic(native-upload): approved bounded run 20261020')";
// Public retained-workflow branch. This only normalizes immutable source in tests;
// it never writes YAML, changes a gate or asserts that a historical run passed.
const retainedPublic = "29c61963198b921a2e22419cce67d38a4ebd9c868012701bfff2b0bb87dff994";
const retainedPipePredecessor = "d0d7b131121f4fbb52714d8fc7661646db4eadf0dad3e1639f1af76cb6074705";
const retainedMarkerGroup = "(github.event.head_commit.message == 'diagnostic(native-upload): approved bounded pipe run 20261022' || github.event.head_commit.message == 'diagnostic(native-upload): reviewed browser counterfactual 20261022' || github.event.head_commit.message == 'diagnostic(native-upload): reviewed introspection gate 20261022')";
const retainedMarkerSingle = "github.event.head_commit.message == 'diagnostic(native-upload): approved bounded pipe run 20261022'";
const retainedPipeJob = "\n  native-upload-diagnostic:\n    # One exact approved publication selects this job instead of the combined suite.\n    if: >-\n      github.event_name == 'push' && github.event.repository.private == false && github.repository == '9hnm24bzvm-wq/nostr-manga' && github.ref == 'refs/heads/ci/browser-failure-diagnostics' && github.event.head_commit.message == 'diagnostic(native-upload): approved bounded pipe run 20261022'\n    runs-on: ubuntu-22.04\n    timeout-minutes: 15\n    env:\n      CI: 'true'\n      PYTHONDONTWRITEBYTECODE: '1'\n      NODE_OPTIONS: --max-old-space-size=512\n      PLAYWRIGHT_BROWSERS_PATH: ${{ github.workspace }}/.ci/native-diagnostic-browsers\n    steps:\n      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1\n        with:\n          persist-credentials: false\n          fetch-depth: 2\n      - name: Admit only the exact approved predecessor and file scope\n        timeout-minutes: 1\n        run: python3 -B weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py preflight\n      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4.4.0\n        with:\n          node-version: 24.19.0\n      - name: Prepare exact official browser tools\n        timeout-minutes: 7\n        run: |\n          mkdir -p weebdex-platform/.build/native-upload-diagnostic\n          if ! (npm ci --ignore-scripts --registry=https://registry.npmjs.org --no-audit --no-fund --omit=optional --prefix .ci/browser && node .ci/browser/node_modules/playwright/cli.js install chromium) >weebdex-platform/.build/native-upload-diagnostic/private-preparation.log 2>&1; then\n            echo 'Dedicated browser preparation failed.'\n            exit 1\n          fi\n      - name: Run six bounded pipe-ownership comparisons with normal Chromium sandbox\n        id: diagnostic\n        timeout-minutes: 5\n        run: python3 -B weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py run\n      - name: Validate one closed sanitized diagnostic report\n        id: diagnostic-validation\n        if: always() && steps.diagnostic.outcome != 'skipped'\n        timeout-minutes: 1\n        run: |\n          python3 -B weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py verify-owner\n          mkdir -p weebdex-platform/ci-native-upload-evidence\n          node weebdex-platform/scripts/ci/native-upload-diagnostic-report.mjs --input weebdex-platform/.build/native-upload-diagnostic/report.json --output weebdex-platform/ci-native-upload-evidence/report.json\n      - name: Upload only the bounded validated diagnostic\n        if: always() && steps.diagnostic-validation.outcome == 'success'\n        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2\n        with:\n          name: native-upload-diagnostic-${{ github.run_id }}-${{ github.run_attempt }}\n          path: weebdex-platform/ci-native-upload-evidence/report.json\n          if-no-files-found: error\n          retention-days: 1\n          include-hidden-files: false\n";
const retainedPipeSkip = " && !(github.event_name == 'push' && github.ref == 'refs/heads/ci/browser-failure-diagnostics' && github.event.head_commit.message == 'diagnostic(native-upload): approved bounded pipe run 20261022')";
export function restoreRetainedPublicWorkflow(input) {
  if (hash(input) === retainedPublic) {
    assert.equal(input.split(retainedMarkerGroup).length - 1, 1);
    input = input.replace(retainedMarkerGroup, retainedMarkerSingle);
  }
  assert.equal(hash(input), retainedPipePredecessor, 'Unreviewed retained public workflow bytes');
  assert.equal(input.split(retainedPipeJob).length - 1, 1);
  assert.equal(input.split(retainedPipeSkip).length - 1, 1);
  const restored = input.replace(retainedPipeJob, '').replace(retainedPipeSkip, '').replace('    if: >-\n      ', '    if: ');
  assert.equal(hash(restored), prior);
  return restored;
}

export function restoreNativeDiagnosticWorkflow(input) {
  if (hash(input) === prior) return input;
  if ([retainedPublic, retainedPipePredecessor].includes(hash(input))) return restoreRetainedPublicWorkflow(input);
  input = restoreSignedFeatureWorkflow(input);
  assert.equal(hash(input), current, 'Unreviewed native diagnostic workflow bytes');
  assert.equal(input.split(job).length - 1, 1);
  assert.equal(input.split(skip).length - 1, 1);
  const restored = input.replace(job, '').replace(skip, '').replace('    if: >-\n      ', '    if: ');
  assert.equal(hash(restored), prior);
  return restored;
}
