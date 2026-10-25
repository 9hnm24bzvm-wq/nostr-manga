#!/usr/bin/env node
// Exact production HTTP/retained-link and actual outer-journey seams with
// explicit transport, DOM and control stand-ins. No real browser/services.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
import { runUi11To16Journeys, ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { homeFailureDiagnosticChecks, retryDeniedHomeRead } from '../../scripts/browser-home-controls.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';

const pg = ['capability','open','held','begin','outage','first-failure','retry-second','retry-third','three-denials','no-false-empty','resume','restored','recovery-retry','recovery-read'];
const private403 = ['login','logout','request-form','mail','link','cached-session','redemption','restricted-auth','native-denial','widget-open','feedback','single-read','retry','repeated-denial'];
const ids = [...pg.map(id => 'home-pg-' + id), ...private403.map(id => 'home-private403-' + id)];
const fixtureRoot = path.resolve(new URL('../../frontend', import.meta.url).pathname);

test('home inventories are fixed, frozen and match the public gate vocabulary', () => {
  assert.deepEqual(homeFailureDiagnosticChecks, { 'ui14-home-failures': ids });
  assert.deepEqual(ui11To16DiagnosticChecks['ui14-home-failures'], ids);
  assert(Object.isFrozen(homeFailureDiagnosticChecks)); assert(Object.isFrozen(homeFailureDiagnosticChecks['ui14-home-failures']));
});

test('normal reader uses current sidebar heading rather than bubbling-stopped end-page link', () => {
  const end = fs.readFileSync(path.join(fixtureRoot, 'src/lib/components/reader/reader-last-page.svelte'), 'utf8');
  const sidebar = fs.readFileSync(path.join(fixtureRoot, 'src/lib/components/reader/reader-panel.svelte'), 'utf8');
  const title = end.slice(end.indexOf('href="/title/'), end.indexOf('Title', end.indexOf('href="/title/')));
  assert.match(title, /e\.stopPropagation\(\)/); assert.match(title, /e\.stopImmediatePropagation\(\)/);
  const header = sidebar.match(/<Sidebar\.Header>([\s\S]*?)<\/Sidebar\.Header>/)[1];
  const heading = header.match(/<h1\b[^>]*>([\s\S]*?)<\/h1>/)[1];
  assert.equal((heading.match(/<a\b/g) ?? []).length, 1); assert.match(heading, /href="\/title\//); assert.doesNotMatch(heading, /onclick=/);
  const journey = fs.readFileSync(new URL('../../scripts/browser-ui10-journeys.mjs', import.meta.url), 'utf8');
  assert.match(journey, /\[data-sidebar="header"\] h1 a\[href\^="\/title\//);
});

function actualHttp(status) {
  const file = path.join(fixtureRoot, 'src/lib/HTTP.ts');
  let source = createFrontendOverlays(fixtureRoot).transform.handler(fs.readFileSync(file, 'utf8'), file).code;
  source = source.replace(/^\s*import[^\n]+\n/gm, '').replace(/\bexport default HTTP;/, '').replace(/\bexport /g, '');
  let sends = 0;
  class XHR {
    listeners = new Map(); withCredentials = false; status = status; response = JSON.stringify({ code: 'unavailable' });
    addEventListener(name, operation) { this.listeners.set(name, operation); }
    removeEventListener(name) { this.listeners.delete(name); }
    open() {} setRequestHeader() {} abort() {}
    getResponseHeader(name) { return name === 'Content-Type' ? 'application/json' : null; }
    send() { sends++; queueMicrotask(() => this.listeners.get('load')?.()); }
  }
  const HTTP = vm.runInNewContext(stripTypeScriptTypes(source, { mode: 'transform' }) + '\nHTTP;', {
    XMLHttpRequest: XHR, StatusCodes: { TOO_MANY_REQUESTS: 429 }, RateLimiter: class { tryRemoveTokens() { return true; } },
    UserVisibleError: class extends Error { constructor(title, body) { super(body); this.title = title; this.body = body; } },
    FormData, window: { setTimeout(operation, duration) { if (duration < 60000) queueMicrotask(operation); return 1; }, clearTimeout() {} },
  });
  return { HTTP, sends: () => sends };
}
for (const [status, attempts] of [[503, 1], [429, 3], [403, 1]]) test(`actual composed GET settles${status} after${attempts} native response(s)`, async () => {
  const fixture = actualHttp(status), response = await new fixture.HTTP('/api/chapter/updates').GET();
  assert.equal(response.status, status); assert.equal(fixture.sends(), attempts);
});

for (const extra of ['none', 'background-replay', 'wrong-status', 'disabled-idle']) test('explicit retry binds one denied response and idle state: ' + extra, async () => {
  const seen = { rows: [{ status: 503, body: { code: 'unavailable' } }], async settle() {} }; let clicks = 0;
  const retry = { async click() { clicks++; seen.rows.push({ status: extra === 'wrong-status' ? 200 : 503, body: { code: 'unavailable' } }); if (extra === 'background-replay') seen.rows.push(seen.rows.at(-1)); }, async isEnabled() { return extra !== 'disabled-idle'; } };
  const section = { getByRole(role, options) { assert.equal(role, 'button'); assert.deepEqual(options, { name: 'Retry updates', exact: true }); return retry; }, getByText(text, options) { assert.equal(text, 'Updates unavailable'); assert.deepEqual(options, { exact: true }); return { async waitFor() {} }; } };
  const execute = retryDeniedHomeRead(section, seen, async (read, valid, message) => { const value = await read(); assert(valid(value), message); return value; }, assert);
  if (extra === 'none') await execute; else await assert.rejects(execute);
  assert.equal(clicks, 1);
});

function actualStep(label, nextLabel) {
  const source = fs.readFileSync(new URL('../../scripts/browser-ui11-16-journeys.mjs', import.meta.url), 'utf8');
  const start = source.indexOf('await step("' + label); assert(start >= 0);
  const action = source.indexOf('async () => {', start) + 'async () => {'.length;
  const end = nextLabel ? source.indexOf('\n    });\n    await step("' + nextLabel, action) : source.indexOf('\n    });\n  }\n\n  if (phase === "ui15-chapter-filter")', action);
  assert(end > action); return source.slice(action, end);
}
const pgBody = actualStep('UI14 public home Grid: the owned real PostgreSQL outage', 'UI14 private My Updates: cookie-withdrawn');
const privateBody = actualStep('UI14 private My Updates: real restricted recovery cookie');
const AsyncFunction = Object.getPrototypeOf(async function() {}).constructor;

for (const failAt of [undefined, ...pg]) test('actual PG step checkpoint/cleanup: ' + (failAt ?? 'complete'), async () => {
  const failure = Error('private synthetic outage'), report = {}, record = createJourneyCheck(report); let closed = 0, observedClosed = 0, active = false, restores = 0;
  const rows = [], seen = { rows, get requests() { return rows.length; }, async settle() {}, async close() { observedClosed++; } };
  const fault = { fixture_only: true, status: 'outage', active: true, independent_restart_watchdog: true, restart_deadline_unix: Date.now() / 1000 + 15, native503_observed: { status: 503, code: 'unavailable' }, postgres_binary_sha256: 'fixed', cluster_config_hashes: { fixed: 'config' } };
  const restored = { ...fault, status: 'ready', active: false, fixed_cluster_preserved: true, native_recovery: { readyz_status: 200, read_status: 200 } };
  const push = () => rows.push(active ? { status: 503, body: { code: 'unavailable' } } : { status: 200, body: { data: [] } });
  const retry = { async click() { push(); }, async isEnabled() { return true; } };
  const section = { filter() { return this; }, getByRole() { return retry; }, getByText() { return { async waitFor() {} }; } };
  const bindings = {
    fixture: { fixture_control: { pg_home_fault: { fixture_only: true, enabled: true, outage_seconds: 15, independent_restart_watchdog: true, allowed_browser_kind: 'ui14-home-failures' } } },
    phase: 'ui14-home-failures', origin: 'http://127.0.0.1:1', marker: 'synthetic',
    check: (id, operation) => record(id, async () => { if (id === 'home-pg-' + failAt) throw failure; return operation(); }),
    insist(condition, message) { assert(condition, message); }, open: async () => {}, requestPath() {},
    hold: async () => ({ async wait() {}, async release() { push(); }, async close() { closed++; } }), observe: () => seen,
    privateControl: async action => { if (action === 'pg-home-fault-begin') { active = true; return { operation: fault }; } active = false; restores++; return { operation: restored }; },
    page: { getByText: () => ({ last() { return this; }, async waitFor() {} }), locator: () => section, getByRole: () => ({}) },
    bodyHas: async () => false, stable: async (predicate, message) => assert(await predicate(), message),
    until: async (read, valid, message) => { const value = await read(); assert(valid(value), message); return value; }, retryDeniedHomeRead,
  };
  const operation = new AsyncFunction(...Object.keys(bindings), pgBody)(...Object.values(bindings));
  if (failAt) await assert.rejects(operation, error => error === failure); else await operation;
  assert.equal(closed, failAt === 'capability' ? 0 : 1); assert.equal(observedClosed, closed); assert.equal(active, false);
  if (!failAt) { assert.equal(rows.length, 4); assert.equal(restores, 1); }
  assert.deepEqual(report.journey_checks.map(row => row.check_id), (failAt ? pg.slice(0, pg.indexOf(failAt) + 1) : pg).map(id => 'home-pg-' + id));
  assert.doesNotMatch(JSON.stringify(report), /private synthetic|127\.0\.0\.1/);
});

for (const failAt of [undefined, ...private403]) test('actual restricted403 step checkpoint/cleanup: ' + (failAt ?? 'complete'), async () => {
  const failure = Error('private synthetic recovery'), report = {}, record = createJourneyCheck(report);
  let observed = 0, closed = 0, cookieClears = 0, authReads = 0;
  const rows = [], seen = { rows, get requests() { return rows.length; }, async close() { closed++; } };
  const simple = { first() { return this; }, async click() {}, async fill() {}, async waitFor() {} };
  const retry = { async click() { rows.push({ status: 403 }); }, async waitFor() {} };
  const section = { filter() { return this; }, getByRole() { return retry; }, getByText() { return simple; } };
  const bindings = {
    check: (id, operation) => record(id, async () => { if (id === 'home-private403-' + failAt) throw failure; return operation(); }),
    insist(condition, message) { assert(condition, message); }, login: async () => {}, ready: async () => {}, mode: async () => {}, open: async () => {}, realm: async () => {}, sameRealm: async () => {},
    account: { email: 'synthetic@example.invalid' }, origin: 'http://127.0.0.1:1', sink: () => 'http://127.0.0.1:2', remember: value => value,
    api: async route => route === '/api/auth/check' ? (++authReads === 1 ? { status: 401 } : { status: 200, body: { session_purpose: 'recovery' } }) : { status: 403 },
    context: {
      request: { async get(url) { return url.includes('/mail/latest?') ? { ok: () => true, json: async () => ({ fixture_only: true, link: 'http://127.0.0.1:1/auth/self-service/recovery?token=synthetic' }) } : { status: () => 303, headers: () => ({ location: '/auth/recovery' }) }; } },
      async clearCookies() { cookieClears++; },
    },
    page: { getByRole: () => simple, locator: selector => selector === 'section' ? section : simple },
    observe: () => { observed++; return seen; }, requestPath() {}, spaLink: async () => { rows.push({ status: 403 }); },
    until: async (read, valid, message) => { const value = await read(); assert(valid(value), message); return value; },
    stable: async (predicate, message) => assert(await predicate(), message),
  };
  const operation = new AsyncFunction(...Object.keys(bindings), privateBody)(...Object.values(bindings));
  if (failAt) await assert.rejects(operation, error => error === failure); else await operation;
  assert.equal(observed, closed); assert.equal(cookieClears, observed);
  assert.deepEqual(report.journey_checks.map(row => row.check_id), (failAt ? private403.slice(0, private403.indexOf(failAt) + 1) : private403).map(id => 'home-private403-' + id));
  assert.doesNotMatch(JSON.stringify(report), /private synthetic|127\.0\.0\.1|token=|@/);
});
