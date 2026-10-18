// Execute the actual continuation helper with explicit DOM/HTTP/device stand-ins.
// No browser, real request, IndexedDB database or fixture process is started.
import assert from 'node:assert/strict';
import test from 'node:test';
import {exerciseReaderContinuation} from '../../scripts/browser-reader-continuation.mjs';

function fixture(fault) {
  const origin = 'http://127.0.0.1:8123', manga = 'SeedManga1', chapter = 'SeedChapt1', nextChapter = 'SeedChapt2';
  const calls = []; let route = `/chapter/${chapter}/2`, complete = false, continued = false;
  const details = id => ({status: 200, body: {id, language: 'en', relationships: {manga: {id: manga}}, data: [1, 2, 3]}});
  return {origin, manga, chapter, nextChapter, calls,
    api: async path => { const value = details(path.split('/').at(-1)); if (fault === 'foreign-title') value.body.relationships.manga.id = 'OtherManga'; if (fault === 'wrong-language') value.body.language = 'ja'; if (fault === 'missing-pages') value.body.data = []; return value; },
    open: async path => { calls.push(path); route = path; },
    page: {
      url: () => origin + route,
      keyboard: {async press(key) { assert.equal(key, 'ArrowRight'); calls.push(key); route = `/chapter/${chapter}/3`; complete = true; }},
      async waitForURL(valid) { assert(valid(new URL(origin + route)), 'Exact destination must match'); },
      locator(selector) { assert.equal(selector, 'img[src^="blob:"]'); return {evaluateAll: async () => fault === 'blank-image' ? 0 : 1}; },
      getByRole(role, options) { assert.equal(role, 'button'); assert.deepEqual(options, {name: 'Continue Reading', exact: true}); return {
        waitFor: async () => {}, count: async () => fault === 'duplicate-control' ? 2 : 1,
        async click() { calls.push('Continue Reading'); continued = true; route = `/chapter/${fault === 'wrong-destination' ? chapter : nextChapter}/1`; }
      }; }
    },
    readDevice: async () => ({records: [
      {family: 'chapters', id: chapter, mangaId: manga, page: complete ? 3 : 2, completed: fault === 'missing-completion' ? false : complete, readAt: complete ? continued && fault === 'rewritten-read-time' ? 'new-time' : 'actual-time' : undefined},
      ...(continued ? [{family: 'chapters', id: nextChapter, mangaId: manga, page: fault === 'wrong-saved-page' ? 2 : 1, completed: fault === 'invented-next-completion', ...(fault === 'invented-next-completion' ? {readAt: 'not-read'} : {})}] : [])
    ]}),
    async until(read, valid, message) { const value = await read(); assert(valid(value), message); return value; }
  };
}

test('actual controls persist completion then continue to the exact next unread page one', async () => {
  const f = fixture(), value = await exerciseReaderContinuation(f);
  assert.equal(value.actual_last_page, 3); assert.equal(value.completed_locally, true);
  assert.equal(value.next_page, 1); assert.equal(value.next_completed, false);
  assert.deepEqual(f.calls, ['ArrowRight', '/title/SeedManga1?tlang=en', 'Continue Reading']);
});
for (const fault of ['foreign-title', 'wrong-language', 'missing-pages', 'blank-image', 'duplicate-control', 'missing-completion', 'wrong-destination', 'wrong-saved-page', 'invented-next-completion', 'rewritten-read-time']) test('actual helper rejects ' + fault, async () => {
  await assert.rejects(exerciseReaderContinuation(fixture(fault)));
});
