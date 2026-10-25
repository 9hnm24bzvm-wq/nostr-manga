// Finite source/locator seams, with explicit old-runner counterfactual. No
// browser result or attribution of the hosted run's unknown inner failure.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import test from 'node:test';
import { uploadDigest } from '../../scripts/public-upload-recovery-observer.mjs';
import { createUploadJourneyProgress, uploadJourneyStages } from '../../scripts/public-upload-journey-progress.mjs';
import { runnerSource, groupsSource, inputGroupSource, inputTextSource, groupFixture, functionBody, runWrapper } from './public-upload-group-controls-fixture.mjs';

test('group trigger source is an exact text span in a child div role=group', () => {
  assert.equal(uploadDigest(groupsSource), '46e6be074d81f70ce14fd47a3e8a74b12542c3271b74d810db4c4aa2f9fa480a');
  assert.equal(uploadDigest(inputGroupSource), '4a9ab42c90148d4cd4005496a1c9a355c389c6b2d21dd72e0e4f8d7375f1846e');
  assert.equal(uploadDigest(inputTextSource), '25a2e635883d8e58d43146be8f5af3f0d0927afe66fa05aa7baa6d95099f080b');
  assert.match(groupsSource, /<Dialog.Trigger[\s\S]*?<InputGroup.Root[\s\S]*?<InputGroup.Text class="px-2">Search\.\.\.<\/InputGroup.Text>/);
  assert.match(inputGroupSource, /<div[\s\S]*?role="group"/); assert.match(inputTextSource, /<span[\s\S]*?{@render children\?\.\(\)}/);
});
test('exact official pinned trigger props do not override the child role', { skip: !process.env.WEEBDEX_BITS_DIALOG_SOURCE_DIR }, () => {
  const dir = process.env.WEEBDEX_BITS_DIALOG_SOURCE_DIR;
  const state = readFileSync(join(dir, 'dialog.svelte.ts'), 'utf8'), trigger = readFileSync(join(dir, 'dialog-trigger.svelte'), 'utf8');
  assert.equal(uploadDigest(state), 'a68947871a8e0b8b2471a77158359d62841a216e9c19170634131d15ec2299b9');
  assert.equal(uploadDigest(trigger), 'eecde0ffeea2523e75cd318a65bf4c66577e8d9481163717a892b112c36dc8f3');
  const start = state.indexOf('export class DialogTriggerState'), end = state.indexOf('export class ', start + 1);
  const body = state.slice(start, end < 0 ? undefined : end); assert.ok(start >= 0);
  assert.match(body, /"aria-haspopup": "dialog"/); assert.doesNotMatch(body, /\brole\s*:/);
  assert.match(trigger, /mergeProps\(restProps, triggerState.props\)/);
  assert.match(trigger, /{#if child}[\s\S]*?{@render child\({ props: mergedProps }\)}[\s\S]*?{:else}[\s\S]*?<button/);
});
for (const options of [{}, { triggerDelay: 3, resultDelay: 3 }]) test('actual corrected chooseGroup waits, selects and closes with source-extracted progress: ' + JSON.stringify(options), async () => {
  const f = groupFixture(options); await f.run();
  assert.equal(f.state.selected, true); assert.equal(f.state.dialogOpen, false); assert.equal(f.state.triggerClicks, 1); assert.equal(f.state.resultClicks, 1);
  assert.deepEqual(f.rows.at(-1), { check_index: 2, substep: 'complete', status: 'passed', caught_category: 'none' });
  assert.deepEqual([...new Set(f.snapshots.filter(rows => rows.length === 3).map(rows => rows[2].substep))], ['start', 'group-trigger', 'group-query', 'group-result', 'group-close', 'complete']);
});
for (const [name, options, stage, category] of [
  ['duplicate trigger', { duplicateTrigger: true }, 'group-trigger', 'duplicate-control'],
  ['disabled trigger', { disabledTrigger: true }, 'group-trigger', 'browser-timeout'],
  ['failed search', { searchFails: true }, 'group-result', 'readiness-timeout'],
  ['duplicate result', { duplicateResult: true }, 'group-result', 'duplicate-control'],
  ['disabled result', { disabledResult: true }, 'group-result', 'readiness-timeout'],
  ['unclosed dialog', { closeFails: true }, 'group-close', 'readiness-timeout'],
]) test('actual group flow fails closed for ' + name, async () => {
  const f = groupFixture(options); await assert.rejects(f.run());
  assert.deepEqual(f.rows.at(-1), { check_index: 2, substep: stage, status: 'failed', caught_category: category });
  assert.equal(f.state.resultClicks, stage === 'group-close' ? 1 : 0);
});
for (const stage of ['group-trigger', 'group-query', 'group-result', 'group-close']) test('first inner failure identity survives source-extracted run at ' + stage, async () => {
  const error = Object.assign(Error('reserved-example.invalid/private-value'), { code: 'ERR_ASSERTION' });
  const f = groupFixture({ failStage: stage, actionError: error }); await assert.rejects(f.run());
  assert.equal(f.observedErrors[0], error); assert.equal(f.rows.at(-1).substep, stage); assert.equal(f.rows.at(-1).caught_category, 'assertion');
  assert.doesNotMatch(JSON.stringify(f.rows), /reserved-example|private-value/);
});
for (const saveAt of [5, 6, 10]) test('runner preserves the first progress persistence failure at save ' + saveAt, async () => {
  const f = groupFixture({ failSaves: [saveAt, saveAt + 1] }); await assert.rejects(f.run());
  assert.equal(f.observedErrors[0], f.saveError); assert.equal(f.rows.at(-1).status, 'failed'); assert.notEqual(f.rows.at(-1).substep, 'complete');
});
test('every runner marker is a fixed helper literal and the final gate precedes final passed()', () => {
  const marks = [...runnerSource.matchAll(/\bmark\('([^']+)'\)/g)].map(match => match[1]);
  assert.deepEqual([...new Set(marks)].sort(), uploadJourneyStages.filter(stage => !['start', 'complete'].includes(stage)).sort());
  assert.match(runnerSource, /progress\.begin\(checkIndex\); const value = await action\(\); progress\.passed\(\)/);
  assert.match(runnerSource, /assert\.equal\(observer\.mutationCount\(\), 21, 'Exact natural-success journey mutation count'\); result\.completed = true; return/);
  assert.doesNotMatch(runnerSource, /\.fulfill\(|dispatchEvent\(|window\.nostr/);
});
test('fourteen source-extracted run calls retain strict exact order and terminal bound', async () => {
  const rows = [], report = { steps: [] }, progress = createUploadJourneyProgress(rows, () => {}), run = runWrapper(progress, report);
  for (let index = 0; index < 14; index++) await run('fixed synthetic check', async () => progress.mark('route'));
  assert.equal(rows.length, 14); assert.deepEqual(rows.map(row => row.check_index), Array.from({ length: 14 }, (_, index) => index));
  await assert.rejects(run('fixed overflow', async () => {})); assert.equal(rows.length, 14);
});
for (const status of [200, 202, 403, 503]) test('source-extracted completeWrites still enforces genuine HTTP status ' + status, async () => {
  const body = functionBody(runnerSource, 'completeWrites(start, count)', '\n  async function durable');
  const observer = { rows: [{ method: 'POST', native: 'finished', status }], mutationCount: () => 1, settled: () => true }, marks = [];
  const until = async (read, valid) => { const value = await read(); assert.equal(valid(value), true); return value; };
  const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
  const result = new AsyncFunction('observer', 'until', 'assertUploadRoute', 'assert', 'mark', 'start', 'count', body)(observer, until, () => {}, assert, stage => marks.push(stage), 0, 1);
  if (status === 200) { await result; assert.deepEqual(marks, ['mutation-count', 'browser-terminal', 'route']); } else await assert.rejects(result);
});
test('exact 17d26 runner counterfactual cannot locate the actual child trigger by button role', { skip: !process.env.WEEBDEX_UPLOAD_GROUP_BASELINE_SOURCE }, async () => {
  const source = readFileSync(process.env.WEEBDEX_UPLOAD_GROUP_BASELINE_SOURCE, 'utf8');
  assert.equal(uploadDigest(source), 'f32c89481ee6cfedddcda1ee1b1a692dbeb89a2e74d209ce06597b99f16146f1');
  const f = groupFixture({ source }); await assert.rejects(f.run());
  assert.equal(f.state.triggerClicks, 0); assert.equal(f.state.resultClicks, 0); assert.equal(f.state.selected, false);
  assert.equal(f.observedErrors[0].code, 'UPLOAD_UI_READINESS_TIMEOUT');
});
