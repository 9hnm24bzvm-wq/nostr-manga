#!/usr/bin/env node
// Owned page-event/action lifetimes, with an explicit EventEmitter page stand-in.
// No browser, fixture, network or service is started.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import test from 'node:test';
import { actAndWaitForPageEvent } from '../../scripts/browser-action-events.mjs';
const clean = page => assert.deepEqual(page.eventNames(), []);
for (const event of ['response', 'filechooser', 'download', 'dialog']) {
  test(event + ' is observed before the action, with one owner and cleanup', async () => {
    const page = new EventEmitter(), value = {};
    assert.equal(await actAndWaitForPageEvent(page, event, () => page.emit(event, value)), value); clean(page);
  });
  for (const kind of ['sync', 'async', 'prerequisite']) test(event + ' preserves ' + kind + ' failure and cancels the observer', async () => {
    const page = new EventEmitter(), failure = Error('synthetic action failure');
    await assert.rejects(actAndWaitForPageEvent(page, event, kind === 'async' ? async () => { throw failure; } : () => { throw failure; }, undefined, 5), error => error === failure);
    clean(page);
    // Let the former timeout interval elapse. A detached reject would terminate
    // node under --unhandled-rejections=strict, which is used for this test.
    await new Promise(resolve => setTimeout(resolve, 10)); clean(page);
  });
}
test('missing or cancelled chooser fails finitely and removes every listener', async () => {
  const page = new EventEmitter(); let clicks = 0;
  await assert.rejects(actAndWaitForPageEvent(page, 'filechooser', () => { clicks++; }, undefined, 5), /Timed out waiting/);
  assert.equal(clicks, 1); clean(page);
});
test('observer waits for a matching response and also requires completed action', async () => {
  const page = new EventEmitter(); let complete;
  const action = new Promise(resolve => { complete = resolve; }); let finished = false;
  const waiting = actAndWaitForPageEvent(page, 'response', () => action, value => value.ok).then(value => { finished = true; return value; });
  page.emit('response', { ok: false }); const value = { ok: true }; page.emit('response', value);
  await Promise.resolve(); assert.equal(finished, false); complete(); assert.equal(await waiting, value); clean(page);
});
for (const event of ['close','crash']) test('page ' + event + ' fails the owned operation', async () => {
  const page = new EventEmitter(); await assert.rejects(actAndWaitForPageEvent(page, 'response', () => page.emit(event)), /Page (closed|crashed)/); clean(page);
});
test('predicate failure is preserved and invalid contracts install no observer', async () => {
  const page = new EventEmitter(), failure = Error('predicate');
  await assert.rejects(actAndWaitForPageEvent(page, 'response', () => page.emit('response', {}), () => { throw failure; }), error => error === failure); clean(page);
  await assert.rejects(actAndWaitForPageEvent(page, 'unknown', () => {}), /Invalid/); clean(page);
});
test('ordinary upload and backup callers have no detached Playwright waiter', () => {
  const source = fs.readFileSync(new URL('../../scripts/browser-journeys.mjs', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /page\.waitFor(?:Response|Event)\(/);
  assert.equal((source.match(/actAndWaitForPageEvent\(/g) ?? []).length, 12);
  assert.match(source, /if \(phase === "upload"\)/);
  assert.match(source, /insist\(uploaded\.publication_scope === "instance-local" && uploaded\.publication_status === "local" && uploaded\.data\?\.length === 2/);
});

for (const event of ['timeout', 'close', 'crash', 'predicate']) test(event + ' retains the pending action until it settles', async () => {
  const page = new EventEmitter(); let complete, finished = false;
  const action = new Promise(resolve => { complete = resolve; });
  const waiting = actAndWaitForPageEvent(page, 'response', () => action, event === 'predicate' ? () => { throw Error('predicate failure'); } : undefined, 5).finally(() => { finished = true; });
  // Observe the rejection now too, so the test does not manufacture an orphan.
  const observed = assert.rejects(waiting, /Timed out|closed|crashed|predicate/);
  if (event === 'close' || event === 'crash') page.emit(event);
  if (event === 'predicate') page.emit('response', {});
  await new Promise(resolve => setTimeout(resolve, 10)); assert.equal(finished, false, 'A pending native action still owns this step');
  complete(); await observed; clean(page);
});
test('event failure and a later action rejection are both surfaced after settlement', async () => {
  const page = new EventEmitter(); let rejectAction;
  const action = new Promise((_, reject) => { rejectAction = reject; });
  const failure = Error('late native action failure');
  const observed = assert.rejects(actAndWaitForPageEvent(page, 'response', () => action, undefined, 5), error => error instanceof AggregateError && /Timed out/.test(error.errors[0].message) && error.errors[1] === failure);
  await new Promise(resolve => setTimeout(resolve, 10)); rejectAction(failure); await observed; clean(page);
});
test('dialog acceptance is owned alongside its triggering click', async () => {
  const page = new EventEmitter(), failure = Error('dialog denied');
  await assert.rejects(actAndWaitForPageEvent(page, 'dialog', () => page.emit('dialog', {}), undefined, 5, async () => { throw failure; }), error => error === failure); clean(page);
});
