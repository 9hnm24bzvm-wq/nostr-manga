// Prospective real-browser coverage of the three reviewed user-facing fixes.
// Each fixed case settles through the existing step recorder independently.
import assert from 'node:assert/strict';
import { publicBackupFeatureCases } from './public-backup-feature-browser.mjs';
import { publicSearchFeatureCases } from './public-search-feature-browser.mjs';
import { publicChapterFeatureCases } from './public-chapter-feature-browser.mjs';

export const publicFeatureRecoveryPhase = 'public-feature-recovery';
export const publicFeatureCaseIds = Object.freeze([
  'backup-v2-merge-roundtrip',
  'backup-v2-foreign-replacement-cancel',
  'public-search-complete-tag-options',
  'public-search-default-sort-ascending',
  'public-search-empty-rating-selection',
  'chapter-edit-entry-signed',
]);
const cases = Object.freeze([...publicBackupFeatureCases, ...publicSearchFeatureCases, ...publicChapterFeatureCases]);

export async function runPublicFeatureRecovery(context) {
  const { phase, report, step, save } = context;
  assert.equal(phase, publicFeatureRecoveryPhase);
  assert.deepEqual(cases.map(value => value.id), [...publicFeatureCaseIds], 'Exact reviewed feature case order');
  assert.ok(cases.every(value => typeof value.run === 'function'));
  assert.equal(report.steps.length, 0, 'Feature coverage requires a fresh browser phase');
  const result = { schema: 1, scope: 'reviewed-user-features-v1', checks: [], completed: false, actual_context_closed: false };
  report.public_features = result; save();
  for (const value of cases) {
    const index = report.steps.length;
    await step('Feature ' + value.id, () => value.run(context));
    assert.equal(report.steps.length, index + 1, 'Each feature case owns exactly one terminal step');
    const status = report.steps[index].status;
    assert.ok(status === 'passed' || status === 'failed', 'An unresolved feature case cannot advance');
    result.checks.push({ id: value.id, status }); save();
  }
  // Completion means every independent case was attempted. The outer verdict
  // and closed projection still require every case and all cleanup gates pass.
  result.completed = true; save();
}
