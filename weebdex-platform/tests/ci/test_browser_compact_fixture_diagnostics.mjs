#!/usr/bin/env node
// Execute the actual compact preflight and recorder. Only browser preference
// setup and HTTP transport are stand-ins; no network, service or database runs.
import assert from 'node:assert/strict';
import { runUi11To16Journeys, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { compactFeedHttpOutcomes, compactFeedHttpStatus, compactFeedDatabaseFailures, compactFeedDatabaseFailure } from '../../scripts/compact-feed-http-outcome.mjs';

const phase = 'ui12-compact-feed', prefix = 'compact-feed-fixture-';
const checks = ['response-received', 'http-200', 'total-integer', 'total-at-least-seeded', 'total-within-known-scenarios'].map(id => prefix + id);
const inventory = ui11To16DiagnosticChecks[phase];
assert.deepEqual(inventory.slice(3, 8), checks);
const privateError = message => Error('private fixture URL and response: ' + message);
let cases = 0;
assert.deepEqual(compactFeedHttpOutcomes, ['success200', 'rate-limited429', 'unavailable503', 'other4xx', 'other5xx', 'otherstatus', 'transportfailure', 'invalidJSON']);
assert(Object.isFrozen(compactFeedHttpOutcomes));
assert(Object.isFrozen(compactFeedDatabaseFailures));
for (const failure of compactFeedDatabaseFailures.slice(2)) assert.equal(compactFeedDatabaseFailure(503, failure), failure);
for (const failure of [undefined, null, '', 'none', 'unavailable', 'query-canceled private fixture', 'PRIVATE', 503, {}, []]) {
  assert.equal(compactFeedDatabaseFailure(503, failure), 'unavailable');
}
assert.equal(compactFeedDatabaseFailure(200, undefined), 'none');
for (const status of [200, 429, 500, undefined]) assert.equal(compactFeedDatabaseFailure(status, 'query-canceled'), 'unavailable');
let classificationCases = 0;
for (let status = 100; status <= 599; status++) {
  const expected = new Map([[200, 'success200'], [429, 'rate-limited429'], [503, 'unavailable503']]).get(status)
    ?? (status >= 500 ? 'other5xx' : status >= 400 ? 'other4xx' : 'otherstatus');
  assert.equal(compactFeedHttpStatus(status), expected); classificationCases++;
}
for (const status of [undefined, null, NaN, Infinity, -1, 0, 600, 429.5, 'private fixture status', {}, []]) {
  assert.equal(compactFeedHttpStatus(status), 'otherstatus'); classificationCases++;
}

async function probe(options) {
  const { status = 200, contentType = 'application/json', bilingual = false, fetchError, jsonError, failedCheck, databaseFailureHeader } = options;
  const body = Object.hasOwn(options, 'body') ? options.body : { total: 505 };
  const report = {}, saved = [], listeners = new Map(), calls = [], steps = [];
  const nextControl = Error('stop before browser-dependent update controls');
  const save = () => saved.push(JSON.parse(JSON.stringify(report)));
  const record = createJourneyCheck(report, save);
  const fixture = { ui_regressions: { fixture_only: true, compact: { total: 505, language: 'es', manga_id: 'synthetic-compact' },
    ...(bilingual ? { bilingual: { manga_id: 'synthetic-bilingual' } } : {}) } };
  const page = {
    on(name, callback) { assert(!listeners.has(name)); listeners.set(name, callback); },
    off(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); }
  };
  const context = { request: { async fetch(url, options) {
    calls.push({ url, options });
    if (fetchError) throw fetchError;
    return { status: () => status, headers: () => ({ 'content-type': contentType, ...(databaseFailureHeader === undefined ? {} : { 'x-weebdex-fixture-feed-failure': databaseFailureHeader }) }), async json() { if (jsonError) throw jsonError; return body; } };
  } } };
  await runUi11To16Journeys({ phase, fixture, page, context, origin: 'http://127.0.0.1:1', report, save, redactions: new Set(),
    insist(value, message) { assert(value, message); },
    check(id, operation) {
      assert(inventory.includes(id));
      return record(id, async () => {
        if (id === 'compact-feed-spanish-preferences') return; // Explicit browser-only prerequisite stand-in.
        if (id === 'compact-feed-open-updates') throw nextControl;
        return operation();
      });
    },
    async step(label, operation) { try { await operation(); assert.fail('The browser-only continuation must not execute'); } catch (error) { steps.push(error); } }
  });
  assert.equal(calls.length, 1, 'The native probe must not retry or manufacture a fallback response');
  assert.equal(calls[0].url, 'http://127.0.0.1:1/api/chapter/feed?tlang=es&limit=100');
  assert.deepEqual(calls[0].options, { maxRedirects: 0, timeout: 12000, headers: { Origin: 'http://127.0.0.1:1' } });
  assert.equal(steps.length, 1);
  assert.equal(listeners.size, 0, 'A successful preflight must still release later observation listeners');
  assert.equal(report.compact_feed_http_outcome, fetchError ? 'transportfailure' : jsonError ? 'invalidJSON' : compactFeedHttpStatus(status));
  assert(compactFeedHttpOutcomes.includes(report.compact_feed_http_outcome));
  assert.equal(report.compact_feed_database_failure, fetchError ? 'unavailable' : compactFeedDatabaseFailure(status, databaseFailureHeader));
  assert.deepEqual(Object.keys(report), ['journey_checks', 'compact_feed_http_outcome', 'compact_feed_database_failure']);
  assert(saved.some(row => row.compact_feed_http_outcome === report.compact_feed_http_outcome
    && row.compact_feed_database_failure === report.compact_feed_database_failure
    && row.journey_checks.some(check => check.check_id === prefix + 'response-received' && check.status === 'running')),
  'The closed outcome must persist before the request checkpoint finishes');
  const rows = new Map(report.journey_checks.map(row => [row.check_id, row.status]));
  assert.equal(rows.size, report.journey_checks.length, 'Every actual check enters once');
  const failedId = failedCheck ? prefix + failedCheck : 'compact-feed-open-updates';
  if (!fetchError && !jsonError) {
    const observedBody = /json/.test(contentType) ? body : undefined;
    const originalAllows = status === 200 && [505, 505 + (bilingual ? 1 : 0)].includes(observedBody?.total);
    assert.equal(!failedCheck, originalAllows, 'Finite diagnostic refinement must preserve the original505/506 acceptance predicate');
  }
  assert.deepEqual(report.journey_checks.map(row => row.check_id), inventory.slice(0, inventory.indexOf(failedId) + 1));
  assert.equal(rows.get(prefix + 'total'), failedCheck ? 'failed' : 'passed');
  for (const id of checks) {
    const position = checks.indexOf(id), failedPosition = checks.indexOf(failedId);
    assert.equal(rows.get(id) ?? 'pending', !failedCheck || position < failedPosition ? 'passed' : position === failedPosition ? 'failed' : 'pending');
  }
  for (const row of saved) {
    for (const check of row.journey_checks) assert.deepEqual(Object.keys(check), ['check_id', 'status']);
    assert(!JSON.stringify(row).includes('private fixture'), 'Recorder must not retain response bodies or exception text');
  }
  assert(saved.some(row => row.journey_checks.some(check => check.check_id === failedId && check.status === 'running')));
  assert(saved.some(row => row.journey_checks.some(check => check.check_id === failedId && check.status === 'failed')));
  if (fetchError || jsonError) assert.equal(steps[0], fetchError ?? jsonError, 'Original transport/JSON failure identity must propagate');
  else if (!failedCheck) assert.equal(steps[0], nextControl);
  cases++;
}

for (const bilingual of [false, true]) await probe({ bilingual });
await probe({ body: { total: 505, private_response: 'private fixture body must not be recorded' } });
await probe({ bilingual: true, body: { total: 506 } });
for (const status of [302, 400, 401, 403, 429, 500, 503]) await probe({ status, failedCheck: 'http-200' });
await probe({ status: 503, contentType: 'text/html', failedCheck: 'http-200' });
for (const databaseFailureHeader of [...compactFeedDatabaseFailures.slice(2), 'private fixture query detail']) {
  await probe({ status: 503, databaseFailureHeader, failedCheck: 'http-200' });
}
await probe({ status: 503, databaseFailureHeader: 'query-canceled', jsonError: privateError('bad JSON with observed database failure'), failedCheck: 'response-received' });
for (const body of [undefined, null, {}, { total: null }, { total: '505' }, { total: 505.5 }, { total: NaN }, { total: Infinity }, { total: Number.MAX_SAFE_INTEGER + 1 }]) {
  await probe({ body, failedCheck: 'total-integer' });
}
await probe({ contentType: 'text/html', failedCheck: 'total-integer' });
for (const total of [-1, 0, 1, 500, 504]) await probe({ body: { total }, failedCheck: 'total-at-least-seeded' });
for (const [total, bilingual] of [[506, false], [507, false], [507, true]]) await probe({ bilingual, body: { total }, failedCheck: 'total-within-known-scenarios' });
await probe({ fetchError: privateError('fetch rejection'), failedCheck: 'response-received' });
const timeout = privateError('request timeout'); timeout.name = 'TimeoutError';
await probe({ fetchError: timeout, failedCheck: 'response-received' });
await probe({ jsonError: privateError('invalid JSON'), failedCheck: 'response-received' });
console.log(JSON.stringify({ status: 'passed', cases, classification_cases: classificationCases, boundary: 'Actual compact fixture preflight, closed HTTP classification and fixed-ID recorder; browser preferences and HTTP transport are explicit stand-ins. No browser, network, service, database or package install.' }));
