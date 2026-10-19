#!/usr/bin/env node
// Finite harness contract/seams only. Inner browser actions are explicit
// stand-ins in wiring cases; none establishes selectors or runtime acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
import test from 'node:test';
import { actorFollowPhase, actorFollowDiagnosticChecks, actorFollowCaseChecks, runActorFollowJourneys, watchActorFollowRequests } from '../../scripts/browser-actor-follow-journeys.mjs';
import { ui11To16JourneyPhases, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';

const caseChecks = {
  'initial-readiness': ['prerequisite', 'initial-session', 'status-pending', 'status-pending-follow-visible', 'status-pending-follow-disabled', 'status-pending-block-hidden', 'status-pending-no-writes', 'status-settled'],
  'read-retry': ['prerequisite', 'delivery-failed', 'retry-pending', 'retry-settled'],
  'unblock-loading': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
  'unblock-error': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
  'pending-ack': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
  'pending-lost': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
  'pending-disposed': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
};
const aggregates = ['actor-follow-account', ...['group', 'user'].flatMap(kind => Object.keys(caseChecks).map(name => `actor-follow-${kind}-${name}`))];
const expected = ['actor-follow-account', ...['group', 'user'].flatMap(kind => Object.entries(caseChecks).flatMap(([name, suffixes]) => {
  const id = `actor-follow-${kind}-${name}`; return [id, ...suffixes.map(suffix => id + '-' + suffix)];
}))];

test('actor-follow enrollment is fixed, complete, ordered and frozen', () => {
  assert.equal(actorFollowPhase, 'ui14-actor-follow');
  assert.deepEqual(actorFollowDiagnosticChecks, { [actorFollowPhase]: expected });
  assert.ok(Object.isFrozen(actorFollowDiagnosticChecks)); assert.ok(Object.isFrozen(actorFollowDiagnosticChecks[actorFollowPhase]));
  assert.equal(new Set(expected).size, 79);
  assert.deepEqual(actorFollowCaseChecks, caseChecks); assert.ok(Object.isFrozen(actorFollowCaseChecks));
  for (const checks of Object.values(actorFollowCaseChecks)) assert.ok(Object.isFrozen(checks));
  assert.equal(aggregates.length, 15);
  assert.equal(ui11To16JourneyPhases.filter(phase => phase === actorFollowPhase).length, 1);
  assert.deepEqual(ui11To16DiagnosticChecks[actorFollowPhase], expected);
});

const currentSourceAssociation = JSON.parse(fs.readFileSync(new URL('./fixtures/browser-actor-follow-current-source.json', import.meta.url), 'utf8'));
const hash = value => createHash('sha256').update(value).digest('hex');
const historicalFixture = currentSourceAssociation.historical_source_fixture;
assert.equal(historicalFixture.path, 'fixtures/browser-actor-follow-historical-source.json.gz');
assert.equal(historicalFixture.maximum_uncompressed_bytes, 4 * 1024 * 1024);
const historicalCompressed = fs.readFileSync(new URL('./' + historicalFixture.path, import.meta.url));
assert.equal(historicalCompressed.length, historicalFixture.bytes);
assert.equal(hash(historicalCompressed), historicalFixture.sha256);
const historicalExpanded = gunzipSync(historicalCompressed, { maxOutputLength: historicalFixture.maximum_uncompressed_bytes });
assert.equal(historicalExpanded.length, historicalFixture.recorded_uncompressed_bytes);
const historicalSource = JSON.parse(historicalExpanded);
assert.equal(historicalSource.schema, 1);
assert.equal(historicalSource.historical_commit, currentSourceAssociation.historical_commit);
assert.equal(historicalSource.initial_current_commit, currentSourceAssociation.current_commit);
function historicalBytes(file) {
  assert.ok(Object.hasOwn(historicalSource.historical_source_base64, file), 'Missing portable historical source: ' + file);
  return Buffer.from(historicalSource.historical_source_base64[file], 'base64');
}

test('supplemental source plan binds its frozen source without changing historical baseline scope', () => {
  const root = new URL('../../', import.meta.url);
  const planBytes = fs.readFileSync(new URL('test-evidence/frontend/browser-actor-follow-plan.json', root));
  assert.equal(hash(planBytes), currentSourceAssociation.historical_plan_sha256);
  assert.deepEqual(planBytes, Buffer.from(historicalSource.historical_plan_base64, 'base64'));
  const plan = JSON.parse(planBytes);
  const historical = JSON.parse(fs.readFileSync(new URL(plan.historical_browser_plan, root), 'utf8'));
  assert.equal(plan.base_commit, '56f9cb248334eff951be571a567697b73f710fab');
  assert.equal(plan.observed_correction_base, '94c00b8b0688b121499743ed213e465eec69bc37');
  assert.equal(plan.status, 'source-prepared-browser-unrun');
  assert.deepEqual(plan.supplemental_phases, [actorFollowPhase]); assert.deepEqual(plan.checks, expected);
  assert.equal(new Set(['startup', ...Object.values(historical.phases).flatMap(group => group.names ?? [])]).size, 38);
  assert.equal(plan.historical_phases_retained, 38); assert.equal(plan.total_required_unique_phases, 39);
  assert.deepEqual(Object.keys(historicalSource.historical_source_base64).sort(), Object.keys(plan.source_sha256).sort());
  for (const [file, digest] of Object.entries(plan.source_sha256)) assert.equal(hash(historicalBytes(file)), digest, file);
  for (const required of ['scripts/browser-actor-follow-journeys.mjs', 'scripts/browser-actor-follow-controls.mjs', 'scripts/browser-ui11-16-journeys.mjs', 'scripts/ci/run-hosted-browser.py', 'scripts/ci/journey-diagnostics.py', 'scripts/ci/validate-ci-evidence.py']) assert.ok(Object.hasOwn(plan.source_sha256, required), required);
});

test('current actor-follow runtime source has a separate exact association without relabeling historical evidence', () => {
  const root = new URL('../../', import.meta.url);
  assert.equal(currentSourceAssociation.status, 'source-associated-browser-unrun');
  assert.equal(currentSourceAssociation.current_commit, 'f593787141469cc8d6da057545eb11a53fb7271b');
  assert.equal(currentSourceAssociation.historical_commit, '9079c3bf02ef49c5d94fdcaf1d696bdb61416cd1');
  assert.deepEqual(Object.keys(currentSourceAssociation.current_runtime_sha256), [
    'scripts/browser-actor-follow-journeys.mjs', 'scripts/browser-actor-follow-controls.mjs',
    'scripts/browser-actor-follow-observations.mjs', 'scripts/browser-ui11-16-journeys.mjs',
    'scripts/ci/run-hosted-browser.py', 'scripts/ci/journey-diagnostics.py',
    'scripts/ci/validate-ci-evidence.py', '../.github/workflows/hosted-browser-e2e.yml',
  ]);
  const workflow = '../.github/workflows/hosted-browser-e2e.yml';
  assert.deepEqual(currentSourceAssociation.current_source_overrides, { [workflow]: 'c6a0dc38a7be5026dbd3c7a5f2ec4c04a752f1d2' });
  const priorWorkflow = Buffer.from(historicalSource.initial_current_workflow_base64, 'base64');
  assert.equal(hash(priorWorkflow), currentSourceAssociation.initial_current_workflow_sha256);
  const proxyStep = '      - name: Check finite deployment proxy route and header contracts\n        timeout-minutes: 2\n        run: node --test weebdex-platform/tests/ci/test_deployment_proxy_routes.mjs\n';
  const publishedCiWorkflow = fs.readFileSync(new URL(workflow, root), 'utf8');
  // Admit only the exact recovery workflow or its separately approved CI-only
  // successor. Neither may relabel immutable c6/f593/9079 evidence.
  const recoveryWorkflowSha = 'df7481090327214e528ec4d8025d5d05212cbdf7f035294677451e74a59c2c27';
  const publicUploadWorkflowSha = '19f3cada257be951ee397ef5a10830efb5af0c6033c1978bbdbc3cd8f54951c1';
  assert.equal(currentSourceAssociation.current_runtime_sha256[workflow], recoveryWorkflowSha);
  const currentApplicationWorkflowSha = '876f174a31c346e17efdbec045e5f00377c2cf3fae966eac5445b4858754f6a1';
  const workflowChanges = [
    ["inputs.batch || 'public-upload'", "inputs.batch || 'all'", 2],
    ['timeout-minutes: 140', 'timeout-minutes: 345', 1],
    ['timeout-minutes: 30', 'timeout-minutes: 245', 1],
    ['--budget-seconds 3600', '--budget-seconds 14400', 1],
  ];
  const sealedQaWorkflowSha = '4a62a482ec907923bb830a41f3524e21f7f23ac6868aedac76caed483feffa3e';
  const sealedQaWorkflowBlocks = ['        id: preparation\n', "      - name: Seal current QA preparation failure diagnostics\n        id: qa-preparation-seal\n        if: always() && steps.preparation.outcome == 'failure' && steps.evidence-validation.outcome == 'success' && hashFiles('weebdex-platform/.build/hosted-browser/qa-preparation-failure/capture.json') != ''\n        timeout-minutes: 2\n        run: |\n          set -euo pipefail\n          PREP_SHA=\"$(python3 -c 'import hashlib; from pathlib import Path; print(hashlib.sha256((Path(\"weebdex-platform/ci-sanitized-evidence\") / \"preparation.json\").read_bytes()).hexdigest())')\"\n          node weebdex-platform/scripts/ci/seal-qa-preparation.mjs --expected-preparation-sha256 \"$PREP_SHA\"\n          SEALED_SHA=\"$(python3 -c 'import hashlib; from pathlib import Path; print(hashlib.sha256(Path(\"weebdex-platform/ci-encrypted-evidence/qa-preparation-failure.json\").read_bytes()).hexdigest())')\"\n          node weebdex-platform/scripts/ci/seal-qa-preparation.mjs --validate --expected-preparation-sha256 \"$PREP_SHA\" --expected-sealed-sha256 \"$SEALED_SHA\"\n          printf 'sealed_sha256=%s\\n' \"$SEALED_SHA\" >> \"$GITHUB_OUTPUT\"\n      - name: Upload only the validated encrypted QA preparation failure\n        if: always() && steps.qa-preparation-seal.outcome == 'success' && steps.qa-preparation-seal.outputs.sealed_sha256 != ''\n        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2\n        with:\n          name: hosted-qa-preparation-sealed-${{ github.run_id }}-${{ github.run_attempt }}\n          path: weebdex-platform/ci-encrypted-evidence/qa-preparation-failure.json\n          if-no-files-found: error\n          retention-days: 1\n          include-hidden-files: false\n"];
  const normalizeWorkflow = input => {
    let digest = hash(input);
    assert([recoveryWorkflowSha, publicUploadWorkflowSha, currentApplicationWorkflowSha, sealedQaWorkflowSha].includes(digest), 'Unknown workflow source profile');
    let value = input;
    if (digest === sealedQaWorkflowSha) {
      for (const block of sealedQaWorkflowBlocks) {
        assert.equal(value.split(block).length - 1, 1);
        value = value.replace(block, '');
      }
      assert.equal(hash(value), currentApplicationWorkflowSha);
      digest = currentApplicationWorkflowSha;
    }
    if (digest === currentApplicationWorkflowSha) {
      const setupPython = "      - uses: actions/setup-python@e797f83bcb11b83ae66e0230d6156d7c80228e7c # v6.0.0\n        with:\n          python-version: '3.12.15'\n          architecture: x64\n          check-latest: false\n";
      assert.equal(value.split(setupPython).length - 1, 1);
      value = value.replace(setupPython, '');
      assert.equal(value.split('runs-on: ubuntu-22.04').length - 1, 1);
      value = value.replace('runs-on: ubuntu-22.04', 'runs-on: ubuntu-24.04');
      assert.equal(value.split('default: public-upload').length - 1, 1);
      value = value.replace('default: public-upload', 'default: all');
      assert.equal(hash(value), publicUploadWorkflowSha);
    }
    if ([publicUploadWorkflowSha, currentApplicationWorkflowSha].includes(digest)) for (const [current, historical, count] of workflowChanges) {
      assert.equal(value.split(current).length - 1, count);
      value = value.replaceAll(current, historical);
    }
    assert.equal(hash(value), recoveryWorkflowSha);
    return value;
  };
  const currentWorkflow = normalizeWorkflow(publishedCiWorkflow);
  // Exercise both known profiles regardless of which exact checkout runs this
  // test, and refuse an otherwise similar workflow with a changed budget.
  let proposedWorkflow = currentWorkflow;
  for (const [current, historical, count] of workflowChanges) {
    assert.equal(proposedWorkflow.split(historical).length - 1, count);
    proposedWorkflow = proposedWorkflow.replaceAll(historical, current);
  }
  assert.equal(hash(proposedWorkflow), publicUploadWorkflowSha);
  assert.equal(normalizeWorkflow(proposedWorkflow), currentWorkflow);
  assert.equal(normalizeWorkflow(currentWorkflow), currentWorkflow);
  assert.throws(() => normalizeWorkflow(proposedWorkflow.replace('--budget-seconds 3600', '--budget-seconds 3601')), /Unknown workflow source profile/);
  assert.equal(currentWorkflow.split(proxyStep).length - 1, 1);
  assert.equal(currentWorkflow.replace(proxyStep, ''), priorWorkflow.toString('utf8'));
  const validator = 'scripts/ci/validate-ci-evidence.py';
  let currentValidator = fs.readFileSync(new URL(validator, root), 'utf8');
  // Independently reviewed native-title hash publication is a later successor.
  // Reverse only its two exact blocks before applying historical diagnostic pins.
  const priorDiagnosticValidatorSha = '0680e55915180c517714560e9d83fa6b52a29cc412cefc2731652af531a13c77';
  const nativeTitleValidatorSha = '29f59b60e44e2b8d5d396c993b5c33f8d9b12abd815195ee3819b859bc472969';
  const nativeTitleValidatorBlocks = [
    "def title_artifact_hashes(prep, source):\n    title = prep['native_feed_regression']['title_disposition']\n    if title['artifacts_verified']:\n        # Hash-shaped text is admitted only for the actual current Cargo-selected\n        # artifact, never arbitrary/private 64-hex with the right syntax.\n        plan = runner.read(ROOT / '.build/hosted-browser/plan.json')\n        executables = native_feed.title.verify_artifacts(plan.get('native_title_tests'), ROOT, prep['source_snapshot_sha256'])\n        insist(title['executable_sha256'] == {kind: source.sha(path) for kind, path in executables.items()})\n\n\n",
    '        title_artifact_hashes(prep, source)\n',
  ];
  const normalizeNativeTitleValidator = input => {
    const digest = hash(input);
    assert([priorDiagnosticValidatorSha, nativeTitleValidatorSha].includes(digest), 'Unknown native-title evidence validator');
    let value = input;
    if (digest === nativeTitleValidatorSha) for (const block of nativeTitleValidatorBlocks) {
      assert.equal(value.split(block).length - 1, 1);
      value = value.replace(block, '');
    }
    assert.equal(hash(value), priorDiagnosticValidatorSha);
    return value;
  };
  // Only the exact fixed preparation-stage successor may reverse this one
  // literal enum addition before the separately fenced native-title reversal.
  const profileStageValidatorSha = '2fcb77caf7be6ee9b12f3c5ef40c7415653b54fd2ec13768a793168cecd4bf4e';
  const profileStageAddition = ", 'complete',\n                            'public-upload-profile-preflight', 'public-upload-profile-native-off',\n                            'public-upload-profile-native-evidence', 'public-upload-qa-build',\n                            'public-upload-qa-source', 'public-upload-qa-receipt', 'public-upload-qa-graph',\n                            'public-upload-qa-record', 'public-upload-qa-final-source',\n                            'public-upload-qa-final-graph', 'public-upload-qa-seal',\n                            'public-upload-profile-restore'])";
  const normalizeProfileStageValidator = input => {
    const digest = hash(input);
    assert([priorDiagnosticValidatorSha, nativeTitleValidatorSha, profileStageValidatorSha].includes(digest), 'Unknown preparation-stage evidence validator');
    let value = input;
    if (digest === profileStageValidatorSha) {
      assert.equal(value.split(profileStageAddition).length - 1, 1);
      value = value.replace(profileStageAddition, ", 'complete'])");
      assert.equal(hash(value), nativeTitleValidatorSha);
    }
    return value;
  };
  const profileStageValidator = currentValidator;
  currentValidator = normalizeProfileStageValidator(currentValidator);
  assert.equal(normalizeProfileStageValidator(currentValidator), currentValidator);
  assert.throws(() => normalizeProfileStageValidator(profileStageValidator.replace('def phase_list(value):', 'def phase_list_changed(value):')), /Unknown preparation-stage evidence validator/);
  const nativeTitleValidator = currentValidator;
  currentValidator = normalizeNativeTitleValidator(currentValidator);
  assert.equal(normalizeNativeTitleValidator(currentValidator), currentValidator);
  assert.throws(() => normalizeNativeTitleValidator(nativeTitleValidator.replace('def phase_list(value):', 'def phase_list_changed(value):')), /Unknown native-title evidence validator/);
  // Exact diagnostic-only successor; preserve the immutable historical/runtime pins.
  assert.equal(hash(currentValidator), "0680e55915180c517714560e9d83fa6b52a29cc412cefc2731652af531a13c77");
  for (const [current, historical] of [
    [
        ", 'caught_stage', 'caught_category', 'browser_created', 'context_created', 'page_created'",
        ""
    ],
    [
        "    insist(type(value['caught_stage']) is str and value['caught_stage'] in runner_diagnostics.STAGES | {'none', 'unavailable'})\n    insist(type(value['caught_category']) is str and value['caught_category'] in runner_diagnostics.CAUGHT_CATEGORIES | {'unavailable'})\n    insist((value['caught_stage'] == 'unavailable') is (value['caught_category'] == 'unavailable'))\n    if value['caught_stage'] != 'unavailable':\n        insist(runner_diagnostics.caught_pair(value['caught_stage'], value['caught_category']))\n    created = [value[key] for key in ['browser_created', 'context_created', 'page_created']]\n    insist(all(item is None for item in created) or all(type(item) is bool for item in created))\n    if all(type(item) is bool for item in created):\n        insist(not value['context_created'] or value['browser_created'])\n        insist(not value['page_created'] or value['context_created'])\n",
        ""
    ],
    [
        "        insist(all(type(item) is bool for item in created))\n",
        ""
    ],
    [
        "'fatal_stage', 'caught_stage', 'caught_category'",
        "'fatal_stage'"
    ],
    [
        "        insist(runner_diagnostics.caught_pair(value['caught_stage'], value['caught_category']))\n",
        ""
    ]
]) {
    assert.equal(currentValidator.split(current).length - 1, 1);
    currentValidator = currentValidator.replace(current, historical);
  }
  assert.equal(hash(currentValidator), currentSourceAssociation.current_runtime_sha256[validator]);
  const runner = 'scripts/ci/run-hosted-browser.py';
  const runnerChanges = [
  [
    "import stat\n",
    ""
  ],
  [
    "_temp_spec = importlib.util.spec_from_file_location('owned_browser_temp', Path(__file__).with_name('browser-temp.py'))\nbrowser_temp = importlib.util.module_from_spec(_temp_spec)\n_temp_spec.loader.exec_module(browser_temp)\n",
    ""
  ],
  [
    "BROWSER_TMP_CLEANUP_SECONDS = 5\n\n\ndef persist_browser_temp(owner, *, process_tree_stopped=False, cleanup_verified=False):\n    \"\"\"Private bounded journal held open from admission through final cleanup.\"\"\"\n    output = owner.browser_temp_record\n    held = os.fstat(output.fileno())\n    named = os.stat(owner.browser_temp_record_path, follow_symlinks=False)\n    if (not stat.S_ISREG(held.st_mode) or not stat.S_ISREG(named.st_mode) or\n            (held.st_dev, held.st_ino) != (named.st_dev, named.st_ino) or\n            held.st_nlink != 1 or held.st_uid != os.geteuid() or\n            stat.S_IMODE(held.st_mode) != 0o600):\n        raise RuntimeError('Owned browser temp journal changed')\n    value = dict(schema=1, **owner.browser_temp.journal(),\n                 process_tree_stopped=process_tree_stopped, cleanup_verified=cleanup_verified)\n    data = json.dumps(value, sort_keys=True) + '\\n'\n    if len(data.encode('utf-8')) > 4096:\n        raise RuntimeError('Owned browser temp journal exceeded its bound')\n    output.seek(0)\n    output.truncate()\n    output.write(data)\n    output.flush()\n    os.fsync(output.fileno())\n\n\ndef prepare_browser_temp(owner, name):\n    \"\"\"Register a fresh exact root and persist its admission before any child.\"\"\"\n    owner.browser_temp = browser_temp.BrowserTemp()\n    owner.browser_temp_record = None\n    owner.browser_temp_record_path = STAGE / ('browser-temp-' + name + '.json')\n    owner.browser_temp.create()\n    descriptor = os.open(owner.browser_temp_record_path,\n                         os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)\n    owner.browser_temp_record = os.fdopen(descriptor, 'w')\n    persist_browser_temp(owner)\n\n\ndef finish_browser_temp(owner, process, batch, deadline):\n    \"\"\"Never remove temporary bytes until the complete owned tree is stopped.\"\"\"\n    temp = getattr(owner, 'browser_temp', None)\n    output = getattr(owner, 'browser_temp_record', None)\n    stopped = clean = False\n    def uncertain():\n        batch['cleanup_error_class'] = 'RuntimeError'\n        batch['cleanup']['verified_stopped'] = False\n        batch.setdefault('outer_cleanup', dict(forced=False,\n            verified_stopped=False, remaining_owned_process_count=0))['verified_stopped'] = False\n    try:\n        if temp is None:\n            if process is not None:\n                raise RuntimeError('Owned browser temp registration unavailable')\n            return\n        if process is not None:\n            if (process.poll() is None or batch.get('supervisor_terminal') is not True or\n                    batch.get('outer_cleanup', {}).get('verified_stopped') is not True):\n                raise RuntimeError('Owned browser temp process closure unverified')\n        elif owner.process_roots:\n            raise RuntimeError('Owned browser temp child admission unverified')\n        if owner.process_members(None) or owner.process_tracking_errors:\n            raise RuntimeError('Owned browser temp process closure unverified')\n        stopped = True\n        temp.cleanup(deadline)\n        clean = not temp.created or temp.removed\n        if temp.created and output is None:\n            raise RuntimeError('Owned browser temp journal unavailable')\n    except Exception:\n        uncertain()\n    finally:\n        try:\n            if output is not None:\n                persist_browser_temp(owner, process_tree_stopped=stopped,\n                                     cleanup_verified=clean and 'cleanup_error_class' not in batch)\n        except Exception:\n            uncertain()\n        finally:\n            try:\n                if temp is not None:\n                    temp.close()\n            except Exception:\n                uncertain()\n            finally:\n                if output is not None:\n                    try:\n                        output.close()\n                    except Exception:\n                        uncertain()\n\n\n",
    ""
  ],
  [
    "            prepare_browser_temp(owner, name)\n            # The fixture forwards this allowlisted override to Node/Chromium.\n            # No caller-supplied TMPDIR/TMP/TEMP can select the browser root.\n            env['TMPDIR'] = str(owner.browser_temp.path)\n",
    ""
  ],
  [
    "                owner.browser_temp.check()\n                if not owner.browser_temp.admitted or env.get('TMPDIR') != str(owner.browser_temp.path):\n                    raise RuntimeError('Owned browser temp environment refused')\n",
    ""
  ],
  [
    "            batch['error_class'] = 'RuntimeError' if isinstance(error, browser_temp.Refused) else type(error).__name__\n",
    "            batch['error_class'] = type(error).__name__\n"
  ],
  [
    "            # Use the existing 600-second teardown envelope; temp cleanup gets\n            # at most five seconds of the remaining reserve, never a new one.\n            cleanup_deadline = time.monotonic() + 600\n            try:\n                if process:\n                    finish_supervisor(owner, process, fixture, batch)\n            finally:\n                finish_browser_temp(owner, process, batch,\n                    min(cleanup_deadline, time.monotonic() + BROWSER_TMP_CLEANUP_SECONDS))\n",
    "            if process:\n                finish_supervisor(owner, process, fixture, batch)\n"
  ]
];
  function normalizeApplicationRunner(input) {
    const digest = hash(input);
    const qaRunnerSha = 'cd82616c45f8d4ee81873a4be45e6206bc133c4f98e167f28b88e8e0449835fb';
    assert(['a904994f8ab19164174dc62f17df48ed926de0567603340f080b39415e00c0a6', '6225772326ec91e82da678aeb805f920536f8b086e03df64041c5f9302ef037b', qaRunnerSha].includes(digest), 'Unknown owned-temp application runner');
    let value = input.toString();
    if (digest === qaRunnerSha) {
      const qaChanges = [
        ["'frontend_index_sha256', 'build_receipt', 'build_receipt_sha256', 'qa_upload_refusals'", "'frontend_index_sha256', 'build_receipt', 'build_receipt_sha256'"],
        ["    spec = importlib.util.spec_from_file_location('admitted_public_upload_qa_build', Path(__file__).with_name('public-upload-qa-build.py'))\n    qa = importlib.util.module_from_spec(spec)\n    spec.loader.exec_module(qa)\n    if qa.verify(ROOT, ui, True) != qa.record(profile['qa_upload_refusals']):\n        raise RuntimeError('Public upload QA graph differs from the prepared generation')\n", ''],
      ];
      for (const [current, previous] of qaChanges) {
        assert.equal(value.split(current).length - 1, 1);
        value = value.replace(current, previous);
      }
      assert.equal(hash(value), '6225772326ec91e82da678aeb805f920536f8b086e03df64041c5f9302ef037b');
    }
    if (hash(value) === '6225772326ec91e82da678aeb805f920536f8b086e03df64041c5f9302ef037b') for (const [current, previous] of runnerChanges) {
      assert.equal(value.split(current).length - 1, 1);
      value = value.replace(current, previous);
    }
    assert.equal(hash(value), currentSourceAssociation.current_runtime_sha256[runner]);
    return value;
  }
  const applicationRunner = fs.readFileSync(new URL(runner, root), 'utf8');
  const historicalRunner = normalizeApplicationRunner(applicationRunner);
  assert.throws(() => normalizeApplicationRunner(applicationRunner + '\n# unexpected source\n'), /Unknown owned-temp application runner/);
  for (const [file, digest] of Object.entries(currentSourceAssociation.current_runtime_sha256)) {
    const bound = file === workflow ? currentWorkflow : file === validator ? currentValidator : file === runner ? historicalRunner : fs.readFileSync(new URL(file, root));
    assert.equal(hash(bound), digest, 'historical current association: ' + file);
  }
});

test('request counter observes exact browser target methods/settings only and removes its listener', () => {
  const listeners = new Map(), origin = 'http://127.0.0.1:31415', endpoint = '/api/user/follows/groups/SeedGroup1';
  const page = { on(name, callback) { assert.equal(name, 'request'); listeners.set(name, callback); }, off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); } };
  const watched = watchActorFollowRequests(page, origin, endpoint);
  const emit = (path, method, base = origin) => listeners.get('request')({ url: () => base + path, method: () => method });
  for (const method of ['GET', 'POST', 'DELETE']) emit(endpoint, method);
  emit('/api/settings', 'PUT'); emit('/api/settings', 'GET'); emit(endpoint, 'PUT'); emit(endpoint + '/other', 'POST'); emit('/api/auth/check', 'GET'); emit(endpoint, 'POST', 'http://localhost:31415'); emit(endpoint, 'POST', 'http://127.0.0.1:31416');
  assert.deepEqual(watched.counts, { GET: 1, POST: 1, DELETE: 1, PUT: 1 });
  watched.close(); assert.equal(listeners.size, 0);
});

for (const failAt of [undefined, ...aggregates]) test('actual outer phase wiring retains checkpoint failures: ' + (failAt ?? 'complete'), async () => {
  const report = {}, snapshots = [], steps = [], error = Error('private synthetic detail must not enter finite diagnostics');
  const record = createJourneyCheck(report, () => snapshots.push(structuredClone(report)));
  await runActorFollowJourneys({ phase: actorFollowPhase, ids: { group: 'synthetic-group', seeder: 'synthetic-user' },
    insist(value, message) { assert.ok(value, message); },
    async step(name, action) { const row = { name }; steps.push(row); try { await action(); row.status = 'passed'; } catch (cause) { row.status = 'failed'; row.error = cause; } },
    async check(id, action) { assert.equal(typeof action, 'function'); return record(id, async () => { if (id === failAt) throw error; }); },
  });
  assert.deepEqual(steps.map(row => row.name), aggregates);
  assert.deepEqual(report.journey_checks, aggregates.map(check_id => ({ check_id, status: check_id === failAt ? 'failed' : 'passed' })));
  assert.equal(steps.filter(row => row.status === 'failed').length, failAt ? 1 : 0);
  if (failAt) assert.equal(steps.find(row => row.name === failAt).error, error);
  assert.doesNotMatch(JSON.stringify(snapshots), /private synthetic|synthetic-group|synthetic-user|127\.0\.0\.1|password|@/);
});

test('actual account prerequisite rejects unverified, missing, wrong-status and self principals', async () => {
  for (const actual of [{ status: 401 }, { status: 200, body: { identity: { id: 'actor', is_verified: false } } }, { status: 200, body: { identity: { id: 'target', is_verified: true } } }]) {
    let failure;
    await runActorFollowJourneys({ phase: actorFollowPhase, ids: { group: 'group', seeder: 'target' }, api: async () => actual, signup: async () => {}, insist(value, message) { assert.ok(value, message); }, async step(id, action) { if (id === 'actor-follow-account') try { await action(); } catch (error) { failure = error; } }, check: async (_id, action) => action() });
    assert.match(String(failure?.message), /real verified non-self account/);
  }
});

// Execute selected actual assertion/cleanup bodies against a small browser/API
// stand-in. These negative controls reject invented/wrong native outcomes; they
// are not real browser or server responses and cannot establish E2E acceptance.
function negativeHarness(selected, fault, actorHeader = 'actor') {
  const origin = 'http://127.0.0.1:31415', endpoint = '/api/user/follows/groups/group', listeners = new Map(), gates = [];
  let followed = false, mode = 'anonymous', error; const report = {}, record = createJourneyCheck(report);
  const emit = method => listeners.get('request')?.({ url: () => origin + endpoint, method: () => method });
  const button = name => ({
    async waitFor() { if (mode !== 'anonymous' && name === 'Follow' && fault === 'missing-initial-follow') throw Error('Strict Follow is absent'); },
    async count() { return mode === 'anonymous' ? 0 : name === 'Follow' ? fault === 'missing-initial-follow' ? 0 : 1 : name === 'Block' && fault === 'exposed-initial-block' ? 1 : 0; },
    async evaluateAll(action) { return action(await this.count() ? [{ tagName: 'BUTTON', disabled: await this.isDisabled() }] : []); },
    async isEnabled() { return true; }, async isDisabled() { return !['actionable-pending', 'actionable-initial-follow'].includes(fault); },
  });
  const header = { filter({ hasText } = {}) { if (!hasText) return this; const name = ['Follow', 'Unfollow', 'Block', 'Unblock', 'Retry follow status'].find(value => hasText.test(value)); return { ...button(name), async click() { followed = true; emit('POST'); } }; }, locator() { return this; }, async waitFor() {}, async count() { return 1; } };
  const page = { on(name, callback) { listeners.set(name, callback); }, off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); }, locator() { return header; }, url: () => origin + '/group/group' };
  const h = { phase: actorFollowPhase, page, origin, report, ids: { group: 'group', seeder: 'user' },
    insist(value, message) { assert.ok(value, message); },
    async step(id, action) { if (id === selected) try { await action(); } catch (cause) { error = cause; } },
    check: record, requestPath: () => true,
    async stable(predicate, message) { assert.ok(await predicate(), message); },
    async until(read, predicate, message) { const value = await read(); assert.ok(await predicate(value), message); return value; },
    async open() { mode = selected.endsWith('initial-readiness') ? 'anonymous' : 'loading'; if (mode === 'loading') emit('GET'); }, async realm() {},
    async api(path, options = {}) {
      if (path === '/api/auth/check') return { status: 200, body: { identity: { id: 'actor', is_verified: true }, preferences: { version: 1, groups: [], uploaders: [] } } };
      if (path === '/api/settings') return { status: 200, body: { groups: (options.data.groups ?? []).map(id => ({ id })), uploaders: [] } };
      if (path === endpoint) {
        if (options.method) { followed = options.method === 'POST'; return { status: 204 }; }
        return { status: fault === 'uncommitted-write' && mode !== 'anonymous' ? 404 : followed ? 204 : 404 };
      }
      throw Error('Unexpected synthetic seam API path');
    },
    async hold(_pattern, predicate) {
      const isAuth = selected.endsWith('initial-readiness') && gates.length === 0;
      const gate = { closed: 0, heldNativeStatus: isAuth ? 200 : followed ? 204 : 404, nativeStatus: isAuth ? (fault === 'bad-auth-status' ? 401 : 200) : fault === 'bad-read-status' ? 500 : followed ? 204 : 404, disposition: 'pending',
        async wait() { if (!isAuth) { predicate({ headers: () => actorHeader ? ({ 'x-weebdex-expected-account': actorHeader }) : ({}) }); predicate({ headers: () => ({ 'x-weebdex-expected-account': 'later-other-actor' }) }); if (fault === 'unexpected-initial-write') emit('POST'); } }, async release() { gate.disposition = 'delivered'; if (isAuth) { mode = 'loading'; emit('GET'); } }, async fail() { gate.disposition = 'failed-delivery-after-real-response'; }, async close() { gate.closed++; } };
      gates.push(gate); return gate;
    },
  };
  return { h, gates, listeners, report, error: () => error };
}

for (const [suffix, fault, message] of [
  ['initial-readiness', 'bad-auth-status', /genuine native200/],
  ['initial-readiness', 'bad-read-status', /Existing native follow/],
  ['read-retry', 'bad-read-status', /delivery loss after a genuine native404/],
  ['pending-ack', 'uncommitted-write', /not actually committed/],
  ['pending-ack', 'actionable-pending', /left its control actionable/],
]) test(`actual ${suffix} rejects ${fault} and closes all owned gates/listeners`, async () => {
  const seam = negativeHarness('actor-follow-group-' + suffix, fault);
  await runActorFollowJourneys(seam.h);
  assert.match(String(seam.error()?.message), message);
  const rows = seam.report.journey_checks;
  assert.equal(rows[0].status, 'failed'); assert.equal(rows.at(-1).status, 'failed');
  assert.ok(rows.slice(1, -1).every(row => row.status === 'passed'));
  assert.ok(rows.every(row => expected.includes(row.check_id) && Object.keys(row).sort().join(',') === 'check_id,status'));
  const indices = rows.map(row => expected.indexOf(row.check_id)); assert.deepEqual(indices, [...indices].sort((a, b) => a - b));
  assert.ok(seam.gates.length > 0); assert.ok(seam.gates.every(gate => gate.closed === 1)); assert.equal(seam.listeners.size, 0);
});

for (const [fault, suffix, message] of [
  ['missing-initial-follow', 'follow-visible', /Strict Follow is absent/],
  ['actionable-initial-follow', 'follow-disabled', /Follow became actionable/],
  ['exposed-initial-block', 'block-hidden', /exposed a new Block/],
  ['unexpected-initial-write', 'no-writes', /wrote follows or account preferences/],
]) test('initial pending failure remains exact and records the owned pre-cleanup view: ' + suffix, async () => {
  const seam = negativeHarness('actor-follow-group-initial-readiness', fault);
  await runActorFollowJourneys(seam.h);
  assert.match(String(seam.error()?.message), message);
  const failed = seam.report.journey_checks.filter(row => row.status === 'failed').map(row => row.check_id);
  assert.deepEqual(failed, ['actor-follow-group-initial-readiness', 'actor-follow-group-initial-readiness-status-pending', 'actor-follow-group-initial-readiness-status-pending-' + suffix]);
  assert.ok(!seam.report.journey_checks.some(row => row.check_id.endsWith('status-settled')));
  const [row] = seam.report.actor_follow_initial_observations;
  assert.equal(seam.report.actor_follow_initial_observations.length, 1);
  assert.equal(row.kind, 'group'); assert.equal(row.route, 'current'); assert.equal(row.held_status, 'native204');
  assert.equal(row.delivery, 'pending'); assert.equal(row.request_actor, 'prerequisite-actor');
  assert.equal(row.follow, fault === 'missing-initial-follow' ? 'absent' : fault === 'actionable-initial-follow' ? 'enabled' : 'disabled');
  assert.equal(row.block, fault === 'exposed-initial-block' ? 'disabled' : 'absent');
  assert.equal(row.post, fault === 'unexpected-initial-write' ? 'one' : 'zero');
  assert.ok(seam.gates.every(gate => gate.closed === 1)); assert.equal(seam.listeners.size, 0);
  assert.doesNotMatch(JSON.stringify(seam.report), /31415|group\/group|Strict Follow|actorId/);
});

test('all31 original assertions and ordered3 polls/5 stability conditions retain exact AST fingerprints', () => {
  assert.ok(process.env.WEEBDEX_FRONTEND_ROOT, 'Use the verified exact-lock dependency root');
  const ts = createRequire(path.join(process.env.WEEBDEX_FRONTEND_ROOT, 'package.json'))('typescript');
  const source = fs.readFileSync(new URL('../../scripts/browser-actor-follow-journeys.mjs', import.meta.url), 'utf8');
  const ast = ts.createSourceFile('actor-follow.mjs', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const calls = { insist: [], until: [], stable: [] }; let holds = 0;
  function walk(node) {
    if (ts.isCallExpression(node)) {
      const name = node.expression.getText(ast);
      if (Object.hasOwn(calls, name)) calls[name].push(node.arguments.map(arg => arg.getText(ast).replace(/\s+/g, ' ')).join(','));
      if (name === 'hold') holds++;
    }
    ts.forEachChild(node, walk);
  }
  walk(ast);
  for (const [name, count, digest] of [
    ['insist', 31, 'd1390f4c77329600af9f2f898723580f006a133b9211a87b8f2153ea39b9b88b'],
    ['until', 3, 'f63a82adb762a930cdde1667aa7af9b93abac7490e187317d1556e5300d10832'],
    ['stable', 5, 'bf70c11f1ee34d4c2e96d95824066bedcfc15fcd1db74ae3ccc3a29dad646a3d'],
  ]) { assert.equal(calls[name].length, count); assert.equal(createHash('sha256').update(JSON.stringify(calls[name])).digest('hex'), digest, name); }
  assert.equal(holds, 7);
});

for (const [header, expectedActor] of [['actor', 'prerequisite-actor'], ['private-other-actor', 'other-actor'], ['', 'anonymous']]) test('snapshot classifies only the first held request actor without publishing headers: ' + expectedActor, async () => {
  const seam = negativeHarness('actor-follow-group-initial-readiness', 'missing-initial-follow', header);
  await runActorFollowJourneys(seam.h);
  assert.match(String(seam.error()?.message), /Strict Follow is absent/);
  assert.equal(seam.report.actor_follow_initial_observations[0].request_actor, expectedActor);
  assert.doesNotMatch(JSON.stringify(seam.report), /private-other-actor|later-other-actor|x-weebdex/);
  assert.ok(seam.gates.every(gate => gate.closed === 1)); assert.equal(seam.listeners.size, 0);
});
