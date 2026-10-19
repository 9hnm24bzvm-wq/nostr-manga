// Execute the actual local-library journey callback with explicit transport/DOM
// stand-ins. This starts no browser, app, database or service.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const source = fs.readFileSync(new URL('../../scripts/browser-journeys.mjs', import.meta.url), 'utf8');
const label = "await step('real reader progress persists locally and survives password identity changes', ";
const start = source.indexOf(label);
assert(start >= 0);
const end = source.indexOf("\n\t\tawait step('custom list controls", start);
assert(end > start);
const callback = source.slice(start + label.length, end).trim().replace(/\);$/, '');

async function run(row) {
  const observations = [];
  const snapshot = {records: [{family: 'series', key: 'm', id: 'm', status: 'reading'}, row]};
  const device = async () => structuredClone(snapshot);
  const operation = vm.runInNewContext('(' + callback + ')', {
    chapter: 'c', manga: 'm', origin: 'http://127.0.0.1', ids: {next_chapter: 'next'}, api() { throw Error('Inner continuation is separately exercised'); }, device,
    exerciseReaderContinuation: async () => { observations.push(['continuation']); return {exact_next_unread_chapter: true}; },
    open: async route => observations.push(['open', route]),
    page: {
      locator(selector) { assert.equal(selector, 'img[src^="blob:"]'); return {evaluateAll: async () => 1}; },
      keyboard: {press: async key => { assert.equal(key, 'ArrowRight'); observations.push(['key', key]); }},
      async waitForURL(valid) { assert(valid(new URL('http://127.0.0.1/chapter/c/2'))); }
    },
    async until(read, valid, message) { const value = await read(); assert(valid(value), message); return value; },
    signup: async profile => { assert.equal(profile, 'user'); observations.push(['signup']); },
    openLibraryManagement: async () => observations.push(['manage']),
    panel: () => ({getByText(text, options) {
      assert.equal(text, 'Library source: this browser device.'); assert.equal(options.exact, false);
      return {waitFor: async () => {}};
    }}),
    logout: async () => observations.push(['logout']),
    insist: (condition, message) => assert(condition, message)
  });
  return {value: await operation(), observations};
}

const exact = {family: 'chapters', key: 'c', id: 'c', mangaId: 'm', page: 2, completed: false};
test('actual callback requires and reports exact page-two incomplete progress before identity changes', async () => {
  const {value, observations} = await run(exact);
  assert.equal(value.actual_device_page, 2);
  assert.equal(value.actual_device_completed, false);
  assert.deepEqual(observations, [['open', '/chapter/c/1'], ['key', 'ArrowRight'], ['continuation'], ['signup'], ['manage'], ['logout']]);
});
for (const [name, patch] of [['stale page one', {page: 1}], ['unrelated later page', {page: 3}], ['invented completion', {completed: true}], ['wrong chapter', {id: 'other'}], ['wrong title', {mangaId: 'other'}]]) {
  test('actual callback refuses ' + name, async () => {
    await assert.rejects(run({...exact, ...patch}), /Reader page.*(?:device progress|incomplete device progress)/);
  });
}
