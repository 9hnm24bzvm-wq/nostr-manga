// Explicit progress/persistence stand-ins. No browser, service, crypto or DB.
import assert from 'node:assert/strict';
import test from 'node:test';
import { createUploadJourneyProgress, uploadJourneyStages, uploadJourneyCategories } from '../../scripts/public-upload-journey-progress.mjs';

function fixture(failSaves = []) {
  const rows = [], snapshots = []; let saves = 0;
  const failure = Error('synthetic persistence failure');
  const progress = createUploadJourneyProgress(rows, () => {
    snapshots.push(structuredClone(rows));
    if (failSaves.includes(++saves)) throw failure;
  });
  return { rows, snapshots, progress, failure };
}

test('fourteen exact checks retain only fixed fields and terminal completion', () => {
  const f = fixture();
  for (let index = 0; index < 14; index++) {
    f.progress.begin(index); f.progress.mark('route'); f.progress.passed();
  }
  assert.equal(f.snapshots.length, 42);
  assert.equal(f.rows.length, 14);
  for (const [index, row] of f.rows.entries()) assert.deepEqual(row,
    { check_index: index, substep: 'complete', status: 'passed', caught_category: 'none' });
  assert.throws(() => f.progress.begin(14));
  assert.throws(() => f.progress.mark('route'));
  assert.throws(() => f.progress.passed());
});

test('closed stage and category lists have no dynamic values', () => {
  assert.equal(uploadJourneyStages.length, 44);
  assert.equal(new Set(uploadJourneyStages).size, 44);
  assert.ok(Object.isFrozen(uploadJourneyStages));
  assert.deepEqual([...uploadJourneyCategories], ['none', 'readiness-timeout', 'duplicate-control', 'browser-timeout', 'assertion', 'unexpected']);
  const f = fixture(); f.progress.begin(0);
  for (const stage of ['start', 'complete', 'reserved-example.invalid/private-value', undefined, null, {}, 0])
    assert.throws(() => f.progress.mark(stage));
  assert.equal(f.rows[0].substep, 'start');
});

test('begin requires exact numeric order and an empty owned initial ledger', () => {
  assert.throws(() => createUploadJourneyProgress([{}], () => {}));
  assert.throws(() => createUploadJourneyProgress({}, () => {}));
  assert.throws(() => createUploadJourneyProgress([], null));
  for (const index of [true, false, -1, 1, 0.5, 14, '0', null]) {
    const f = fixture(); assert.throws(() => f.progress.begin(index)); assert.deepEqual(f.rows, []);
  }
  const f = fixture(); f.progress.begin(0);
  assert.throws(() => f.progress.begin(1));
  assert.equal(f.rows.length, 1);
});

for (const [error, category] of [
  [Object.assign(Error('synthetic'), { code: 'UPLOAD_UI_READINESS_TIMEOUT' }), 'readiness-timeout'],
  [Object.assign(Error('synthetic'), { code: 'UPLOAD_UI_DUPLICATE_CONTROL' }), 'duplicate-control'],
  [Object.assign(Error('synthetic'), { name: 'TimeoutError' }), 'browser-timeout'],
  [Object.assign(Error('synthetic'), { code: 'ERR_ASSERTION' }), 'assertion'],
  [Error('reserved-example.invalid/private-value'), 'unexpected'],
]) test('failure preserves the first exception and closed category ' + category, () => {
  const f = fixture(); f.progress.begin(0); f.progress.mark('group-trigger');
  assert.equal(f.progress.failed(error), error);
  assert.deepEqual(f.rows, [{ check_index: 0, substep: 'group-trigger', status: 'failed', caught_category: category }]);
  assert.throws(() => f.progress.begin(1)); assert.throws(() => f.progress.mark('route'));
  assert.throws(() => f.progress.passed());
  assert.doesNotMatch(JSON.stringify(f.rows), /reserved-example|private-value|synthetic/);
});

for (const saveAt of [1, 2, 3]) test('persistence failure remains the primary exception at save ' + saveAt, () => {
  const f = fixture([saveAt, saveAt + 1]); let observed;
  try { f.progress.begin(0); f.progress.mark('group-query'); f.progress.passed(); }
  catch (error) { observed = error; assert.equal(f.progress.failed(error), error); }
  assert.equal(observed, f.failure);
  assert.equal(f.rows[0].status, 'failed');
  assert.equal(f.rows[0].substep, saveAt === 1 ? 'start' : 'group-query');
  assert.equal(f.rows[0].caught_category, 'unexpected');
  assert.notEqual(f.rows[0].substep, 'complete');
});

test('secondary failed-record persistence cannot replace an action error', () => {
  const f = fixture([3]), primary = Error('synthetic primary');
  f.progress.begin(0); f.progress.mark('group-result');
  assert.equal(f.progress.failed(primary), primary);
  assert.equal(f.rows[0].status, 'failed');
  assert.equal(f.rows[0].substep, 'group-result');
});

test('throwing error metadata is generic and cannot leak or replace the error', () => {
  let reads = 0;
  const primary = Object.defineProperty({}, 'code', { get() { reads++; throw Error('reserved-example.invalid'); } });
  const f = fixture(); f.progress.begin(0); f.progress.mark('route');
  assert.equal(f.progress.failed(primary), primary); assert.equal(reads, 1);
  assert.equal(f.rows[0].caught_category, 'unexpected');
  assert.doesNotMatch(JSON.stringify(f.snapshots), /reserved-example/);
});

test('failure before begin or after a passed check does not rewrite terminal state', () => {
  const f = fixture(), error = Error('synthetic');
  assert.equal(f.progress.failed(error), error); assert.deepEqual(f.rows, []);
  f.progress.begin(0); f.progress.mark('route'); f.progress.passed();
  assert.equal(f.progress.failed(error), error);
  assert.equal(f.rows[0].status, 'passed');
});
