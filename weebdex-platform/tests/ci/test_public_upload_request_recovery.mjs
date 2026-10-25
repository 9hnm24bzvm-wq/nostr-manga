#!/usr/bin/env node
import {restoreBundleSource} from './reviewed-bundle-source-successors.mjs';
import {restoreModeratorTransportSource} from './reviewed-moderator-transport-source-successors.mjs';
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
import { fileURLToPath } from 'node:url';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { applyPublicUploadOverlay } from '../../platform-integrations/public-api/upload-overlays.mjs';
import { restoreReviewedLanguageSource } from './reviewed-local-page-source-successors.mjs';

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
      const request = { url, init, abortCount: 0 };
      init.signal.addEventListener('abort', () => { request.abortCount++; }, { once: true });
      requests.push(request);
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
  const module = new vm.SourceTextModule(stripTypeScriptTypes(options.transportSource ?? source), { context });
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

// Actual uploadPreview source plus the actual transport, with explicit staged
// metadata/stream stand-ins. This identifies consumption boundaries, not a
// browser's native terminal or the cause of an observed hosted abort.
function previewConsumer(f, options = {}) {
  const client = fs.readFileSync(new URL('../../platform-integrations/public-api/upload-client.ts', import.meta.url), 'utf8');
  assert.equal(digest(restoreReviewedLanguageSource(client, 'uploadClient')), 'c8d3e55319a2f93ae1f9b4e11875c2d4711a1d8d985e4dc7df5ab722f26dca8f');
  const start = client.indexOf('  async function uploadPreview('), end = client.indexOf('\n  function uploadAccountFacts', start);
  assert.ok(start > 0 && end > start);
  // Chapter-access successor leaves this exact preview consumer unchanged.
  assert.equal(digest(client.slice(start, end)), '369d7261e2b8b84989602d45154bd44ba9fcc2817c9e986c4d1ac50fc3ad3fae');
  const bytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
  const sessionId = '11111111-1111-8111-8111-111111111111', fileId = '22222222-2222-8222-8222-222222222222';
  const input = { id: fileId, file_hash: digest(bytes), file_size: String(bytes.length), original_file_name: 'finite-preview.png',
    public_upload: { fileSize: bytes.length, mimeType: 'image/png', dimensions: { width: 2, height: 3 },
      previewUrl: `/api/v1/nostr/uploads/${sessionId}/files/${fileId}/preview` } };
  const fresh = { id: sessionId, version: 1, relationships: { files: [input] } };
  const owned = { account: { scope: f.scope }, latest: fresh, assert() {}, request: value => f.transport(value) };
  const deps = { digest: async value => options.badDigest ? '0'.repeat(64) : digest(value),
    decodeImage: async () => ({ width: options.badDimensions ? 9 : 2, height: 3 }) };
  const fail = message => { throw Error(message); };
  const factory = new Function('ownedBy', 'publicUuid', 'exact', 'fail', 'VisibleError', 'replacementPending', 'unknownReplacement', 'Blob', 'deps',
    stripTypeScriptTypes(client.slice(start, end), { mode: 'transform' }) + '; return uploadPreview;');
  const preview = factory(() => owned, value => value, async () => fresh, fail, Error, () => undefined, 'finite replacement fence', Blob, deps);
  return { bytes, input, owned, load: () => preview({}, sessionId, input) };
}
function previewResponse(bytes, options = {}) {
  const headers = new Headers({ 'content-type': options.mime ?? 'image/png', 'content-length': String(options.length ?? bytes.length),
    'cache-control': options.private === false ? 'public' : 'private, no-store', vary: 'Authorization', 'x-content-type-options': 'nosniff' });
  return new Response(bytes, { status: 200, headers });
}
function heldPreviewBody(bytes) {
  const entered = deferred(), terminal = deferred(); let reads = 0, cancelled = 0, released = 0;
  const response = { status: 200, headers: new Headers({ 'content-type': 'image/png', 'content-length': String(bytes.length),
    'cache-control': 'private, no-store', vary: 'Authorization', 'x-content-type-options': 'nosniff' }),
    body: { getReader: () => ({ read: async () => {
      if (++reads === 1) return { done: false, value: bytes };
      entered.resolve(); return terminal.promise;
    }, cancel: async () => { cancelled++; terminal.resolve({ done: true }); }, releaseLock: () => { released++; } }) } };
  return { response, entered: entered.promise, finish: () => terminal.resolve({ done: true }),
    get reads() { return reads; }, get cancelled() { return cancelled; }, get released() { return released; } };
}

test('valid 200 preview consumption survives metadata-only autosave change without controller abort', { timeout: 3000 }, async () => {
  const bytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), stream = heldPreviewBody(bytes);
  const f = await fixture({ fetch: async () => stream.response }), preview = previewConsumer(f);
  const pending = preview.load(); await stream.entered;
  preview.owned.latest = { version: 2, page_order: [preview.input.id], relationships: { files: [{ ...preview.input }] } };
  stream.finish(); const blob = await pending;
  assert.equal(blob.type, 'image/png'); assert.equal(blob.size, bytes.length);
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].init.method, 'GET'); assert.equal(f.requests[0].abortCount, 0);
  assert.equal(stream.cancelled, 0); assert.equal(stream.released, 1); assert.equal(f.timers.size, 0);
});
for (const invalidation of ['owner/view', 'selected identity', 'deadline']) test('HTTP200 pending preview body remains unavailable after ' + invalidation + ' invalidation', { timeout: 3000 }, async () => {
  const bytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), stream = heldPreviewBody(bytes);
  const f = await fixture({ fetch: async () => stream.response }), preview = previewConsumer(f);
  const pending = preview.load(); pending.catch(() => {}); await stream.entered;
  if (invalidation === 'owner/view') f.close(); else if (invalidation === 'selected identity') f.changeIdentity(); else f.advance(60000);
  stream.finish(); await assert.rejects(pending, errorIs('response', 200));
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 1);
  assert.equal(stream.cancelled, 1); assert.equal(stream.released, 1); assert.equal(f.timers.size, 0);
});
for (const boundary of ['media', 'declared size', 'actual size', 'unsafe header']) test('HTTP200 preview consumption rejects ' + boundary + ' without inventing a completed response', { timeout: 3000 }, async () => {
  let f;
  f = await fixture({ fetch: async () => {
    const bytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
    if (boundary === 'unsafe header') return { status: 200, headers: { get: name => name === 'content-type' ? 'image/png' : name === 'cache-control' ? 'private\tno-store' : null }, body: { cancel: async () => {} } };
    return previewResponse(boundary === 'actual size' ? new Uint8Array(bytes.length + 1) : bytes,
      { mime: boundary === 'media' ? 'application/json' : 'image/png', length: boundary === 'declared size' ? bytes.length + 1 : bytes.length });
  } });
  const preview = previewConsumer(f); await assert.rejects(preview.load(), errorIs('response', 200));
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 1); assert.equal(f.timers.size, 0);
});
test('HTTP200 preview stream read rejection cancels only its unfinished reader and retains known status', { timeout: 3000 }, async () => {
  let cancelled = 0, released = 0;
  const f = await fixture({ fetch: async () => ({ status: 200, headers: new Headers({ 'content-type': 'image/png' }),
    body: { getReader: () => ({ read: async () => { throw Error('synthetic stream read failure'); },
      cancel: async () => { cancelled++; }, releaseLock: () => { released++; } }) } }) });
  await assert.rejects(previewConsumer(f).load(), errorIs('response', 200));
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 1);
  assert.equal(cancelled, 1); assert.equal(released, 1); assert.equal(f.timers.size, 0);
});
for (const refusal of ['privacy', 'digest', 'dimensions']) test('actual preview ' + refusal + ' refusal after complete200 consumption cannot abort that completed request', { timeout: 3000 }, async () => {
  const bytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
  const f = await fixture({ fetch: async () => previewResponse(bytes, { private: refusal !== 'privacy' }) });
  const preview = previewConsumer(f, { badDigest: refusal === 'digest', badDimensions: refusal === 'dimensions' });
  await assert.rejects(preview.load(), error => {
    assert.notEqual(error.name, 'PublicUploadRequestError');
    assert.match(error.message, refusal === 'privacy' ? /privacy headers/ : refusal === 'digest' ? /original image receipt/ : /dimensions/); return true;
  });
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 0); assert.equal(f.timers.size, 0);
});
test('empty but fully consumed200 preview is refused by actual receipt validation without controller abort', { timeout: 3000 }, async () => {
  const f = await fixture({ fetch: async () => previewResponse(new Uint8Array()) });
  await assert.rejects(previewConsumer(f).load(), error => error.name !== 'PublicUploadRequestError' && /original image receipt/.test(error.message));
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 0); assert.equal(f.timers.size, 0);
});

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
  assert.equal(init.headers['content-type'], 'application/json'); assert.equal(init.signal.aborted, false);
  assert.equal(f.requests[0].abortCount, 0);
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
  for (const method of ['DELETE', 'GET', 'PATCH', 'PUT']) {
    const f = await fixture(); await f.request({ method, body: new Uint8Array(), contentType: undefined });
    assert.equal(tags(f).method, method);
    if (method === 'GET') { assert.equal(tags(f).payload, undefined); assert.equal(f.requests[0].init.body, undefined); }
    else { assert.equal(tags(f).payload, digest(new Uint8Array())); assert.equal(f.requests[0].init.body.byteLength, 0); }
  }
});

test('unsafe, noncanonical or foreign URLs and unsupported encodings/methods never ask the signer or dispatch', { timeout: 3000 }, async () => {
  const f = await fixture();
  for (const url of ['https://elsewhere.invalid/api', 'https://name:password@fixture.invalid/api',
    'https://fixture.invalid/api#fragment', 'https://fixture.invalid/api#', 'https://fixture.invalid/a/../b', 'https://fixture.invalid/api\n', '//fixture.invalid/api']) {
    await assert.rejects(f.request({ url }), errorIs('unsent'));
  }
  for (const overrides of [{ method: 'OPTIONS' }, { body: new FormData() }, { contentType: 'text/plain' },
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
  assert.equal(f.requests[0].abortCount, 1);
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
  assert.equal(f.requests[0].abortCount, 1);
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

test('accepted HTTP 202 with malformed JSON remains a known 202 and is never replayed', { timeout: 3000 }, async () => {
  const f = await fixture({ fetch: async () => new Response('{incomplete', { status: 202, headers: { 'content-type': 'application/json' } }) });
  await assert.rejects(f.request(), errorIs('response', 202));
  assert.equal(f.requests.length, 1); assert.equal(f.proofCalls, 1); assert.equal(f.requests[0].abortCount, 1);
});

test('HTTP 200 with an unavailable body remains a known 200 without fabricating success data', { timeout: 3000 }, async () => {
  let cancelled = 0, released = 0;
  const f = await fixture({ fetch: async () => ({ status: 200, headers: new Headers({ 'content-type': 'application/json' }),
    body: { getReader() { return { async read() { throw Error('SECRET unavailable body'); },
      async cancel() { cancelled++; }, releaseLock() { released++; } }; } },
  }) });
  await assert.rejects(f.request(), errorIs('response', 200));
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 1); assert.equal(cancelled, 1); assert.equal(released, 1);
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

test('only the bounded allowlisted actual response headers are exposed, with absent privacy fields left null', { timeout: 3000 }, async () => {
  const f = await fixture({ fetch: async () => new Response('{}', { status: 200, headers: {
    'content-type': 'application/json', 'retry-after': '17', 'cache-control': 'private, no-store',
    vary: 'Authorization, Origin', 'x-content-type-options': 'nosniff',
    authorization: 'Bearer SECRET', 'x-manga-response-proof': 'Nostr SECRET', 'set-cookie': 'SECRET=value',
  } }) });
  const response = await f.request();
  assert.deepEqual(Object.keys(response.headers).sort(), ['cache-control', 'content-type', 'retry-after', 'vary', 'x-content-type-options', 'x-weebdex-instance-removal', 'x-weebdex-source-deletion-request', 'x-weebdex-source-deletion-status']);
  assert.equal(response.headers['content-type'], 'application/json'); assert.equal(response.headers['retry-after'], '17');
  assert.equal(response.headers['cache-control'], 'private, no-store'); assert.equal(response.headers.vary, 'Authorization, Origin');
  assert.equal(response.headers['x-content-type-options'], 'nosniff');
  assert(!JSON.stringify(response.headers).includes('SECRET'));
  const missing = await (await fixture()).request();
  assert.equal(missing.headers['cache-control'], null); assert.equal(missing.headers.vary, null);
  assert.equal(missing.headers['x-content-type-options'], null);
});

test('overlong or control-bearing allowlisted header values fail closed while retaining the actual status', { timeout: 3000 }, async () => {
  for (const value of ['x'.repeat(4097), 'private,\nno-store', 'private\u0000no-store', 'private\tno-store']) {
    let cancelled = 0;
    const f = await fixture({ fetch: async () => ({ status: 200, headers: {
      get(name) { return name === 'content-type' ? 'application/json' : name === 'cache-control' ? value : null; },
    }, body: { async cancel() { cancelled++; } } }) });
    await assert.rejects(f.request(), errorIs('response', 200));
    assert.equal(f.requests.length, 1); assert.equal(f.requests[0].abortCount, 1); assert.equal(cancelled, 1);
  }
});

async function failure(f, overrides) {
  try { await f.request(overrides); assert.fail('The finite request must refuse'); }
  catch (error) { assert.equal(error.name, 'PublicUploadRequestError'); return error; }
}
test('transport refusal codes are exact frozen literals and never fields on the error', async () => {
  const f = await fixture({ fetch: async () => { throw Error('reserved-example.invalid/private-value'); } });
  assert.deepEqual([...f.api.uploadRequestRefusalStages], ['checkpoint', 'fetch', 'response-header', 'response-media', 'declared-size', 'body-read', 'body-budget', 'decode', 'other']);
  assert.ok(Object.isFrozen(f.api.uploadRequestRefusalStages));
  const error = await failure(f); assert.equal(f.api.uploadRequestRefusalStage(error), 'fetch');
  assert.deepEqual(Object.keys(error).sort(), ['bodyAvailable', 'name', 'outcome', 'status']);
  assert.doesNotMatch(JSON.stringify(error), /fetch|reserved-example|private-value/);
});
for (const stage of ['checkpoint', 'fetch', 'response-header', 'response-media', 'declared-size', 'body-read', 'body-budget', 'decode', 'other']) test('actual request brands only the genuine refusal boundary ' + stage, async () => {
  const bytes = new Uint8Array([1, 2]);
  const f = await fixture({ fetch: async () => {
    if (stage === 'fetch') throw Error('finite fetch rejection before Response');
    if (stage === 'response-header') return { status: 200, headers: { get() { throw Error('finite header read failure'); } } };
    if (stage === 'body-read') return { status: 200, headers: new Headers({ 'content-type': 'image/png' }), body: { getReader: () => ({ read: async () => { throw Error('finite reader failure'); }, cancel: async () => {}, releaseLock() {} }) } };
    return stage === 'decode' ? new Response('{broken', { status: 200, headers: { 'content-type': 'application/json' } })
      : new Response(stage === 'body-budget' ? new Uint8Array(3) : bytes, { status: 200, headers: { 'content-type': stage === 'response-media' ? 'application/json' : 'image/png', 'content-length': stage === 'declared-size' ? '3' : '2' } });
  } });
  if (stage === 'checkpoint') f.close();
  const error = await failure(f, stage === 'other' ? { timeoutMs: -1 } : stage === 'decode' ? {} : { method: 'GET', body: undefined, contentType: undefined, responseType: 'image', responseLimit: 2 });
  assert.equal(f.api.uploadRequestRefusalStage(error), stage);
  assert.equal(error.status, ['checkpoint', 'fetch', 'other'].includes(stage) ? undefined : 200);
  assert.equal(error.outcome, ['checkpoint', 'other'].includes(stage) ? 'unsent' : stage === 'fetch' ? 'uncertain' : 'response');
  assert.equal(f.requests.length, ['checkpoint', 'other'].includes(stage) ? 0 : 1);
  if (f.requests.length) assert.equal(f.requests[0].abortCount, 1);
  assert.equal(f.timers.size, 0);
});
for (const invalidate of ['owner/view', 'identity']) test('checkpoint refusal during body read is not falsely branded body-read: ' + invalidate, async () => {
  const stream = heldPreviewBody(new Uint8Array([1, 2])), f = await fixture({ fetch: async () => stream.response });
  const pending = f.request({ method: 'GET', body: undefined, contentType: undefined, responseType: 'image' }); pending.catch(() => {});
  await stream.entered; if (invalidate === 'identity') f.changeIdentity(); else f.close(); stream.finish();
  await assert.rejects(pending, error => { assert.equal(f.api.uploadRequestRefusalStage(error), 'checkpoint'); return errorIs('response', 200)(error); });
  assert.equal(f.requests[0].abortCount, 1);
});
test('refusal accessor never inspects getters, prototypes, forged or proxied errors', async () => {
  const f = await fixture({ fetch: async () => { throw Error('finite'); } }), branded = await failure(f);
  let reads = 0;
  const trap = () => { reads++; throw Error('reserved-example.invalid/getter-value'); };
  const unknown = Object.defineProperties({}, { name: { get: trap }, code: { get: trap }, status: { get: trap }, outcome: { get: trap } });
  const proxy = new Proxy({}, { get: trap, getPrototypeOf: trap, ownKeys: trap });
  const revoked = Proxy.revocable({}, {}); revoked.revoke();
  const forged = Object.create(f.api.PublicUploadRequestError.prototype), manual = new f.api.PublicUploadRequestError('finite', 'response', 200);
  for (const value of [undefined, null, false, 0, 'fetch', () => {}, unknown, proxy, revoked.proxy, forged, manual, new Proxy(branded, {})])
    assert.equal(f.api.uploadRequestRefusalStage(value), 'unavailable');
  for (const name of ['name', 'code', 'status', 'outcome']) Object.defineProperty(branded, name, { get: trap });
  Object.preventExtensions(branded);
  assert.equal(f.api.uploadRequestRefusalStage(branded), 'fetch'); assert.equal(reads, 0);
});

let leaseApi;
async function actualLease() {
  if (leaseApi) return leaseApi;
  const context = vm.createContext({ URL, Blob, Error, Object, Map, Set, Symbol, Number, Promise });
  const code = fs.readFileSync(new URL('../../platform-integrations/public-api/upload-preview.ts', import.meta.url), 'utf8');
  const module = new vm.SourceTextModule(stripTypeScriptTypes(code), { context });
  await module.link(() => { throw Error('The preview lease has no runtime imports'); }); await module.evaluate(); leaseApi = module.namespace; return leaseApi;
}
function previewSurface(createUploadPreviewLease, stageAccessor, error) {
  const root = fileURLToPath(new URL('../../frontend/', import.meta.url)), relative = 'src/lib/components/upload/UploadChapterPagesEntry.svelte', id = root + relative;
  const raw = fs.readFileSync(id, 'utf8'), mid = createFrontendOverlays(root).transform.handler(raw, id).code;
  const composed = applyPublicUploadOverlay(mid, relative);
  assert.match(composed, /<p[^>]+role="status" data-public-upload-preview-refusal={previewRefusal}>{previewError}<\/p>/);
  assert.doesNotMatch(composed, /{previewRefusal}<|console\.(?:log|warn|error).*previewRefusal/);
  const start = composed.indexOf('\tlet previewError = '), end = composed.indexOf('\n\tconst replacePage = ', start);
  assert.ok(start > 0 && end > start);
  const page = { id: 'file', file_hash: 'a'.repeat(64), file_size: '2', public_upload: { mimeType: 'image/png' } };
  const factory = new Function('createUploadPreviewLease', 'uploadPreview', 'uploadRequestRefusalStage', 'onDestroy', '$state', '$effect', 'untrack', 'owner', 'uploadSession', 'page', 'isDragging', 'isRemoteUploadFile',
    stripTypeScriptTypes(composed.slice(start, end), { mode: 'transform' }) + '; return { ready: () => previewLease.update(previewContext), get: () => ({ text: previewError, stage: previewRefusal }), dispose: () => previewLease.dispose() };');
  return factory(createUploadPreviewLease, async () => { throw error; }, stageAccessor, () => {}, value => value, callback => callback(), callback => callback(), { assert() {} }, { id: 'session' }, page, false, () => true);
}
test('genuine refusal brand survives the actual lease Error and composed existing preview-error attribute', async () => {
  const f = await fixture({ fetch: async () => { throw Error('reserved-example.invalid/private-value'); } }), error = await failure(f), api = await actualLease();
  let reads = 0;
  for (const name of ['code', 'name', 'status', 'outcome']) Object.defineProperty(error, name, { get() { reads++; throw Error('finite getter'); } });
  const surface = previewSurface(api.createUploadPreviewLease, f.api.uploadRequestRefusalStage, error);
  try { await surface.ready(); assert.deepEqual(surface.get(), { text: 'The signed private preview could not be verified. The staged image was kept.', stage: 'fetch' }); assert.equal(reads, 0); }
  finally { surface.dispose(); }
});
test('unknown getter/forged errors remain unavailable through the actual lease and preview surface', async () => {
  const f = await fixture(), api = await actualLease(); let reads = 0;
  const error = new Error('reserved-example.invalid/private-value');
  for (const name of ['code', 'name', 'status', 'outcome']) Object.defineProperty(error, name, { get() { reads++; throw Error('finite getter'); } });
  for (const value of [error, Object.create(f.api.PublicUploadRequestError.prototype), { code: 'fetch', message: 'reserved-example.invalid' }]) {
    const surface = previewSurface(api.createUploadPreviewLease, f.api.uploadRequestRefusalStage, value);
    try { await surface.ready(); assert.equal(surface.get().stage, 'unavailable'); assert.equal(surface.get().text, 'The signed private preview could not be verified. The staged image was kept.'); }
    finally { surface.dispose(); }
  }
  assert.equal(reads, 0);
});
test('a malformed error that defeats lease Error classification gets only the existing fallback and unavailable code', async () => {
  const f = await fixture(), api = await actualLease();
  const malformed = new Proxy({}, { getPrototypeOf() { throw Error('reserved-example.invalid/prototype-value'); } });
  const surface = previewSurface(api.createUploadPreviewLease, f.api.uploadRequestRefusalStage, malformed);
  try { await assert.rejects(surface.ready()); assert.deepEqual(surface.get(), { text: 'Private preview unavailable. The staged image was kept.', stage: 'unavailable' }); }
  finally { surface.dispose(); }
});
test('exact original transport counterfactual has the same error contract without a refusal code', { skip: !process.env.WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE }, async () => {
  const baseline = fs.readFileSync(process.env.WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE, 'utf8');
  assert.equal(digest(baseline), '9418845c867fd7fd0706756197900221e3cd73cd87a48133d5ed3ff6ef0d3056');
  const f = await fixture({ transportSource: baseline, fetch: async () => { throw Error('finite'); } }), old = await failure(f);
  assert.equal(f.api.uploadRequestRefusalStage, undefined); assert.equal(old.outcome, 'uncertain'); assert.equal(old.status, undefined);
  assert.deepEqual(Object.keys(old).sort(), ['bodyAvailable', 'name', 'outcome', 'status']); assert.equal(f.requests[0].abortCount, 1);
});

test('only exact native user discussion supports native 50000-byte bodies including worst-case JSON escapes',async()=>{
 const route='/api/v1/nostr/user/11111111-1111-8111-8111-111111111111/discussion';
 for(const body of ['x'.repeat(50000),'\x01'.repeat(50000)]){const f=await fixture(),bytes=text.encode(JSON.stringify({body}));await f.request({url:f.scope.origin+route,body:bytes});assert.equal(f.requests.length,1);assert.equal(f.templates[0].tags.find(row=>row[0]==='payload')[1],digest(bytes));assert.deepEqual(new Uint8Array(f.requests[0].init.body),bytes);}
 for(const path of [route+'/',route+'?extra=1',route.replace('/user/','/publisher/'),route.replace('11111111-1111-8111-8111-111111111111','native'),'/api/v1/upload/begin']){const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+path,body:text.encode(JSON.stringify({body:'x'.repeat(50000)}))}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);assert.equal(f.signerCalls,0);}
 const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route,body:new Uint8Array(393217)}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);
});
test('native discussion page supports twenty maximally escaped stored comments but retains a hard response bound',async()=>{
 const route='/api/v1/nostr/user/11111111-1111-8111-8111-111111111111/discussion',value=JSON.stringify({data:Array.from({length:20},()=>({body:'\x01'.repeat(50000)}))});assert.ok(text.encode(value).length>1024*1024);
 const f=await fixture({fetch:()=>new Response(value,{status:200,headers:{'content-type':'application/json'}})});assert.equal((await f.request({url:f.scope.origin+route+'?page=1',method:'GET',body:undefined,contentType:undefined})).body.data.length,20);
 const ordinary=await fixture({fetch:()=>new Response(value,{status:200,headers:{'content-type':'application/json'}})});await assert.rejects(()=>ordinary.request(),error=>error.outcome!=='unsent');
 const huge=await fixture({fetch:()=>new Response('{}',{status:200,headers:{'content-type':'application/json','content-length':String(8*1024*1024+1)}})});await assert.rejects(()=>huge.request({url:huge.scope.origin+route}),error=>error.outcome!=='unsent');
});
test('only exact native moderation POST allows bounded20000-byte details with worst JSON escaping',async()=>{const route='/api/v1/nostr/user/11111111-1111-8111-8111-111111111111/discussion/reports';for(const details of['x'.repeat(20000),'\x01'.repeat(20000)]){const f=await fixture(),bytes=text.encode(JSON.stringify({details}));await f.request({url:f.scope.origin+route,body:bytes});assert.equal(f.requests.length,1);assert.equal(f.templates[0].tags.find(row=>row[0]==='payload')[1],digest(bytes));assert.deepEqual(new Uint8Array(f.requests[0].init.body),bytes);}const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route,body:new Uint8Array(131073)}));assert.equal(f.requests.length,0);assert.equal(f.templates.length,0);});
test('native moderation larger body budget cannot widen source-report, methods, query, response type or sibling routes',async()=>{const root='/api/v1/nostr/user/11111111-1111-8111-8111-111111111111/discussion/reports';for(const patch of[{url:root+'/context'},{url:root+'/extra'},{url:root+'?foo=1'},{url:root,method:'PATCH'},{url:root,method:'PUT'},{url:root,responseType:'image'},{url:'/api/v1/report'},{url:root.replace('/11111111-1111-8111-8111-111111111111/','/NativeUser/')},{url:root+'/'}]){const f=await fixture();await assert.rejects(()=>f.request({...patch,url:f.scope.origin+patch.url,body:new Uint8Array(32769)}));assert.equal(f.requests.length,0);assert.equal(f.templates.length,0);}});

for(const kind of ['manga','chapter','group']){
 test('exact '+kind+' native report POST shares only the bounded native moderation budget',async()=>{
  const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion/reports';
  for(const details of ['x'.repeat(20000),'🌌'.repeat(5000),'\x01'.repeat(20000)]){const f=await fixture(),bytes=text.encode(JSON.stringify({details}));await f.request({url:f.scope.origin+route,body:bytes});assert.equal(f.requests.length,1);assert.equal(f.templates[0].tags.find(row=>row[0]==='payload')[1],digest(bytes));assert.deepEqual(new Uint8Array(f.requests[0].init.body),bytes);}
  const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route,body:new Uint8Array(131073)}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);assert.equal(f.templates.length,0);
 });
 test('generic '+kind+' report budget never widens protocol reports, unrelated kinds or context/inspection',async()=>{
  const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion/reports';
  for(const patch of [{url:route+'/context'},{url:route+'/extra'},{url:route+'/'},{url:route+'?extra=1'},{url:route,method:'PATCH'},{url:route,method:'PUT'},{url:route,method:'DELETE'},{url:route,method:'GET'},{url:route,responseType:'image'},{url:route.replace('/'+kind+'/','/publisher/')},{url:route.replace('/'+kind+'/','/author/')},{url:'/api/v1/report'},{url:'/api/v1/nostr/account/reports/native'}]){const f=await fixture();await assert.rejects(()=>f.request({...patch,url:f.scope.origin+patch.url,body:new Uint8Array(32769)}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);assert.equal(f.templates.length,0);}
 });
}

// Separately bounded repair: generic POST comments previously inherited32KiB,
// despite the actual controller and native storage admitting50,000 UTF8 bytes.
for(const kind of ['manga','chapter','group']){
 test('actual '+kind+' reviewed comment plans at32KiB and50K bytes reach exact-budget transport',async()=>{
  const {fixture:accountFixture,id,actor,loadSignedFixtureModule}=await import('./report-draft-fixture.mjs'),{createSignedNativeDiscussion}=await loadSignedFixtureModule('native-forum/signed-client'),threadId='ActualStoredThread';
  const a=accountFixture({handler:()=>({status:200,body:{result:'ok',nostr:{scope:'instance-account',source:'persisted-native-discussion',identity:'native_catalogue_resource',accountId:actor,targetKind:kind,targetId:id,threadId},data:{thread:{id:threadId,locked:false},threadVote:0,feed:{total:0,limit:20,page:1,data:[]},actions:{create:true,threadVote:false,replies:{}}}}})}),c=await createSignedNativeDiscussion(kind,id,threadId,a.check,a.deps);await c.read();
  const route='/api/v1/nostr/'+kind+'/'+id+'/discussion';
  try{for(const body of ['x'.repeat(32768),'x'.repeat(50000),'🌌'.repeat(12500),'\x01'.repeat(50000)]){
   const prepared=c.plan({operation:'create',body}),bytes=text.encode(JSON.stringify(prepared)),f=await fixture();assert.equal(prepared.expectedTargetId,id);assert.equal(prepared.expectedThreadId,threadId);await f.request({url:f.scope.origin+route,body:bytes});assert.equal(f.requests.length,1);assert.equal(f.templates[0].tags.find(row=>row[0]==='payload')[1],digest(bytes));assert.deepEqual(new Uint8Array(f.requests[0].init.body),bytes);
  }
  for(const body of ['x'.repeat(50001),'🌌'.repeat(12501)])assert.throws(()=>c.plan({operation:'create',body}));
  const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route,body:new Uint8Array(393217)}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);assert.equal(f.signerCalls,0);
  }finally{c.dispose();}
 });
 test('generic '+kind+' comment budget cannot widen methods,query,response,IDs,kinds or source APIs',async()=>{
  const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion';
  for(const patch of [{url:route+'/'},{url:route+'/extra'},{url:route+'?page=1'},{url:route,method:'PATCH'},{url:route,method:'PUT'},{url:route,method:'DELETE'},{url:route,method:'GET'},{url:route,responseType:'image'},{url:route.replace('/'+kind+'/','/publisher/')},{url:route.replace('/'+kind+'/','/author/')},{url:route.replace('11111111-1111-8111-8111-111111111111','native-id')},{url:'/api/v1/report'},{url:'/api/v1/upload/begin'}]){const f=await fixture();await assert.rejects(()=>f.request({...patch,url:f.scope.origin+patch.url,body:new Uint8Array(32769)}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);assert.equal(f.signerCalls,0);}
 });
 test('exact frozen pre-budget '+kind+' transport rejects otherwise valid50K payload',async()=>{
  const relative='weebdex-platform/platform-integrations/public-api/nostr-upload-request.ts';
  const beforeModerator=restoreModeratorTransportSource(relative,source,'raw');
  const baseline=restoreBundleSource('pre-generic-discussion-budget',relative,beforeModerator);
  assert.equal(digest(baseline),'b9079ea5b27f19ecdbbaaea5f9316804b563fa4400c04c19b26a5ed966640411');
  const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion',f=await fixture({transportSource:baseline});await assert.rejects(()=>f.request({url:f.scope.origin+route,body:text.encode(JSON.stringify({body:'x'.repeat(50000)}))}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);assert.equal(f.signerCalls,0);
 });
}

for(const kind of ['manga','chapter','group']){
 test('exact '+kind+' generic feed accepts20maximal native comments but stops at8MiB',async()=>{
  const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion',value=JSON.stringify({data:Array.from({length:20},()=>({body:'\x01'.repeat(50000)}))});assert.ok(text.encode(value).length>1024*1024);assert.ok(text.encode(value).length<8*1024*1024);
  for(const method of ['GET','POST']){const f=await fixture({fetch:()=>new Response(value,{status:200,headers:{'content-type':'application/json'}})});assert.equal((await f.request({url:f.scope.origin+route+(method==='GET'?'?page=1&order=desc':''),method,...(method==='GET'?{body:undefined,contentType:undefined}:{})})).body.data.length,20);}
  const f=await fixture({fetch:()=>new Response('{}',{status:200,headers:{'content-type':'application/json','content-length':String(8*1024*1024+1)}})});await assert.rejects(()=>f.request({url:f.scope.origin+route,method:'GET',body:undefined,contentType:undefined}),error=>error.outcome!=='unsent');
  const streamed=await fixture({fetch:()=>new Response(JSON.stringify('x'.repeat(8*1024*1024)),{status:200,headers:{'content-type':'application/json'}})});await assert.rejects(()=>streamed.request({url:streamed.scope.origin+route,method:'GET',body:undefined,contentType:undefined}),error=>error.outcome!=='unsent');assert.equal(streamed.requests.length,1);
 });
 test('generic '+kind+' response allowance stays absent on wrong method,POSTquery,nonJSON or unrelated endpoint',async()=>{
  const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion';
  for(const patch of [{url:route,method:'PATCH'},{url:route,method:'PUT'},{url:route,method:'DELETE'},{url:route+'?unexpected=1'},{url:route,contentType:undefined,body:undefined},{url:route+'/reports/context',method:'GET',body:undefined,contentType:undefined},{url:route+'/'},{url:route+'/extra'},{url:'/api/v1/report'},{url:route.replace('/'+kind+'/','/publisher/')}]){const f=await fixture({fetch:()=>new Response('{}',{status:200,headers:{'content-type':'application/json','content-length':String(1024*1024+1)}})});await assert.rejects(()=>f.request({...patch,url:f.scope.origin+patch.url}),error=>error.outcome!=='unsent');assert.equal(f.requests.length,1);}
  const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route,contentType:'text/plain',body:text.encode('notJSON')}),error=>error.outcome==='unsent');assert.equal(f.requests.length,0);
 });
 test('exact frozen pre-budget '+kind+' transport rejects otherwise valid6MB feed',async()=>{
  const relative='weebdex-platform/platform-integrations/public-api/nostr-upload-request.ts';
  const beforeModerator=restoreModeratorTransportSource(relative,source,'raw');
  const baseline=restoreBundleSource('pre-generic-discussion-budget',relative,beforeModerator);
  assert.equal(digest(baseline),'b9079ea5b27f19ecdbbaaea5f9316804b563fa4400c04c19b26a5ed966640411');const route='/api/v1/nostr/'+kind+'/11111111-1111-8111-8111-111111111111/discussion';const value=JSON.stringify({data:Array.from({length:20},()=>({body:'\x01'.repeat(50000)}))}),f=await fixture({transportSource:baseline,fetch:()=>new Response(value,{status:200,headers:{'content-type':'application/json','content-length':String(text.encode(value).length)}})});await assert.rejects(()=>f.request({url:f.scope.origin+route+'?page=1',method:'GET',body:undefined,contentType:undefined}),error=>error.outcome!=='unsent');assert.equal(f.requests.length,1);
 });
}
