#!/usr/bin/env node
// Actual observer and enclosing journey helper. Explicit driver/response and
// verifier stand-ins isolate harness ownership; no browser/network is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { watchSignedConversationResponses } from '../../scripts/browser-signed-conversation-observer.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
const origin = 'http://127.0.0.1:4000', protocol = 'http://127.0.0.1:4001', id = 'SeedManga1', eventId = 'a'.repeat(64), publisher = 'b'.repeat(64), key = 'c'.repeat(64), namespace = 'private namespace';
const mapping = { kind: 'manga', id, event_id: eventId, publisher, source_kind: 1002 };
function emitter() {
  const listeners = new Map();
  return { on(event, callback) { (listeners.get(event) ?? (listeners.set(event, new Set()), listeners.get(event))).add(callback); },
    off(event, callback) { assert(listeners.get(event)?.delete(callback)); }, emit(event, value) { for (const callback of listeners.get(event) ?? []) callback(value); },
    count() { return [...listeners.values()].reduce((sum, rows) => sum + rows.size, 0); } };
}
function response(page, type, options = {}) {
  const routes = { mapping: origin + '/api/protocol/resources', root: protocol + '/events/' + eventId, feed: protocol + '/social?target=' + eventId + '&limit=50&offset=0' };
  const body = { mapping: { data: [mapping], origin: protocol, service_key: key, catalog_namespace: namespace }, root: { id: eventId, pubkey: publisher, kind: 1002 }, feed: { public: true, events: [] } };
  const request = { url: () => options.url ?? routes[type], method: () => options.method ?? (type === 'mapping' ? 'POST' : 'GET'), postDataJSON: () => options.post ?? { resources: [{ kind: 'manga', id }] } };
  const item = { request: () => request, status: () => options.status ?? 200, headers: () => ({ 'content-type': options.contentType ?? 'application/json' }),
    async body() { if (options.rejectBody) throw Error('private body error'); return Buffer.from(options.bytes ?? JSON.stringify(options.data ?? body[type])); } };
  page.emit('response', item); if (options.finished !== false) page.emit('requestfinished', request);
  return item;
}
const observed = page => watchSignedConversationResponses(page, { nativeOrigin: origin, protocolOrigin: protocol, kind: 'manga', id, eventId });

test('passive observer accepts only exact owned request method/body/path and joins finished decodes', async () => {
  const page = emitter(), watch = observed(page);
  response(page, 'mapping'); // Before the explicit action boundary.
  watch.start();
  response(page, 'mapping', { post: { resources: [{ kind: 'manga', id: 'SeedManga2' }] } });
  response(page, 'mapping', { post: { resources: [{ kind: 'manga', id }], extra: true } });
  response(page, 'root', { url: protocol + '/events/' + 'd'.repeat(64) });
  response(page, 'root', { method: 'POST' });
  for (const type of ['mapping', 'root', 'feed']) response(page, type);
  await watch.close();
  assert.equal(page.count(), 0);
  for (const type of ['mapping', 'root', 'feed']) assert.equal(watch.one(type).status, 200);
  assert.equal(watch.one('mapping').body.data[0].event_id, eventId);
  await watch.close(); assert.throws(() => watch.start(), /already used/);
});

test('missing duplicate unfinished oversized invalid JSON and failed bodies never prove a response', async () => {
  for (const options of [undefined, { duplicate: true }, { finished: false }, { bytes: 'x'.repeat(16385) }, { bytes: '{' }, { contentType: 'text/html' }, { rejectBody: true }]) {
    const page = emitter(), watch = observed(page); watch.start();
    if (options) { response(page, 'root', options); if (options.duplicate) response(page, 'root'); }
    await watch.close(); assert.throws(() => watch.one('root'), /absent, duplicated, unfinished, oversized or invalid JSON/); assert.equal(page.count(), 0);
  }
});

const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const start = source.indexOf('  let firstConversationObserved = false;'), end = source.indexOf('\n  const sendAction =', start);
assert(start > 0 && end > start);
const suffixes = ['conversation-target-controls', 'conversation-mapping-response', 'conversation-root-response', 'conversation-feed-response', 'conversation-source-visible'];
async function probe({ fault, visibilityError, targetError } = {}) {
  const page = emitter(), report = {}, record = createJourneyCheck(report), seen = [], original = Error('private original source visibility error');
  let kindValue = '', idValue = '', clicks = 0;
  const panel = {
    getByLabel(label, options) { assert.equal(label, 'Current instance catalogue ID'); assert.equal(options.exact, true); return { inputValue: async () => idValue }; },
    getByRole(role, options) { assert.equal(role, 'button'); assert.equal(options.name, 'Load verified public conversation'); assert.equal(options.exact, true); return { async click() { clicks++; for (const type of ['mapping', 'root', 'feed']) if (fault !== 'missing-' + type) response(page, type, fault === 'bad-' + type ? { data: {} } : {}); } }; },
    getByText(text, options) { assert.equal(text, 'Verified canonical source: ' + eventId); assert.equal(options.exact, false); return { async waitFor(value) { assert.equal(value.state, 'visible'); assert.equal(value.timeout, 45000); if (visibilityError) throw original; } }; },
  };
  const global = { phase: 'signed-social', page, origin, protocol, fixture: { service_key: key }, seeded: { catalog_namespace: namespace },
    watchSignedConversationResponses, identity: () => panel, nativeSelectForLabel(_panel, label) { assert.equal(label, 'Target type'); return { inputValue: async () => kindValue }; },
    async target(kind, selectedId) { if (targetError) throw original; kindValue = kind; idValue = selectedId; }, identifier: value => /^[0-9a-f]{64}$/.test(value),
    verify(value) { seen.push(value); return value.id === eventId && value.kind === 1002 && value.pubkey === publisher; },
    insist(value, message) { assert(value, message); }, inspect: (suffix, operation) => record('signed-social-' + suffix, operation),
  };
  const context = vm.createContext(global);
  vm.runInContext(source.slice(start, end) + '\nglobalThis.load = loadConversation;', context);
  let error; try { await context.load('manga', id, eventId); } catch (cause) { error = cause; }
  assert.equal(page.count(), 0, 'All observer listeners are released on every boundary');
  assert.doesNotMatch(JSON.stringify(report), /private namespace|body error|original source visibility|http:|SeedManga|aaaa/);
  return { report, error, original, seen, context, clicks: () => clicks };
}

test('actual first-call helper preserves original visibility budget and crypto/binding gates exactly once', async () => {
  const value = await probe(); assert.equal(value.error, undefined);
  assert.deepEqual(value.report.journey_checks, suffixes.map(suffix => ({ check_id: 'signed-social-' + suffix, status: 'passed' })));
  assert.equal(value.seen.length, 1, 'The exact source event still requires cryptographic verification');
  await value.context.load('manga', id, eventId); assert.equal(value.clicks(), 2); assert.equal(value.report.journey_checks.length, suffixes.length, 'Later conversation loads never replay the enrolled first-call diagnostics');
});

test('actual mapping/root/feed and native control failures retain their own gate and cleanup', async () => {
  for (const type of ['mapping', 'root', 'feed']) for (const prefix of ['missing-', 'bad-']) {
    const value = await probe({ fault: prefix + type, visibilityError: true });
    assert(value.error); assert.equal(value.report.journey_checks.at(-1).check_id, 'signed-social-conversation-' + type + '-response'); assert.equal(value.report.journey_checks.at(-1).status, 'failed');
  }
  const target = await probe({ targetError: true }); assert.equal(target.error, target.original); assert.equal(target.clicks(), 0);
  const visible = await probe({ visibilityError: true }); assert.equal(visible.error, visible.original); assert.equal(visible.report.journey_checks.at(-1).check_id, 'signed-social-conversation-source-visible');
});
