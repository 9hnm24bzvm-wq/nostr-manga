#!/usr/bin/env node
// Run with Node >= 22.13 and --experimental-vm-modules. Only this production
// transport is evaluated, with explicit finite in-memory primitive adapters.
// SHA-256 is real Node crypto; signatures/service proof are deliberately NOT
// claimed here. No dependencies, server, sockets, accounts or remote requests.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';

assert(vm.SourceTextModule, 'Run with node --experimental-vm-modules');
const source = fs.readFileSync(new URL('../../platform-integrations/public-api/nostr-upload-request.ts', import.meta.url), 'utf8');
const text = new TextEncoder();
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const clone = value => JSON.parse(JSON.stringify(value));
function deferred() { let resolve; const promise = new Promise(yes => { resolve = yes; }); return { promise, resolve }; }

async function fixture(options = {}) {
  let clock = 0, alive = true, identityCurrent = true, signerCalls = 0, proofCalls = 0, beforeCalls = 0;
  const timers = new Map(), requests = [], templates = [], timerHistory = [];
  const signing = deferred(), signEntered = deferred(), preflight = deferred(), preflightEntered = deferred();
  const owner = '1'.repeat(64), serviceKey = '2'.repeat(64);
  const scope = { origin: 'https://fixture.invalid', serviceKey, catalogNamespace: '33333333-3333-8333-8333-333333333333' };
  const identity = { owner, generation: 1, check() { if (!identityCurrent) throw Error('SECRET selected identity changed'); }, signer: {
    async signEvent(event) {
      signerCalls++; signEntered.resolve();
      if (options.holdSigner) await signing.promise;
      options.onSign?.();
      return { ...clone(event), id: 'synthetic-id', sig: 'synthetic-signature' };
    },
  } };
  const primitives = {
    async signExact(signer, selectedOwner, event) {
      proofCalls++; templates.push(clone(event));
      if (options.beforeSigner) await options.beforeSigner();
      return signer.signEvent({ ...event, pubkey: selectedOwner });
    },
    hash: digest,
    b64: bytes => Buffer.from(bytes).toString('base64'),
    nonce: () => 'synthetic_nonce_0001', now: () => 1700000000, clock: () => clock,
    setTimer(callback, delay) { const token = {}; timers.set(token, { callback, at: clock + delay }); timerHistory.push(delay); return token; },
    clearTimer(token) { timers.delete(token); },
    async fetch(url, init) {
      requests.push({ url, init });
      if (options.fetch) return options.fetch(url, init);
      return new Response('{"result":"ok"}', { status: 201, headers: { 'content-type': 'application/json' } });
    },
  };
  const context = vm.createContext({ URL, TextEncoder, TextDecoder, Uint8Array, AbortController, Promise, Error, Number, Object,
    setTimeout, clearTimeout, performance, crypto, fetch() { throw Error('A real fetch is forbidden'); } });
  const nostr = new vm.SyntheticModule(['signExact', 'origin'], function () {
    this.setExport('signExact', primitives.signExact);
    this.setExport('origin', value => {
      const url = new URL(value);
      if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash || url.pathname !== '/') throw Error('Invalid fixture origin');
      return url.origin;
    });
  }, { context });
  const codec = new vm.SyntheticModule(['hash', 'b64'], function () { this.setExport('hash', digest); this.setExport('b64', primitives.b64); }, { context });
  const module = new vm.SourceTextModule(stripTypeScriptTypes(source), { context });
  await module.link(specifier => {
    if (specifier === '../local-library/protocol/nostr.js') return nostr;
    if (specifier === '../local-library/protocol/codec.js') return codec;
    throw Error('Unreviewed import: ' + specifier);
  });
  await module.evaluate();
  const api = module.namespace;
  const transport = api.createNostrUploadRequest({ scope, identity,
    assertCurrent() { if (!alive) throw Error('SECRET configured scope or view changed'); options.onCheck?.(); },
    async beforeSend(signal) {
      beforeCalls++; preflightEntered.resolve();
      if (options.holdBeforeSend) await preflight.promise;
      if (options.beforeSend) await options.beforeSend(signal);
    },
  }, primitives);
  const request = overrides => transport({ url: scope.origin + '/api/v1/upload/begin', method: 'POST',
    body: text.encode('{"manga":"synthetic"}'), contentType: 'application/json', ...overrides });
  return { api, transport, request, requests, templates, timers, timerHistory, scope, identity,
    get signerCalls() { return signerCalls; }, get proofCalls() { return proofCalls; }, get beforeCalls() { return beforeCalls; },
    close() { alive = false; }, changeIdentity() { identityCurrent = false; },
    advance(milliseconds) {
      clock += milliseconds;
      for (const [token, timer] of [...timers]) if (timer.at <= clock) { timers.delete(token); timer.callback(); }
    },
    signEntered: signEntered.promise, releaseSign: signing.resolve,
    preflightEntered: preflightEntered.promise, releasePreflight: preflight.resolve,
  };
}
const errorIs = (outcome, status) => error => {
  assert.equal(error.name, 'PublicUploadRequestError'); assert.equal(error.outcome, outcome);
  assert.equal(error.status, status); assert.equal(error.bodyAvailable, false);
  assert(!error.message.includes('SECRET')); assert(!error.message.includes('synthetic-signature'));
  return true;
};
const tags = f => Object.fromEntries(f.templates[0].tags);

test('exact original JSON bytes, URL, service, nonce and owner reach one credential-free dispatch without relay response proof', { timeout: 3000 }, async () => {
  const f = await fixture({ holdSigner: true });
  const original = text.encode(' {"unicode":"雪", "ordered":1}\n'), expected = new Uint8Array(original);
  const input = { url: f.scope.origin + '/api/v1/upload/begin', method: 'POST', body: original, contentType: 'application/json' };
  const pending = f.transport(input); await f.signEntered;
  original.fill(0); input.url = 'https://unrelated.invalid/leak'; input.method = 'DELETE'; input.contentType = 'text/plain';
  f.releaseSign(); const result = await pending;
  assert.equal(result.status, 201); assert.equal(result.bodyAvailable, true); assert.equal(result.body.result, 'ok');
  assert.equal(f.requests.length, 1); assert.equal(f.proofCalls, 1); assert.equal(f.signerCalls, 1);
  const { url, init } = f.requests[0], proof = JSON.parse(Buffer.from(init.headers.authorization.slice(6), 'base64'));
  assert.equal(url, f.scope.origin + '/api/v1/upload/begin'); assert.equal(init.method, 'POST');
  assert.deepEqual(init.body, expected); assert.equal(tags(f).payload, digest(expected));
  assert.deepEqual(tags(f), { u: url, method: 'POST', service: f.scope.serviceKey, nonce: 'synthetic_nonce_0001', payload: digest(expected) });
  assert.equal(proof.kind, 27235); assert.equal(proof.pubkey, f.identity.owner); assert.equal(proof.content, '');
  assert.equal(init.credentials, 'omit'); assert.equal(init.redirect, 'error'); assert.equal(init.cache, 'no-store');
  assert.equal(init.headers['content-type'], 'application/json'); assert.equal(init.signal.aborted, true);
  assert.equal(f.timers.size, 0);
});

test('multipart is already serialized, explicitly bounded and identical to its signed digest', { timeout: 3000 }, async () => {
  const f = await fixture();
  const bytes = text.encode('--synthetic_boundary\r\nContent-Disposition: form-data; name="file"; filename="page.png"\r\n\r\nsynthetic\r\n--synthetic_boundary--\r\n');
  await f.request({ body: bytes, contentType: 'multipart/form-data; boundary=synthetic_boundary' });
  assert.deepEqual(f.requests[0].init.body, bytes); assert.equal(tags(f).payload, digest(bytes));
  assert.equal(f.requests[0].init.headers['content-type'], 'multipart/form-data; boundary=synthetic_boundary');
  await assert.rejects(f.request({ body: bytes, contentType: 'multipart/form-data' }), errorIs('unsent'));
  assert.equal(f.requests.length, 1);
});

test('empty mutations bind SHA-256 of empty bytes; empty GET omits payload and body; PATCH is supported', { timeout: 3000 }, async () => {
  for (const method of ['DELETE', 'GET', 'PATCH']) {
    const f = await fixture(); await f.request({ method, body: new Uint8Array(), contentType: undefined });
    assert.equal(tags(f).method, method);
    if (method === 'GET') { assert.equal(tags(f).payload, undefined); assert.equal(f.requests[0].init.body, undefined); }
    else { assert.equal(tags(f).payload, digest(new Uint8Array())); assert.equal(f.requests[0].init.body.byteLength, 0); }
  }
});

test('unsafe, noncanonical or foreign URLs and unsupported encodings/methods never ask the signer or dispatch', { timeout: 3000 }, async () => {
  const f = await fixture();
  for (const url of ['https://elsewhere.invalid/api', 'https://name:password@fixture.invalid/api',
    'https://fixture.invalid/api#fragment', 'https://fixture.invalid/a/../b', 'https://fixture.invalid/api\n', '//fixture.invalid/api']) {
    await assert.rejects(f.request({ url }), errorIs('unsent'));
  }
  for (const overrides of [{ method: 'PUT' }, { body: new FormData() }, { contentType: 'text/plain' },
    { method: 'GET', body: text.encode('x'), contentType: undefined }, { body: text.encode('x'), contentType: undefined },
    { contentType: 'multipart/form-data; boundary=unsafe; extra=x' }, { responseType: 'html' }]) {
    await assert.rejects(f.request(overrides), errorIs('unsent'));
  }
  assert.equal(f.proofCalls, 0); assert.equal(f.requests.length, 0); assert.equal(f.timers.size, 0);
});

test('JSON, multipart and caller-lowered request budgets cannot be raised', { timeout: 3000 }, async () => {
  const f = await fixture();
  for (const overrides of [{ body: new Uint8Array(32769) }, { requestLimit: 32769 },
    { body: new Uint8Array(32 * 1024 * 1024 + 1), contentType: 'multipart/form-data; boundary=fixture' },
    { contentType: 'multipart/form-data; boundary=fixture', requestLimit: 32 * 1024 * 1024 + 1 },
    { body: text.encode('123'), requestLimit: 2 }, { requestLimit: -1 }, { requestLimit: NaN },
    { responseLimit: 1024 * 1024 + 1 }, { responseType: 'image', responseLimit: 32 * 1024 * 1024 + 1 }]) {
    await assert.rejects(f.request(overrides), errorIs('unsent'));
  }
  assert.equal(f.proofCalls, 0); assert.equal(f.requests.length, 0);
});

test('pre-abort and obsolete signer or view prevent signing', { timeout: 3000 }, async () => {
  const controller = new AbortController(); controller.abort();
  const f = await fixture(); await assert.rejects(f.request({ signal: controller.signal }), errorIs('unsent'));
  f.changeIdentity(); await assert.rejects(f.request(), errorIs('unsent'));
  const g = await fixture(); g.close(); await assert.rejects(g.request(), errorIs('unsent'));
  assert.equal(f.proofCalls + g.proofCalls, 0); assert.equal(f.requests.length + g.requests.length, 0);
});

test('closing before a deferred signer callback prevents the external prompt itself', { timeout: 3000 }, async () => {
  const gate = deferred(), entered = deferred();
  const f = await fixture({ beforeSigner: async () => { entered.resolve(); await gate.promise; } });
  const pending = f.request(); await entered.promise; f.close(); gate.resolve();
  await assert.rejects(pending, errorIs('unsent')); assert.equal(f.signerCalls, 0); assert.equal(f.requests.length, 0);
});

test('identity change during a signer prompt and scope change during final refresh prevent dispatch', { timeout: 3000 }, async () => {
  const f = await fixture({ holdSigner: true }); const first = f.request(); await f.signEntered;
  f.changeIdentity(); f.releaseSign(); await assert.rejects(first, errorIs('unsent')); assert.equal(f.requests.length, 0);
  const g = await fixture({ holdBeforeSend: true }); const second = g.request(); await g.preflightEntered;
  g.close(); g.releasePreflight(); await assert.rejects(second, errorIs('unsent')); assert.equal(g.requests.length, 0);
  assert.equal(g.beforeCalls, 1);
});

test('captured identity fields cannot be swapped while a prompt is pending', { timeout: 3000 }, async () => {
  for (const change of [f => { f.identity.owner = '9'.repeat(64); }, f => { f.identity.generation++; },
    f => { f.identity.signer = { async signEvent() { throw Error('Unexpected replacement signer'); } }; }]) {
    const f = await fixture({ holdSigner: true }); const pending = f.request(); await f.signEntered;
    change(f); f.releaseSign(); await assert.rejects(pending, errorIs('unsent')); assert.equal(f.requests.length, 0);
  }
});

test('signer deadline settles locally; late approval is ignored and never replayed', { timeout: 3000 }, async () => {
  const f = await fixture({ holdSigner: true }); const pending = f.request({ timeoutMs: 50 }); await f.signEntered;
  f.advance(50); await assert.rejects(pending, errorIs('unsent')); f.releaseSign(); await Promise.resolve();
  assert.equal(f.requests.length, 0); assert.equal(f.signerCalls, 1); assert.equal(f.timers.size, 0);
});

test('one absolute deadline spans signer, final refresh, dispatch and a stalled response reader', { timeout: 3000 }, async () => {
  const readEntered = deferred(), read = deferred(); let cancelled = 0, released = 0, f;
  const body = { getReader() { return { read() { readEntered.resolve(); return read.promise; },
    async cancel() { cancelled++; read.resolve({ done: true }); }, releaseLock() { released++; } }; } };
  f = await fixture({ onSign() { f.advance(20); }, beforeSend() { f.advance(20); },
    fetch: async () => ({ status: 201, headers: new Headers({ 'content-type': 'application/json' }), body }) });
  const pending = f.request({ timeoutMs: 50 }); await readEntered.promise;
  assert.deepEqual(f.timerHistory, [50]); f.advance(10);
  await assert.rejects(pending, errorIs('response', 201));
  assert.equal(f.requests.length, 1); assert.equal(cancelled, 1); assert.equal(released, 1); assert.equal(f.timers.size, 0);
});

test('external abort during signing leaves no dispatch, timer or external listener', { timeout: 3000 }, async () => {
  const f = await fixture({ holdSigner: true }), controller = new AbortController();
  let added = 0, removed = 0;
  const add = controller.signal.addEventListener.bind(controller.signal), remove = controller.signal.removeEventListener.bind(controller.signal);
  controller.signal.addEventListener = (...args) => { added++; return add(...args); };
  controller.signal.removeEventListener = (...args) => { removed++; return remove(...args); };
  const pending = f.request({ signal: controller.signal }); await f.signEntered; controller.abort();
  await assert.rejects(pending, errorIs('unsent')); f.releaseSign();
  assert.equal(added, 1); assert.equal(removed, 1); assert.equal(f.requests.length, 0); assert.equal(f.timers.size, 0);
});

test('network failure is uncertain after exactly one mutation dispatch and is never retried', { timeout: 3000 }, async () => {
  const f = await fixture({ fetch: async () => { throw Error('SECRET internal transport failure'); } });
  await assert.rejects(f.request(), errorIs('uncertain'));
  assert.equal(f.requests.length, 1); assert.equal(f.proofCalls, 1); assert.equal(f.timers.size, 0);
});

test('a late response after deadline is discarded and its stream is cancelled', { timeout: 3000 }, async () => {
  const fetched = deferred(), entered = deferred(); let cancelled = 0;
  const f = await fixture({ fetch: () => { entered.resolve(); return fetched.promise; } });
  const pending = f.request({ timeoutMs: 50 }); await entered.promise; f.advance(50);
  await assert.rejects(pending, errorIs('uncertain'));
  fetched.resolve({ status: 201, body: { async cancel() { cancelled++; } } });
  await new Promise(resolve => setImmediate(resolve)); assert.equal(cancelled, 1); assert.equal(f.requests.length, 1);
});

test('actual HTTP errors are returned with their body and retry header', { timeout: 3000 }, async () => {
  const f = await fixture({ fetch: async () => new Response('{"result":"error","errors":[{"title":"No grant"}]}',
    { status: 403, headers: { 'content-type': 'application/json', 'retry-after': '12' } }) });
  const response = await f.request(); assert.equal(response.status, 403); assert.equal(response.body.result, 'error');
  assert.equal(response.headers['retry-after'], '12'); assert.equal(f.requests.length, 1);
});

test('known status survives malformed JSON, invalid UTF-8, unexpected types and failed body reads', { timeout: 3000 }, async () => {
  const responses = [() => new Response('{broken', { status: 409, headers: { 'content-type': 'application/json' } }),
    () => new Response(new Uint8Array([255]), { status: 409, headers: { 'content-type': 'application/json' } }),
    () => new Response('SECRET backend text', { status: 409, headers: { 'content-type': 'text/plain' } }),
    () => ({ status: 409, headers: new Headers({ 'content-type': 'application/json' }), body: {
      getReader() { return { async read() { throw Error('SECRET read error'); }, async cancel() {}, releaseLock() {} }; },
    } })];
  for (const response of responses) {
    const f = await fixture({ fetch: async () => response() });
    await assert.rejects(f.request(), errorIs('response', 409)); assert.equal(f.requests.length, 1);
  }
});

test('scope invalidation upon fetch resolution preserves observed mutation status', { timeout: 3000 }, async () => {
  let f; f = await fixture({ fetch: async () => { f.close(); return new Response('{}', { status: 201, headers: { 'content-type': 'application/json' } }); } });
  await assert.rejects(f.request(), errorIs('response', 201)); assert.equal(f.requests.length, 1);
});

test('decoded JSON and images have hard default response caps and lower caller caps', { timeout: 3000 }, async () => {
  for (const [type, cap, size, lower] of [['json', 1024 * 1024, 1024 * 1024 + 1], ['image', 32 * 1024 * 1024, 32 * 1024 * 1024 + 1], ['json', 3, 4, 3]]) {
    let cancelled = 0;
    const f = await fixture({ fetch: async () => ({ status: 200,
      headers: new Headers({ 'content-type': type === 'image' ? 'image/png' : 'application/json', 'content-length': String(size) }),
      body: { async cancel() { cancelled++; } },
    }) });
    await assert.rejects(f.request({ responseType: type, ...(lower === undefined ? {} : { responseLimit: cap }) }), errorIs('response', 200));
    assert.equal(cancelled, 1);
  }
  let cancelled = 0, released = 0;
  const f = await fixture({ fetch: async () => ({ status: 200, headers: new Headers({ 'content-type': 'application/json' }),
    body: { getReader() { return { async read() { return { done: false, value: text.encode('12345') }; },
      async cancel() { cancelled++; }, releaseLock() { released++; } }; } },
  }) });
  await assert.rejects(f.request({ responseLimit: 4 }), errorIs('response', 200));
  assert.equal(cancelled, 1); assert.equal(released, 1);
});

test('binary preview bytes remain exact; unsafe image content and JSON errors preserve real status', { timeout: 3000 }, async () => {
  const bytes = new Uint8Array([137, 80, 78, 71, 13, 10]);
  const f = await fixture({ fetch: async () => new Response(bytes, { status: 200, headers: { 'content-type': 'image/png' } }) });
  const response = await f.request({ method: 'GET', body: undefined, contentType: undefined, responseType: 'image' });
  assert.equal(response.status, 200); assert.deepEqual(response.body, bytes);
  const g = await fixture({ fetch: async () => new Response('<svg/>', { status: 200, headers: { 'content-type': 'image/svg+xml' } }) });
  await assert.rejects(g.request({ responseType: 'image' }), errorIs('response', 200));
  const h = await fixture({ fetch: async () => new Response('{"result":"error"}', { status: 404, headers: { 'content-type': 'application/json' } }) });
  const missing = await h.request({ responseType: 'image' }); assert.equal(missing.status, 404); assert.equal(missing.body.result, 'error');
});

test('a genuine empty 204 keeps its status and has no invented success body', { timeout: 3000 }, async () => {
  const f = await fixture({ fetch: async () => new Response(null, { status: 204 }) });
  const response = await f.request(); assert.equal(response.status, 204); assert.equal(response.body, undefined);
});
