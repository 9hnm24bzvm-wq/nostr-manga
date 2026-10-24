// Source-bound finite fakes only. Actual Schnorr, HTTP, browser and hosted-run
// first failure are outside these controls; the old observer is a counterfactual.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { validateUploadHttpEvent, uploadDigest } from '../../scripts/public-upload-recovery-observer.mjs';
import { restoreReviewedLanguageSource } from './reviewed-local-page-source-successors.mjs';
import {
  accountId, otherAccountId, origin, owner, service, scope, discoveryPath, clientSource,
  actualClientDiscovery, encode, eventFor, seal, harness, finish, observerFailureCases,
} from './public-upload-observer-query-fixture.mjs';

// Signed chapter-affordance successor: requalify this exact current client. The actual
// source-extracted signed discovery tail below remains the exercised contract.
const clientSha = 'c8d3e55319a2f93ae1f9b4e11875c2d4711a1d8d985e4dc7df5ab722f26dca8f';
const baselineSha = '2536bb20ef35db77f697ed30bb1eb14ad00bf8a2828eaa12d490bf32666b4c71';
function input(options = {}) {
  const { method = 'GET', pathname = discoveryPath, body = null, event = eventFor(options), ...overrides } = options;
  return { ...scope, expectedAccountId: accountId, authorization: encode(event), url: origin + pathname, method, body, ...overrides };
}
function rejectedObservation(options) {
  const h = harness();
  try { h.emit(options); assert.throws(h.observer.assertHealthy); assert.equal(h.observer.rows.length, 0);
    assert.deepEqual(h.observer.snapshot().failures, ['signed_request_invalid']); }
  finally { h.observer.close(); }
}

test('exact actual client discovery tail reaches a bound observer without mutation or query retention', async () => {
  assert.equal(uploadDigest(restoreReviewedLanguageSource(clientSource, 'uploadClient')), clientSha);
  const actual = await actualClientDiscovery(); assert.equal(actual.method, 'GET'); assert.equal(actual.body, undefined);
  const h = harness();
  try {
    const request = h.emit({ pathname: actual.url.slice(origin.length), method: actual.method }); finish(h.page, request, 404);
    assert.equal(h.observer.settled(), true); assert.equal(h.observer.mutationCount(), 0);
    const row = h.observer.snapshot().rows[0]; assert.equal(row.path, '/api/v1/upload'); assert.equal(row.status, 404);
    assert.equal(row.native, 'finished');
    for (const marker of [accountId, 'expectedAccountId', 'Nostr ', 'explicit_fake_query_nonce_', 'net::']) assert.ok(!JSON.stringify(h.observer.snapshot()).includes(marker));
  } finally { h.observer.close(); }
});

test('one-time account binding is required for discovery, despite any caller scope override', () => {
  const h = harness({ bind: false, scopeOverride: { expectedAccountId: accountId } });
  try { h.emit(); assert.equal(h.observer.rows.length, 0); assert.throws(h.observer.assertHealthy); }
  finally { h.observer.close(); }
});
for (const value of [undefined, null, 1, '', 'not-a-uuid', accountId.toUpperCase(), accountId + ' ']) test('binding rejects noncanonical value ' + JSON.stringify(value), () => {
  const h = harness({ bind: false });
  try { assert.throws(() => h.observer.bindAccount(value)); h.emit(); assert.throws(h.observer.assertHealthy); assert.equal(h.observer.rows.length, 0); }
  finally { h.observer.close(); }
});
for (const value of [accountId, otherAccountId]) test('binding refuses a second assignment ' + value, () => {
  const h = harness();
  try {
    assert.throws(() => h.observer.bindAccount(value));
    finish(h.page, h.emit()); assert.equal(h.observer.settled(), true);
    h.emit({ pathname: '/api/v1/upload?expectedAccountId=' + otherAccountId, index: 2 });
    assert.throws(h.observer.assertHealthy); assert.equal(h.observer.rows.length, 1);
  } finally { h.observer.close(); }
});
test('binding to the wrong fixture account cannot admit actual client discovery', () => {
  const h = harness({ bind: false });
  try { h.observer.bindAccount(otherAccountId); h.emit(); assert.equal(h.observer.rows.length, 0); assert.throws(h.observer.assertHealthy); }
  finally { h.observer.close(); }
});
test('caller scope mutation cannot retarget the private bound account', () => {
  const h = harness();
  try {
    h.observerScope.expectedAccountId = otherAccountId;
    finish(h.page, h.emit()); assert.equal(h.observer.settled(), true);
    h.emit({ pathname: '/api/v1/upload?expectedAccountId=' + otherAccountId, index: 2 });
    assert.throws(h.observer.assertHealthy); assert.equal(h.observer.rows.length, 1);
  } finally { h.observer.close(); }
});
test('closed observer cannot acquire account binding', () => {
  const h = harness({ bind: false }); h.observer.close(); assert.throws(() => h.observer.bindAccount(accountId));
});
for (const bind of [false, true]) test('queryless reads remain observed with bind=' + bind, () => {
  const h = harness({ bind });
  try {
    for (const [index, pathname] of ['/api/v1/upload', '/api/v1/nostr/account-context'].entries()) finish(h.page, h.emit({ pathname, index }));
    assert.equal(h.observer.rows.length, 2); assert.equal(h.observer.settled(), true);
  } finally { h.observer.close(); }
});

for (const [name, pathname] of [
  ['empty query', '/api/v1/upload?'], ['missing value', '/api/v1/upload?expectedAccountId'],
  ['empty value', '/api/v1/upload?expectedAccountId='], ['foreign account', '/api/v1/upload?expectedAccountId=' + otherAccountId],
  ['malformed account', '/api/v1/upload?expectedAccountId=not-a-uuid'], ['uppercase account', '/api/v1/upload?expectedAccountId=' + accountId.toUpperCase()],
  ['duplicate', discoveryPath + '&expectedAccountId=' + accountId], ['unknown field', discoveryPath + '&extra=1'],
  ['unknown vocabulary', '/api/v1/upload?accountId=' + accountId], ['encoded field', '/api/v1/upload?%65xpectedAccountId=' + accountId],
  ['encoded value', '/api/v1/upload?expectedAccountId=%31' + accountId.slice(1)], ['trailing separator', discoveryPath + '&'],
  ['upload child', '/api/v1/upload/begin?expectedAccountId=' + accountId],
  ['account context', '/api/v1/nostr/account-context?expectedAccountId=' + accountId],
  ['preview', `/api/v1/nostr/uploads/${accountId}/files/${accountId}/preview?expectedAccountId=${accountId}`],
  ['empty fragment', discoveryPath + '#'], ['fragment', discoveryPath + '#anything'],
]) test('bound discovery refuses ' + name + ' before retaining a native row', () => rejectedObservation({ pathname }));
for (const method of ['POST', 'PATCH', 'DELETE']) test('discovery query refuses method ' + method, () => rejectedObservation({ method, body: Buffer.alloc(0) }));
for (const body of [Buffer.from('x'), Buffer.from('{}')]) test('discovery GET refuses actual bytes ' + body.length, () => rejectedObservation({ body }));
test('bound discovery still refuses cookies', () => rejectedObservation({ headers: { cookie: 'synthetic-cookie' } }));
test('bound discovery still refuses oversized actual request bytes', () => rejectedObservation({ body: Buffer.alloc(32769) }));

for (const [name, change] of [
  ['query stripped from u', event => { event.tags[0][1] = origin + '/api/v1/upload'; }],
  ['foreign account in u', event => { event.tags[0][1] = origin + '/api/v1/upload?expectedAccountId=' + otherAccountId; }],
  ['method tag', event => { event.tags[1][1] = 'POST'; }],
  ['service tag', event => { event.tags[2][1] = 'd'.repeat(64); }],
  ['nonce tag', event => { event.tags[3][1] = 'short'; }],
  ['duplicate tag', event => { event.tags.push(['u', event.tags[0][1]]); }],
  ['malformed signature', event => { event.sig = 'not-a-signature'; }],
]) test('bound query retains exact signed envelope check for ' + name, () => {
  const event = eventFor(); change(event); seal(event); rejectedObservation({ event });
});
test('signature refusal cannot become a retained native discovery row', () => {
  const h = harness({ scopeOverride: { verify: () => false } });
  try { h.emit(); assert.equal(h.observer.rows.length, 0); assert.throws(h.observer.assertHealthy); }
  finally { h.observer.close(); }
});
for (const [name, change] of [
  ['signature verifier refusal', value => { value.verify = () => false; }],
  ['foreign origin', value => { value.url = value.url.replace(':50123', ':50124'); }],
  ['missing account scope', value => { delete value.expectedAccountId; }],
  ['malformed account scope', value => { value.expectedAccountId = 'x'; }],
  ['foreign owner', value => { value.owner = 'd'.repeat(64); }],
  ['stale signed event', value => { value.now = Date.now() + 121000; }],
  ['changed event id', value => { const event = eventFor(); event.id = '0'.repeat(64); value.authorization = encode(event); }],
]) test('bound query validator refuses ' + name, () => {
  const value = input(); change(value); assert.throws(() => validateUploadHttpEvent(value));
});
test('full query URL participates in the event id sent to the signature verifier', () => {
  const event = eventFor(); let checked = 0;
  const row = validateUploadHttpEvent(input({ event, verify: (signature, id, key) => {
    checked++; assert.equal(signature, event.sig); assert.equal(id, event.id); assert.equal(key, owner); return true;
  } }));
  assert.equal(checked, 1); assert.equal(row.event_id, event.id);
  const stripped = eventFor({ pathname: '/api/v1/upload' }); assert.notEqual(row.event_id, stripped.id);
});
for (const mode of ['same event', 'same nonce']) test('bound discovery still refuses replayed ' + mode, () => {
  const h = harness();
  try {
    const event = eventFor(); finish(h.page, h.emit({ event }));
    if (mode === 'same nonce') { event.created_at--; seal(event); }
    h.emit({ event }); assert.throws(h.observer.assertHealthy); assert.equal(h.observer.rows.length, 1);
  } finally { h.observer.close(); }
});
for (const nativeError of ['net::ERR_ABORTED', 'synthetic transport failure']) test('bound discovery cannot turn native failure into success: ' + nativeError, () => {
  const h = harness();
  try {
    const request = h.emit({ nativeError }); h.page.emit('response', { request: () => request, status: () => 200 }); h.page.emit('requestfailed', request);
    assert.throws(h.observer.assertHealthy); assert.throws(h.observer.settled);
    const row = h.observer.rows[0]; assert.equal(row.native, 'failed'); assert.equal(row.native_error, nativeError === 'net::ERR_ABORTED' ? nativeError : 'other');
    assert.deepEqual(h.observer.snapshot().failures, ['signed_native_failed']);
  } finally { h.observer.close(); }
});
test('JSON seam contains only actual closed observer failure families', () => {
  assert.deepEqual(observerFailureCases(), { valid_discovery: [], malformed_query: ['signed_request_invalid'],
    duplicate_native_terminal: ['duplicate_native_terminal'], native_abort: ['signed_native_failed'] });
});
test('exact 95944 observer counterfactual rejects actual client discovery as signed_request_invalid', { skip: !process.env.WEEBDEX_UPLOAD_OBSERVER_BASELINE_SOURCE }, async () => {
  const source = readFileSync(process.env.WEEBDEX_UPLOAD_OBSERVER_BASELINE_SOURCE, 'utf8'); assert.equal(uploadDigest(source), baselineSha);
  const dependency = new URL('../../scripts/browser-signed-event-envelope.mjs', import.meta.url).href;
  const baseline = await import('data:text/javascript;base64,' + Buffer.from(source.replace("from './browser-signed-event-envelope.mjs'", 'from ' + JSON.stringify(dependency))).toString('base64'));
  const actual = await actualClientDiscovery(), h = harness({ bind: false, observe: baseline.observePublicUpload });
  try {
    h.emit({ pathname: actual.url.slice(origin.length), method: actual.method }); assert.equal(h.observer.rows.length, 0);
    assert.deepEqual(h.observer.snapshot().failures, ['signed_request_invalid']); assert.throws(h.observer.assertHealthy);
  } finally { h.observer.close(); }
});
