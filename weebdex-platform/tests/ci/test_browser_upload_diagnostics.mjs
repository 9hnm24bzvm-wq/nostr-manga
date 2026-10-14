#!/usr/bin/env node
// Fixed upload checkpoint wiring; inner UI/transport operations are explicit
// stand-ins. The real outer control flow, prerequisite guards and cleanup run.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { ordinaryUploadDiagnosticChecks } from '../../scripts/browser-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
const source = fs.readFileSync(new URL('../../scripts/browser-journeys.mjs', import.meta.url), 'utf8');
const expected = ['account-ready','begin-control','begin-uncertainty','begin-staging','corrupt-image-input','corrupt-image-feedback','corrupt-archive-input','corrupt-archive-feedback','corrupt-no-persisted-pages','failed-batch-feedback','failed-page-controls','cancel-conflict-retained','cancel-confirmed','stage-selection','stage-metadata','stage-durable','stage-reload','replacement-prerequisite','replacement-invalid-retained','replacement-lost-reply','replacement-durable','commit-prerequisite','commit-response','commit-receipt','published-reader','revert-prerequisite','revert-controls','page-edit-prerequisite','page-edit-controls'].map(id => 'upload-' + id);
const start = source.indexOf('} else if (phase === "upload") {') + '} else if (phase === "upload") {'.length;
const end = source.indexOf('\n\t} else if (phase === "reader")', start);
assert(start > 0 && end > start); const branch = source.slice(start,end);
test('upload has an exact deeply frozen closed diagnostic inventory', () => {
  assert.deepEqual(ordinaryUploadDiagnosticChecks, { upload: expected }); assert(Object.isFrozen(ordinaryUploadDiagnosticChecks)); assert(Object.isFrozen(ordinaryUploadDiagnosticChecks.upload));
  const calls = [...source.matchAll(/uploadCheck\(['"]([^'"]+)/g)].map(match => 'upload-' + match[1]);
  assert.deepEqual(calls, expected); assert.equal(new Set(expected).size, expected.length);
});
async function probe(failAt) {
  const report = {}, recorder = createJourneyCheck(report), steps = [], listeners = new Set(), routes = new Set(); let currentStep = -1, checked = 0, events = 0;
  const error = Error('synthetic fixed checkpoint interruption');
  const remote = { id: 'RemotePag0', original_file_name: 'page1.png', file_hash: 'a'.repeat(64) };
  const stage = { status: 200, body: { id: 'OwnedStage', relationships: { files: [remote] }, page_order: [remote.id], draft: {} } };
  const control = { filter() { return this; }, getByRole() { return this; }, async inputValue() { return 'initial'; } };
  const page = { locator() { return control; }, getByText() { return control; }, getByPlaceholder() { return control; }, on(name) { assert(!listeners.has(name)); listeners.add(name); }, off(name) { assert(listeners.delete(name)); } };
  const context = {
    async route(endpoint) { assert(!routes.has(endpoint)); routes.add(endpoint); },
    async unroute(endpoint) { assert(routes.delete(endpoint)); }
  };
  const check = async (suffix, operation) => recorder('upload-' + suffix, async () => {
    if ('upload-' + suffix === failAt) throw error;
    if (suffix.endsWith('-prerequisite') || suffix === 'commit-receipt') return operation();
    if (suffix === 'stage-durable') return stage;
    if (suffix === 'stage-reload') return 2;
    if (suffix === 'commit-response') return { status: () => 200, async json() { return { id: 'Published1', publication_scope: 'instance-local', publication_status: 'local', data: [{},{}] }; } };
  });
  const env = vm.createContext({ account: {}, uploaded: undefined, origin: 'http://127.0.0.1:1', manga: 'SeedManga1', marker: 'synthetic', Buffer, image: Buffer.from('fixture'), page, context,
    insist(value, message) { assert(value, message); }, open: async () => {}, checked: async () => { checked++; },
    api: async () => currentStep === 2 ? { status:200, body: { id: 'OwnedStage', relationships: { files: [] } } } : stage,
    uploadCheck: check,
    // Any real waiter created outside a skipped checkpoint is a wiring error.
    actAndWaitForPageEvent() { events++; throw Error('Unexpected observer outside checkpoint'); },
    step: async (name, operation) => { currentStep++; try { await operation(); steps.push({ name, passed: true }); } catch (failure) { steps.push({ name, passed:false, failure }); } },
  });
  await vm.runInContext('(async () => {' + branch + '})()', env);
  assert.equal(steps.length, 8); assert.equal(listeners.size, 0); assert.equal(routes.size, 0); assert.equal(events, 0);
  const rows = report.journey_checks; const order = rows.map(row => expected.indexOf(row.check_id));
  assert(order.every((value,index) => value >= 0 && (!index || value > order[index-1])));
  if (failAt) { assert(rows.some(row => row.check_id === failAt && row.status === 'failed')); assert(steps.some(step => step.failure === error)); }
  else { assert(steps.every(step => step.passed)); assert.deepEqual(rows.map(row => row.check_id), expected.slice(1)); assert(rows.every(row => row.status === 'passed')); assert.equal(checked, 1); }
  if (failAt?.startsWith('upload-stage-')) {
    assert(rows.some(row => row.check_id === 'upload-replacement-prerequisite' && row.status === 'failed'));
    assert(rows.some(row => row.check_id === 'upload-commit-prerequisite' && row.status === 'failed'));
    assert(!rows.some(row => row.check_id === 'upload-commit-response')); assert.equal(checked, 0);
  }
}
test('real outer branch retains original steps, guards, checkpoint order and resource cleanup', async () => {
  await probe();
  for (const id of expected.slice(1)) await probe(id);
});
test('registration uses the same fixed checkpoint and truthful failed prerequisites attempt no publication', () => {
  assert.match(source, /phase === "upload" \? uploadCheck\("account-ready", \(\) => signup\("user"\)\)/);
  assert.match(branch, /insist\(uploadStageReady && replacementReady/);
  assert.equal((branch.match(/timeout: 15000/g) ?? []).length, 1);
  assert.equal((branch.match(/25000/g) ?? []).length, 1);
});
