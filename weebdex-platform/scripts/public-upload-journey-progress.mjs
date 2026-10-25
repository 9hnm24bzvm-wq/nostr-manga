// Last fixed substep for each disposable upload check. No labels, URLs, values,
// request data or exception text are copied into the public progress record.
import assert from 'node:assert/strict';
import { keyImportCategories, keyImportFailureCategory } from './public-upload-key-import.mjs';

export const uploadJourneyStages = Object.freeze([
  'start', 'fixture-account', 'navigation', 'key-import', 'account-context',
  'group-trigger', 'group-query', 'group-result', 'group-close', 'begin-control',
  'begin-dispatch', 'browser-terminal', 'current-read', 'snapshot-action',
  'mutation-count', 'exact-read', 'durable-read', 'durable-check', 'snapshot-check',
  'language', 'file-input', 'preview-order', 'preview-decode', 'cancel-dialog',
  'cancel-dismiss', 'cancel-confirm', 'cancel-check', 'metadata-title',
  'metadata-chapter', 'reorder', 'remove', 'replacement-control',
  'replacement-chooser', 'replacement-check', 'reload', 'metadata-read',
  'preview-read', 'commit-control', 'publication-check', 'receipt-read',
  'completion', 'receipt-refresh', 'route', 'complete',
]);
export const uploadJourneyCategories = keyImportCategories;
export function createUploadJourneyProgress(rows, save) {
  assert.ok(Array.isArray(rows) && rows.length === 0);
  assert.equal(typeof save, 'function');
  let current;
  const active = () => {
    assert.ok(current && rows.at(-1) === current && current.status === 'running', 'No running upload check');
    return current;
  };
  return {
    begin(checkIndex) {
      assert.ok(Number.isInteger(checkIndex) && checkIndex >= 0 && checkIndex < 14);
      assert.equal(checkIndex, rows.length);
      assert.ok(rows.every((row, index) => row.check_index === index && row.status === 'passed'));
      current = { check_index: checkIndex, substep: 'start', status: 'running', caught_category: 'none' };
      rows.push(current); save();
    },
    mark(stage) {
      const row = active();
      assert.ok(typeof stage === 'string' && uploadJourneyStages.includes(stage) && !['start', 'complete'].includes(stage), 'Unknown upload substep');
      row.substep = stage; save();
    },
    passed() {
      const row = active(), previous = row.substep;
      row.substep = 'complete'; row.status = 'passed';
      try { save(); } catch (error) { row.substep = previous; row.status = 'running'; throw error; }
    },
    failed(error) {
      // Recording or persisting a diagnostic must not replace the first error,
      // including failure before begin() admitted a current check.
      if (!current || rows.at(-1) !== current || current.status !== 'running') return error;
      current.status = 'failed'; current.caught_category = keyImportFailureCategory(error);
      try { save(); } catch { /* Preserve the original action/persistence error. */ }
      return error;
    },
  };
}
