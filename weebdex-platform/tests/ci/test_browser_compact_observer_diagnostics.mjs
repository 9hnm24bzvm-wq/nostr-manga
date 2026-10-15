#!/usr/bin/env node
// Actual observer and compact-phase wiring with explicit event-driven stand-ins.
// No browser, service, network, database, timers, installs or E2E claims.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import test from 'node:test';
import { observeUi11To16Requests, runUi11To16Journeys, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';

const origin = 'http://127.0.0.1:1', secret = 'PRIVATE_credentials_query_body_error';
const matches = value => new URL(value.url()).pathname === '/api/chapter/feed';
function request(page, timing = { startTime: 1000, responseEnd: 10 }) {
  return { url: () => `${origin}/api/chapter/feed?page=${page}&private=${secret}`, method: () => 'GET', timing: () => ({ ...timing, private: secret }) };
}
function response(source, body = { page: Number(new URL(source.url()).searchParams.get('page')), private: secret }, status = 200) {
  return { request: () => source, url: source.url, status: () => status, headers: () => ({ 'content-type': 'application/json', private: secret }), json: async () => body };
}
async function complete(page, seen, source, body) {
  page.emit('request', source); page.emit('response', response(source, body));
  await seen.settle(); page.emit('requestfinished', source);
}
function privateFree(observation) {
  assert.doesNotMatch(JSON.stringify(observation), /PRIVATE|127\.0\.0\.1|chapter\/feed|private|http:|Error/);
  assert.deepEqual(Object.keys(observation), ['maximum', 'request_count', 'pages', 'statuses', 'request_limit', 'event_limit', 'requests', 'truncated', 'unavailable']);
  for (const record of observation.requests) {
    assert.deepEqual(Object.keys(record), ['id', 'page', 'response_page', 'status', 'timing', 'events']);
    assert.deepEqual(Object.keys(record.timing), ['startTime', 'responseEnd']);
    for (const event of record.events) assert.deepEqual(Object.keys(event), ['sequence', 'event', 'startTime', 'responseEnd']);
  }
}

test('diagnostics opt in; original rows, pending settlement, active lifetime and cleanup remain intact', async () => {
  const page = new EventEmitter(), seen = observeUi11To16Requests(page, matches), source = request(1);
  await complete(page, seen, source);
  assert.equal(seen.observation(), undefined); assert.equal(seen.requests, 1); assert.equal(seen.maximum, 1);
  assert.equal(seen.rows[0].body.private, secret); assert.match(seen.rows[0].query, /PRIVATE/);
  assert.equal(seen.rows[0].method, 'GET'); assert.equal(seen.rows[0].pathname, '/api/chapter/feed');
  await seen.close(); assert.equal(page.eventNames().length, 0);
});

test('real emitted duplicate pages and response-before-finished overlap remain distinguishable', async () => {
  const page = new EventEmitter(), seen = observeUi11To16Requests(page, matches, { compactDiagnostics: true });
  const first = request(2, { startTime: 1000, responseEnd: 8 }), next = request(2, { startTime: 1010, responseEnd: 5 });
  page.emit('request', first); page.emit('response', response(first)); await seen.settle();
  page.emit('request', next); page.emit('requestfinished', first); page.emit('response', response(next));
  await seen.settle(); page.emit('requestfinished', next);
  const observed = seen.observation(); privateFree(observed);
  assert.deepEqual(observed.pages, [2, 2]); assert.deepEqual(observed.statuses, [200, 200]);
  assert.equal(observed.maximum, 2); assert.equal(observed.request_count, 2);
  assert.deepEqual(observed.requests[0].events.map(row => row.event), ['request', 'response', 'body-parsed', 'requestfinished']);
  assert(observed.requests[1].events[0].sequence < observed.requests[0].events.at(-1).sequence);
  assert.deepEqual(observed.requests.map(row => row.timing), [{ startTime: 1000, responseEnd: 8 }, { startTime: 1010, responseEnd: 5 }]);
  assert.deepEqual(observed.unavailable, { request_event: false, request_page: false, response_page: false, status: false, timing: false });
  assert.deepEqual(observed.truncated, { requests: false, events: false, rows: false });
  await seen.close(); assert.equal(page.eventNames().length, 0);
});

test('requestfinished can precede body parsing; timing is sampled again after response', async () => {
  const page = new EventEmitter(), seen = observeUi11To16Requests(page, matches, { compactDiagnostics: true });
  let finishBody; const body = new Promise(resolve => { finishBody = resolve; });
  const timing = { startTime: 1234, responseEnd: -1 }, source = request(3, timing), reply = response(source); reply.json = () => body;
  page.emit('request', source); page.emit('response', reply); timing.responseEnd = 20; page.emit('requestfinished', source);
  assert.equal(seen.rows.length, 0); finishBody({ page: 3, private: secret }); await seen.settle();
  const observed = seen.observation(); privateFree(observed);
  assert.deepEqual(observed.requests[0].events.map(row => row.event), ['request', 'response', 'requestfinished', 'body-parsed']);
  assert.equal(observed.requests[0].events[1].responseEnd, null); assert.equal(observed.requests[0].events[2].responseEnd, 20);
  assert.deepEqual(observed.pages, [3]); await seen.close();
});

test('32-request/row cap and finite event cap retain counts and mark truncation without changing rows', async () => {
  const page = new EventEmitter(), seen = observeUi11To16Requests(page, matches, { compactDiagnostics: true });
  for (let number = 1; number <= 40; number++) await complete(page, seen, request(number));
  let observed = seen.observation(); privateFree(observed);
  assert.equal(observed.requests.length, 32); assert.equal(observed.pages.length, 32); assert.equal(observed.statuses.length, 32);
  assert.equal(observed.request_count, 40); assert.equal(seen.rows.length, 40);
  assert.deepEqual(observed.truncated, { requests: true, events: false, rows: true });
  const first = request(41); // Omitted requests never create diagnostic records.
  page.emit('request', first); page.emit('requestfailed', first);
  assert.equal(seen.observation().requests.length, 32); await seen.close();
  const cappedPage = new EventEmitter(), capped = observeUi11To16Requests(cappedPage, matches, { compactDiagnostics: true }), source = request(1);
  cappedPage.emit('request', source);
  for (let index = 0; index < 300; index++) cappedPage.emit('requestfinished', source);
  cappedPage.emit('response', response(source)); await capped.settle(); observed = capped.observation(); privateFree(observed);
  assert.equal(observed.requests.flatMap(row => row.events).length, 256); assert.equal(observed.truncated.events, true);
  assert.equal(observed.requests[0].status, 200); assert.equal(observed.requests[0].response_page, 1); await capped.close();
});

test('failed/missing timing and nonnumeric data yield closed unavailable flags, never error text', async () => {
  const page = new EventEmitter(), seen = observeUi11To16Requests(page, matches, { compactDiagnostics: true });
  const source = request(secret); source.timing = () => { throw Error(secret); };
  page.emit('request', source); page.emit('requestfailed', source);
  const observed = seen.observation(); privateFree(observed);
  assert.deepEqual(observed.requests[0].events.map(row => row.event), ['request', 'requestfailed']);
  assert.equal(observed.requests[0].page, null); assert.deepEqual(observed.requests[0].timing, { startTime: null, responseEnd: null });
  assert.equal(observed.unavailable.request_page, true); assert.equal(observed.unavailable.response_page, true); assert.equal(observed.unavailable.status, true); assert.equal(observed.unavailable.timing, true);
  await seen.close();
});

for (const parallel of [false, true]) test('actual compact wiring saves before unchanged predicate and again in finally: ' + parallel, async () => {
  const page = new EventEmitter(), report = {}, snapshots = [], sentinel = Error('stop after actual unique/sequential predicate');
  const save = () => snapshots.push(structuredClone(report)), record = createJourneyCheck(report, save);
  let finalRequest, failure;
  async function emit(number, finish = true) {
    const source = request(number); page.emit('request', source); page.emit('response', response(source));
    await Promise.resolve(); await Promise.resolve(); if (finish) page.emit('requestfinished', source); return source;
  }
  page.getByRole = (_kind, options) => options.name === 'Load more updates' ? { async waitFor() {}, async click() { await emit(6); if (finalRequest) page.emit('requestfinished', finalRequest); } } : { async count() { return 0; } };
  page.locator = () => ({ last: () => ({ async boundingBox() { return { y: 200 }; } }) });
  await runUi11To16Journeys({ phase: 'ui12-compact-feed', page, report, save, origin, redactions: new Set(),
    fixture: { ui_regressions: { fixture_only: true, compact: { total: 505, language: 'es', manga_id: 'synthetic' } } },
    context: { request: { async fetch() { return { status: () => 200, headers: () => ({ 'content-type': 'application/json' }), json: async () => ({ total: 505 }) }; } } },
    insist(value, message) { assert(value, message); },
    async open() { for (let number = 1; number <= 5; number++) finalRequest = await emit(number, !(parallel && number === 5)); },
    check(id, action) { assert(ui11To16DiagnosticChecks['ui12-compact-feed'].includes(id)); return record(id, async () => { if (id === 'compact-feed-spanish-preferences') return; if (id === 'compact-feed-scenario-membership') throw sentinel; return action(); }); },
    async step(_label, action) { try { await action(); } catch (error) { failure = error; } }
  });
  assert.deepEqual(report.compact_feed_observation.pages, [1, 2, 3, 4, 5, 6]);
  assert.equal(report.compact_feed_observation.maximum, parallel ? 2 : 1);
  const unique = report.journey_checks.find(row => row.check_id === 'compact-feed-unique-sequential-pages');
  assert.equal(unique.status, parallel ? 'failed' : 'passed');
  if (parallel) assert.match(failure.message, /duplicate or parallel/); else assert.equal(failure, sentinel);
  const before = snapshots.findIndex(row => row.compact_feed_observation && !row.journey_checks.some(check => check.check_id === unique.check_id));
  assert(before >= 0, 'Numeric evidence must persist before entering the original predicate');
  assert(snapshots.at(-1).compact_feed_observation); privateFree(report.compact_feed_observation);
  assert.equal(page.eventNames().length, 0);
});

test('original unique/sequential assertion and fixed diagnostic identity remain exact', () => {
  const source = fs.readFileSync(new URL('../../scripts/browser-ui11-16-journeys.mjs', import.meta.url), 'utf8');
  assert.equal(source.split('new Set(pages).size === pages.length && seen.maximum === 1').length - 1, 1);
  assert.equal(ui11To16DiagnosticChecks['ui12-compact-feed'].filter(id => id === 'compact-feed-unique-sequential-pages').length, 1);
});
