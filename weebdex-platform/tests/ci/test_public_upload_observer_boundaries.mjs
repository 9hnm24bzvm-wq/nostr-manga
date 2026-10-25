// Finite, explicit EventEmitter/fetch stand-ins. No browser, socket, signature
// verification, database or application journey is executed by these tests.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import {
  createUploadIndependentReader,
  observePublicUpload,
  uploadDigest,
  validateUploadHttpEvent,
} from '../../scripts/public-upload-recovery-observer.mjs';

const origin = 'http://127.0.0.1:50123';
const owner = 'a'.repeat(64), service = 'b'.repeat(64);
const scope = { origin, owner, service, verify: () => true };
const encode = event => 'Nostr ' + Buffer.from(JSON.stringify(event)).toString('base64');
function eventFor({ method = 'GET', pathname = '/api/v1/upload', body = null, index = 1 } = {}) {
  const event = {
    kind: 27235, created_at: Math.floor(Date.now() / 1000), content: '', pubkey: owner,
    tags: [['u', origin + pathname], ['method', method], ['service', service],
      ['nonce', 'explicit_fake_nonce_' + index],
      ...(method === 'GET' ? [] : [['payload', uploadDigest(body ?? Buffer.alloc(0))]])],
    sig: 'c'.repeat(128),
  };
  event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content]));
  return event;
}
function requestFor(options = {}) {
  const { method = 'GET', pathname = '/api/v1/upload', body = null, headers = {}, index = 1 } = options;
  const event = options.event ?? eventFor({ method, pathname, body, index });
  return {
    url: () => origin + pathname, method: () => method,
    headers: () => ({ authorization: encode(event), ...headers }),
    postDataBuffer: () => body, failure: () => ({ errorText: 'net::ERR_ABORTED' }),
  };
}
function harness() {
  const page = new EventEmitter(), observer = observePublicUpload(page, scope);
  return { page, observer, emit: options => { const request = requestFor(options); page.emit('request', request); return request; } };
}
function finish(page, request, status = 200) {
  page.emit('response', { request: () => request, status: () => status });
  page.emit('requestfinished', request);
}
const fakeSign = async template => ({ ...template, pubkey: owner, id: 'e'.repeat(64), sig: 'f'.repeat(128) });
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }

test('passive observer requires both actual HTTP status and native completion', () => {
  const h = harness();
  try {
    const request = h.emit(); assert.equal(h.observer.settled(), false);
    h.page.emit('requestfinished', request); assert.equal(h.observer.settled(), false);
    h.page.emit('response', { request: () => request, status: () => 200 });
    assert.equal(h.observer.settled(), true);
    assert.equal(h.observer.rows[0].status, 200);
  } finally { h.observer.close(); }
});

test('passive observer records an actual native-finished HTTP refusal without inventing success', () => {
  const h = harness();
  try { const request = h.emit(); finish(h.page, request, 403); assert.equal(h.observer.settled(), true); assert.equal(h.observer.rows[0].status, 403); }
  finally { h.observer.close(); }
});

for (const terminal of ['requestfinished', 'requestfailed']) test('duplicate ' + terminal + ' poisons observation', () => {
  const h = harness();
  try {
    const request = h.emit(); finish(h.page, request);
    h.page.emit(terminal, request);
    assert.throws(h.observer.assertHealthy);
    assert.ok(h.observer.snapshot().failures.includes('duplicate_native_terminal'));
  } finally { h.observer.close(); }
});

for (const mode of ['same event', 'same nonce with different event']) test('replay detection refuses ' + mode, () => {
  const h = harness();
  try {
    const event = eventFor(); h.emit({ event });
    if (mode === 'same nonce with different event') {
      event.tags[0][1] = origin + '/api/v1/upload/anything';
      event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content]));
      h.emit({ event, pathname: '/api/v1/upload/anything' });
    } else h.emit({ event });
    assert.equal(h.observer.rows.length, 1); assert.throws(h.observer.assertHealthy);
  } finally { h.observer.close(); }
});

for (const [name, options] of [
  ['cookie authority', { headers: { cookie: 'explicit-fake-cookie' } }],
  ['oversized observed body', { method: 'POST', body: Buffer.alloc(32769) }],
  ['query on tracked route', { pathname: '/api/v1/upload?token=explicit-fake' }],
  ['GET with bytes', { body: Buffer.from('{}') }],
]) test('passive observer rejects ' + name, () => {
  const h = harness();
  try { h.emit(options); assert.throws(h.observer.assertHealthy); assert.equal(h.observer.rows.length, 0); }
  finally { h.observer.close(); }
});

test('observer request boundary is exactly 128 retained signed rows', () => {
  const h = harness();
  try {
    for (let index = 0; index < 128; index++) finish(h.page, h.emit({ index }));
    assert.equal(h.observer.rows.length, 128); assert.equal(h.observer.settled(), true);
    h.emit({ index: 128 }); assert.equal(h.observer.rows.length, 128); assert.throws(h.observer.assertHealthy);
  } finally { h.observer.close(); }
});

test('observer mutation boundary is exactly 32 and never admits the next write', () => {
  const h = harness();
  try {
    for (let index = 0; index < 32; index++) finish(h.page, h.emit({ index, method: 'POST', body: Buffer.from('{}') }));
    assert.equal(h.observer.mutationCount(), 32); assert.equal(h.observer.settled(), true);
    h.emit({ index: 32, method: 'POST', body: Buffer.from('{}') });
    assert.equal(h.observer.mutationCount(), 32); assert.throws(h.observer.assertHealthy);
  } finally { h.observer.close(); }
});

test('observer close removes only owned listeners, is idempotent and leaves detached snapshots', () => {
  const h = harness(), unrelated = () => {};
  h.page.on('request', unrelated); const request = h.emit(); finish(h.page, request);
  const snapshot = h.observer.snapshot(); snapshot.rows[0].status = 599; snapshot.failures.push('fake');
  assert.equal(h.observer.rows[0].status, 200); h.observer.assertHealthy();
  h.observer.close(); h.observer.close();
  assert.equal(h.page.listenerCount('request'), 1);
  for (const name of ['response', 'requestfinished', 'requestfailed']) assert.equal(h.page.listenerCount(name), 0);
  h.emit({ index: 2 }); assert.equal(h.observer.rows.length, 1);
  assert.equal(h.observer.snapshot().closed, true); h.page.off('request', unrelated);
});

test('observer stores neither authorization, cookie, nonce nor observed raw body', () => {
  const h = harness();
  try {
    const body = Buffer.from('{"synthetic":"private test marker"}');
    finish(h.page, h.emit({ method: 'POST', body }));
    const snapshot = h.observer.snapshot(), row = snapshot.rows[0];
    assert.deepEqual(Object.keys(row).sort(), ['body_observation', 'event_id', 'method', 'native', 'observed_body_bytes', 'observed_body_sha256', 'path', 'signed_payload_sha256', 'status']);
    for (const marker of ['private test marker', 'Nostr ', 'explicit_fake_nonce_', 'cookie']) assert.ok(!JSON.stringify(snapshot).includes(marker));
    assert.equal(row.observed_body_sha256, uploadDigest(body));
  } finally { h.observer.close(); }
});

for (const [name, change] of [
  ['noncanonical base64', value => { value.authorization = value.authorization.replace(/=+$/, ''); }],
  ['extra tag', value => { const event = eventFor(); event.tags.push(['extra', 'fake']); event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content])); value.authorization = encode(event); }],
  ['nonempty event content', value => { const event = eventFor(); event.content = 'fake'; event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content])); value.authorization = encode(event); }],
]) test('event boundary refuses ' + name, () => {
  const value = { ...scope, authorization: encode(eventFor()), url: origin + '/api/v1/upload', method: 'GET', body: null };
  if (name === 'noncanonical base64') {
    // Vary an otherwise valid envelope until its canonical encoding needs padding.
    for (let index = 1; index <= 100 && !value.authorization.endsWith('='); index++) value.authorization = encode(eventFor({ index }));
    assert.ok(value.authorization.endsWith('='), 'Explicit fixture must exercise padding removal');
  }
  change(value); assert.throws(() => validateUploadHttpEvent(value));
});

test('independent reader reserves its owner before a reentrant signer callback', async () => {
  let reader, reentrant, calls = 0;
  reader = createUploadIndependentReader({ ...scope, sign: async template => {
    reentrant = reader.get('/api/v1/upload'); reentrant.catch(() => {}); return fakeSign(template);
  }, fetch: async () => { calls++; return json({ result: 'ok' }); } });
  const value = await reader.get('/api/v1/upload');
  await assert.rejects(reentrant); assert.equal(value.status, 200); assert.equal(calls, 1);
  assert.equal(reader.snapshot().pending, 0); reader.close();
});

test('independent reader prevents concurrent reads and premature close while signing', async () => {
  const signing = deferred(); let calls = 0;
  const reader = createUploadIndependentReader({ ...scope, sign: async template => { await signing.promise; return fakeSign(template); }, fetch: async () => { calls++; return json({}); } });
  const pending = reader.get('/api/v1/upload'); assert.equal(reader.snapshot().pending, 1);
  await assert.rejects(reader.get('/api/v1/upload')); assert.throws(() => reader.close()); assert.equal(calls, 0);
  signing.resolve(); await pending; reader.close(); assert.equal(reader.snapshot().closed, true);
});

for (const [name, response] of [
  ['wrong media type', () => new Response('{}', { headers: { 'content-type': 'text/html' } })],
  ['invalid JSON', () => new Response('{', { headers: { 'content-type': 'application/json' } })],
  ['oversized later chunk', () => new Response(new ReadableStream({ start(controller) { controller.enqueue(new Uint8Array(65536)); controller.enqueue(new Uint8Array(1)); controller.close(); } }), { headers: { 'content-type': 'application/json' } })],
]) test('independent read failure releases body ownership and retains no success for ' + name, async () => {
  const body = response(); let calls = 0;
  const reader = createUploadIndependentReader({ ...scope, sign: fakeSign, fetch: async () => { calls++; return body; } });
  await assert.rejects(reader.get('/api/v1/upload')); assert.equal(calls, 1);
  assert.equal(body.body.locked, false); assert.deepEqual(reader.snapshot().rows, []); assert.equal(reader.snapshot().pending, 0);
  reader.close();
});

test('independent reader accepts exactly 65536 JSON bytes and stores only bounded receipt facts', async () => {
  const body = JSON.stringify({ text: 'x'.repeat(65536 - '{"text":""}'.length) }); assert.equal(Buffer.byteLength(body), 65536);
  const reader = createUploadIndependentReader({ ...scope, sign: fakeSign, fetch: async () => new Response(body, { headers: { 'content-type': 'application/json; charset=utf-8' } }) });
  assert.equal((await reader.get('/api/v1/upload')).body.text.length, 65525);
  const row = reader.snapshot().rows[0]; assert.deepEqual(Object.keys(row).sort(), ['bytes', 'complete', 'event_id', 'method', 'path', 'sha256', 'status']);
  assert.equal(row.bytes, 65536); assert.equal(row.sha256, uploadDigest(body)); reader.close();
});

for (const pathname of ['http://example.invalid/api/v1/upload', '/api/v1/upload?token=fake', '/api/v1/upload#', '/api/v1/nostr/not-an-allowed-path']) test('independent reader rejects path before signing or transport: ' + pathname, async () => {
  let signatures = 0, calls = 0;
  const reader = createUploadIndependentReader({ ...scope, sign: async value => { signatures++; return fakeSign(value); }, fetch: async () => { calls++; return json({}); } });
  await assert.rejects(reader.get(pathname)); assert.equal(signatures, 0); assert.equal(calls, 0); reader.close();
});

test('independent successful-read cap is 96; closed owners cannot sign or transmit', async () => {
  let calls = 0, signatures = 0; const events = [];
  const reader = createUploadIndependentReader({ ...scope, sign: async value => { signatures++; events.push(value); return fakeSign(value); }, fetch: async () => { calls++; return json({}); } });
  for (let i = 0; i < 96; i++) await reader.get('/api/v1/upload');
  await assert.rejects(reader.get('/api/v1/upload'));
  assert.equal(calls, 96); assert.equal(signatures, 96); assert.equal(new Set(events.map(event => event.tags.at(-1)[1])).size, 96);
  const snapshot = reader.snapshot(); snapshot.rows[0].status = 599; assert.equal(reader.snapshot().rows[0].status, 200);
  reader.close(); reader.close(); await assert.rejects(reader.get('/api/v1/upload'));
  assert.equal(calls, 96); assert.equal(signatures, 96); assert.equal(reader.snapshot().pending, 0);
});

test('signer and transport failures release ownership without automatic retry', async () => {
  for (const failedAt of ['signer', 'transport']) {
    let signatures = 0, calls = 0;
    const reader = createUploadIndependentReader({ ...scope, sign: async value => { signatures++; if (failedAt === 'signer') throw Error('synthetic signer failure'); return fakeSign(value); }, fetch: async () => { calls++; throw Error('synthetic transport failure'); } });
    await assert.rejects(reader.get('/api/v1/upload')); assert.equal(signatures, 1); assert.equal(calls, failedAt === 'signer' ? 0 : 1);
    assert.equal(reader.snapshot().pending, 0); assert.deepEqual(reader.snapshot().rows, []); reader.close();
  }
});

test('five-second abort prevents transport after a late signer callback', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const signing = deferred(); let calls = 0;
  const reader = createUploadIndependentReader({ ...scope, sign: async value => { await signing.promise; return fakeSign(value); }, fetch: async () => { calls++; return json({}); } });
  const pending = reader.get('/api/v1/upload'); pending.catch(() => {});
  t.mock.timers.tick(5000); signing.resolve(); await assert.rejects(pending);
  assert.equal(calls, 0); assert.equal(reader.snapshot().pending, 0); assert.deepEqual(reader.snapshot().rows, []); reader.close();
});

test('five-second abort closes an abort-aware pending response stream without a receipt', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const started = deferred(); let response, calls = 0, observedSignal;
  const reader = createUploadIndependentReader({ ...scope, sign: fakeSign, fetch: async (_url, init) => {
    calls++; observedSignal = init.signal;
    const stream = new ReadableStream({ start(controller) {
      init.signal.addEventListener('abort', () => controller.error(new Error('synthetic stream abort')), { once: true });
    } });
    response = new Response(stream, { headers: { 'content-type': 'application/json' } }); started.resolve(); return response;
  } });
  const pending = reader.get('/api/v1/upload'); pending.catch(() => {});
  await started.promise; t.mock.timers.tick(5000); await assert.rejects(pending);
  assert.equal(observedSignal.aborted, true); assert.equal(calls, 1); assert.equal(response.body.locked, false);
  assert.equal(reader.snapshot().pending, 0); assert.deepEqual(reader.snapshot().rows, []); reader.close();
});
