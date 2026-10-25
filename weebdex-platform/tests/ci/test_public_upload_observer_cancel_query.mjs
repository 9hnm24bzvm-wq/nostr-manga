// Source-extracted client tail and finite fake verifier only; no browser/network.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { validateUploadHttpEvent, uploadDigest } from '../../scripts/public-upload-recovery-observer.mjs';
import { restoreCancellationObserver } from './upload-cancel-observer-association.mjs';
import { accountId, otherAccountId, origin, owner, scope, clientSource, encode, eventFor, seal, harness, finish } from './public-upload-observer-query-fixture.mjs';

const session = 'abcdef01-1234-8234-8234-123456789abc';
const path = '/api/v1/upload/' + session;
const pathname = path + '?expectedAccountId=' + accountId;
const options = { method: 'DELETE', pathname, body: Buffer.alloc(0) };
const source = fs.readFileSync(new URL('../../scripts/public-upload-recovery-observer.mjs', import.meta.url), 'utf8');

async function actualCancellation() {
  assert.equal(uploadDigest(clientSource), '366517eb1d56d0e3474cd0d8067ace517b2439d5bb132d090b54570f77b4c980');
  const start = clientSource.indexOf('    const signedPath = '), end = clientSource.indexOf('\n  }\n  function basicSession', start);
  assert(start > 0 && end > start);
  let observed, guards = 0;
  const request = async value => { assert.equal(observed, undefined); observed = value; return { status: 200 }; };
  const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
  await new AsyncFunction('method', 'path', 'owned', 'request', 'guard', 'payload', 'encoder', 'responseType', clientSource.slice(start, end))(
    'DELETE', '/upload/' + session, { account: { actor: accountId, scope: { origin } } }, request, () => { guards++; }, undefined, new TextEncoder(), 'json');
  assert.equal(guards, 1); assert.equal(observed.url, origin + pathname); assert.equal(observed.method, 'DELETE'); assert.equal(observed.body, undefined);
  return observed;
}

test('actual client cancellation is one exact account-fenced mutation with strict native completion', async () => {
  const actual = await actualCancellation();
  for (const body of [null, Buffer.alloc(0)]) {
    const h = harness();
    try {
      const request = h.emit({ method: actual.method, pathname: actual.url.slice(origin.length), body });
      assert.equal(h.observer.mutationCount(), 1); assert.equal(h.observer.settled(), false);
      finish(h.page, request); assert.equal(h.observer.settled(), true);
      const row = h.observer.rows[0]; assert.equal(row.path, path); assert.equal(row.signed_payload_sha256, uploadDigest(Buffer.alloc(0)));
      assert.equal(row.native, 'finished'); assert.equal(row.status, 200);
      for (const privateValue of [accountId, 'expectedAccountId', 'Nostr ', 'explicit_fake_query_nonce_']) assert(!JSON.stringify(h.observer.snapshot()).includes(privateValue));
    } finally { h.observer.close(); }
  }
});

test('exact published predecessor reproduces invalid-request failure for the same actual cancel URL', async () => {
  const before = restoreCancellationObserver(source);
  assert.equal(uploadDigest(before), 'f211b70bc0b9089e79ed48a46291a88204b3b60212cf604db4fb15c7dd8f7340');
  const dependency = new URL('../../scripts/browser-signed-event-envelope.mjs', import.meta.url).href;
  const old = await import('data:text/javascript;base64,' + Buffer.from(before.replace("from './browser-signed-event-envelope.mjs'", 'from ' + JSON.stringify(dependency))).toString('base64'));
  const actual = await actualCancellation(), h = harness({ observe: old.observePublicUpload });
  try {
    h.emit({ ...options, pathname: actual.url.slice(origin.length) });
    assert.deepEqual(h.observer.snapshot().failures, ['signed_request_invalid']);
    assert.equal(h.observer.mutationCount(), 0); assert.throws(h.observer.assertHealthy);
  } finally { h.observer.close(); }
  for (const changed of [source + '\n', source.replace('discovery || cancellation', 'true')]) assert.throws(() => restoreCancellationObserver(changed));
});

function rejected(changes, configuration) {
  const h = harness(configuration);
  try {
    h.emit({ ...options, ...changes }); assert.throws(h.observer.assertHealthy);
    assert.equal(h.observer.mutationCount(), 0); assert.deepEqual(h.observer.snapshot().failures, ['signed_request_invalid']);
  } finally { h.observer.close(); }
}
for (const [name, value] of [
  ['foreign account', path + '?expectedAccountId=' + otherAccountId],
  ['empty query', path + '?'], ['missing value', path + '?expectedAccountId='],
  ['duplicate account', pathname + '&expectedAccountId=' + accountId], ['unknown field', pathname + '&extra=1'],
  ['encoded field', path + '?%65xpectedAccountId=' + accountId],
  ['encoded account', path + '?expectedAccountId=%31' + accountId.slice(1)],
  ['trailing separator', pathname + '&'], ['fragment', pathname + '#'],
  ['malformed UUID', pathname.replace(session, 'a'.repeat(36))],
  ['uppercase UUID', pathname.replace(session, session.toUpperCase())],
  ['file deletion', path + '/' + session + '?expectedAccountId=' + accountId],
  ['batch deletion', path + '/batch?expectedAccountId=' + accountId],
  ['begin', '/api/v1/upload/begin?expectedAccountId=' + accountId],
  ['discovery DELETE', '/api/v1/upload?expectedAccountId=' + accountId],
]) test('cancellation query rejects ' + name, () => rejected({ pathname: value }));
for (const method of ['GET', 'POST', 'PATCH']) test('exact-session query rejects method ' + method, () => rejected({ method }));
test('cancellation rejects actual nonempty body even with a matching payload hash', () => rejected({ body: Buffer.from('{}') }));
test('cancellation requires prior private account binding', () => rejected({}, { bind: false, scopeOverride: { expectedAccountId: accountId } }));
test('cancellation still rejects cookie authority', () => rejected({ headers: { cookie: 'synthetic' } }));
test('cancellation still rejects signature verifier refusal', () => rejected({}, { scopeOverride: { verify: () => false } }));
for (const [name, change] of [
  ['query stripped from signed URL', event => { event.tags[0][1] = origin + path; }],
  ['different signed session', event => { event.tags[0][1] = origin + pathname.replace(session, otherAccountId); }],
  ['method mismatch', event => { event.tags[1][1] = 'POST'; }],
  ['service mismatch', event => { event.tags[2][1] = 'd'.repeat(64); }],
  ['nonempty payload when browser bytes unavailable', event => { event.tags[4][1] = uploadDigest(Buffer.from('{}')); }],
  ['missing payload', event => { event.tags.pop(); }],
  ['duplicate payload', event => { event.tags.push([...event.tags[4]]); }],
]) test('cancellation preserves signed association for ' + name, () => {
  const event = eventFor(options); change(event); seal(event); rejected({ event, body: null });
});
test('exact signed full URL and empty payload reach the verifier unchanged', () => {
  const event = eventFor(options); let checked = 0;
  const row = validateUploadHttpEvent({ ...scope, expectedAccountId: accountId, ...options, url: origin + pathname, authorization: encode(event), verify: (sig, id, key) => {
    checked++; assert.equal(sig, event.sig); assert.equal(id, event.id); assert.equal(key, owner); return true;
  } });
  assert.equal(checked, 1); assert.equal(row.event_id, event.id);
  assert.notEqual(row.event_id, eventFor({ ...options, pathname: path }).id);
});
for (const failure of ['native-abort', 'duplicate-terminal', 'replayed-event']) test('cancellation still refuses ' + failure, () => {
  const h = harness();
  try {
    const request = h.emit(options);
    if (failure === 'native-abort') h.page.emit('requestfailed', request);
    else { finish(h.page, request); if (failure === 'duplicate-terminal') h.page.emit('requestfinished', request); else h.page.emit('request', request); }
    assert.throws(h.observer.assertHealthy); assert.throws(h.observer.settled);
    assert.equal(h.observer.rows.length, 1);
  } finally { h.observer.close(); }
});
