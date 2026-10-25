// Execute the actual continuity driver and real strict selector helper using
// explicit page/storage/network stand-ins. No browser or IndexedDB is started.
import assert from 'node:assert/strict';
import test from 'node:test';
import {exerciseDeviceContinuity} from '../../scripts/browser-device-continuity.mjs';

function fixture(failAt) {
  const calls = [], listeners = new Map(), frame = {};
  let status = 'reading', offline = false, siblingClosed = false, siblingStatus = 'Reading';
  const text = value => ({reading: 'Reading', on_hold: 'On Hold'})[value];
  function act(name) { calls.push(name); if (name === failAt) throw Error('Synthetic boundary refused'); }
  function browserPage(sibling = false) {
    const events = new Map();
    return {
      url: () => 'http://127.0.0.1:8123/title/SeedManga1',
      async goto(url, options) { assert.equal(url, this.url()); assert.deepEqual(options, {waitUntil: 'domcontentloaded'}); act('sibling-open'); return {status: () => 200}; },
      locator(selector) {
        assert.equal(selector, 'div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible');
        return {filter({hasText}) { return {
          async click() { act(sibling ? 'sibling-trigger' : 'main-trigger'); },
          async waitFor() { act('status-visible'); assert(hasText.test(sibling ? siblingStatus : text(status)), 'Actual observer status must agree'); }
        }; }};
      },
      getByRole(role, {name, exact}) {
        assert.equal(role, 'option'); assert.equal(exact, true);
        return {async click() {
          act(offline ? 'offline-save' : sibling ? 'sibling-save' : 'main-save');
          status = name === 'On Hold' ? 'on_hold' : 'reading';
          if (failAt !== 'missing-broadcast') siblingStatus = text(status);
          if (failAt === 'unexpected-navigation') events.get('framenavigated')?.(frame);
          const mutations = {
            'authority-write': '/api/user/follows/manga/SeedManga1',
            'status-write': '/api/v1/manga/SeedManga1/status',
            'settings-write': '/api/v1/settings',
            'rating-write': '/api/v1/rating',
            'public-command': '/accounts',
            'allowed-analytics': '/api/analytics/manga/SeedManga1/views',
            'queried-analytics': '/api/analytics/manga/SeedManga1/views?unreviewed=1',
            'analytics-other-target': '/api/analytics/manga/OtherManga/views'
          };
          if (mutations[failAt]) listeners.get('request')({method: () => 'POST', url: () => 'http://127.0.0.1:8123' + mutations[failAt]});
          if (failAt === 'foreign-analytics') listeners.get('request')({method: () => 'POST', url: () => 'http://127.0.0.1:8999/api/analytics/manga/SeedManga1/views'});
        }};
      },
      mainFrame: () => frame,
      on(name, listener) { events.set(name, listener); },
      off(name, listener) { assert.equal(events.get(name), listener); events.delete(name); },
      async close() { act('sibling-close'); siblingClosed = true; }
    };
  }
  const page = browserPage(), sibling = browserPage(true);
  const context = {
    on(name, listener) { listeners.set(name, listener); },
    off(name, listener) { assert.equal(listeners.get(name), listener); listeners.delete(name); },
    async newPage() { act('new-page'); return sibling; },
    async setOffline(value) { act(value ? 'offline' : 'online'); offline = value; }
  };
  return {page, context, origin: 'http://127.0.0.1:8123', manga: 'SeedManga1', calls,
    readDevice: async () => ({records: [{family: 'series', id: 'SeedManga1', status: failAt === 'stale-durable-row' ? 'reading' : status}]}),
    outboxCount: async () => failAt === 'public-job' ? 1 : 0,
    async until(read, valid, message) { const value = await read(); assert(valid(value), message); return value; },
    state: () => ({status, offline, siblingClosed, listeners: listeners.size})};
}

test('actual helper exercises bidirectional live tabs and offline durable local write then restores ownership', async () => {
  const f = fixture(), value = await exerciseDeviceContinuity(f);
  assert.equal(value.same_context_tabs, 2); assert.equal(value.offline_mounted_device_write_durable, true);
  assert.equal(value.public_outbox_jobs, 0); assert.equal(value.authority_mutation_requests, 0);
  assert.deepEqual(f.state(), {status: 'reading', offline: false, siblingClosed: true, listeners: 0});
  assert(f.calls.indexOf('offline-save') < f.calls.indexOf('online'));
  assert(f.calls.indexOf('online') < f.calls.indexOf('sibling-close'));
});
test('only the exact same-origin anonymous title analytics path is allowed', async () => {
  const value = await exerciseDeviceContinuity(fixture('allowed-analytics'));
  assert.equal(value.authority_mutation_requests, 0);
});
for (const failAt of ['new-page', 'sibling-open', 'main-save', 'sibling-save', 'offline', 'offline-save', 'missing-broadcast', 'stale-durable-row', 'unexpected-navigation', 'authority-write', 'status-write', 'settings-write', 'rating-write', 'public-command', 'queried-analytics', 'analytics-other-target', 'foreign-analytics', 'public-job']) test('actual helper refuses ' + failAt + ' and releases acquired ownership', async () => {
  const f = fixture(failAt); await assert.rejects(exerciseDeviceContinuity(f));
  assert.equal(f.state().offline, false); assert.equal(f.state().listeners, 0);
  if (failAt !== 'new-page') assert.equal(f.state().siblingClosed, true);
});
for (const failAt of ['online', 'sibling-close']) test('uncertain cleanup cannot pass: ' + failAt, async () => {
  const f = fixture(failAt); await assert.rejects(exerciseDeviceContinuity(f), /cleanup must complete/);
  assert.equal(f.state().listeners, 0);
});
