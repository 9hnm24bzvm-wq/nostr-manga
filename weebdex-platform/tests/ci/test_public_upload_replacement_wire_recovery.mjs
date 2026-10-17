#!/usr/bin/env node
// Finite exact-byte replacement transport seam. Actual production serializer and
// transport, Node SHA-256, deliberately fake signer/proof/service/fetch. No real
// signature verification, HTTP, browser, decoder, database or publication proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const root = new URL('../../platform-integrations/public-api/', import.meta.url);
const scope = { origin: 'https://replacement.invalid', serviceKey: 'a'.repeat(64), catalogNamespace: '11111111-1111-8111-8111-111111111111' };
const url = scope.origin + '/api/v1/nostr/uploads/22222222-2222-8222-8222-222222222222/files/33333333-3333-8333-8333-333333333333/replace';
const png = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10, 4, 3, 2, 1]);

async function fixture() {
  const requests = [], templates = [], context = vm.createContext({ URL, TextEncoder, TextDecoder, Uint8Array, ArrayBuffer, AbortController, performance });
  const source = name => stripTypeScriptTypes(fs.readFileSync(new URL(name, root), 'utf8'), { mode: 'transform' });
  const forbidden = () => { throw Error('Unexpected real primitive in finite replacement seam'); };
  const nostr = new vm.SyntheticModule(['origin', 'signExact'], function () { this.setExport('origin', input => new URL(input).origin); this.setExport('signExact', forbidden); }, { context });
  const codec = new vm.SyntheticModule(['hash', 'b64'], function () { this.setExport('hash', forbidden); this.setExport('b64', forbidden); }, { context });
  const transport = new vm.SourceTextModule(source('nostr-upload-request.ts'), { context });
  await transport.link(specifier => { if (specifier.endsWith('/nostr.js')) return nostr; if (specifier.endsWith('/codec.js')) return codec; throw Error('Unexpected dependency ' + specifier); }); await transport.evaluate();
  const multipart = new vm.SourceTextModule(source('upload-multipart.ts'), { context }); await multipart.link(forbidden); await multipart.evaluate();
  const identity = { owner: 'b'.repeat(64), generation: 1, check() {}, signer: { async signEvent(event) { templates.push(structuredClone(event)); return { ...event, id: 'finite-fake-id', sig: 'finite-fake-signature' }; } } };
  const request = transport.namespace.createNostrUploadRequest({ scope, identity, assertCurrent() {} }, {
    hash, b64: bytes => Buffer.from(bytes).toString('base64'), now: () => 1700000000, clock: () => 0, nonce: () => 'finite_replacement_nonce', setTimer: () => 0, clearTimer() {},
    signExact: (signer, owner, event) => signer.signEvent({ ...event, pubkey: owner }),
    fetch: async (target, init) => { requests.push({ target, init }); return new Response('{"result":"ok"}', { status: 200, headers: { 'content-type': 'application/json' } }); },
  });
  return { requests, templates, request, serialize: multipart.namespace.serializeUploadReplacementMultipart };
}

test('replacement expectedVersion and file bytes are bound to one fake NIP98 payload and exact credential-free dispatch', async () => {
  const f = await fixture(), file = { name: 'original replacement.png', type: 'image/png', size: png.length, arrayBuffer: async () => png.slice().buffer };
  const serialized = await f.serialize(file, 17, { boundary: 'finite-replacement-boundary-12345' }), expected = serialized.body.slice();
  const result = await f.request({ url, method: 'POST', body: serialized.body, contentType: serialized.contentType });
  assert.equal(result.status, 200); assert.equal(f.requests.length, 1); assert.equal(f.templates.length, 1);
  const { target, init } = f.requests[0], proof = JSON.parse(Buffer.from(init.headers.authorization.slice(6), 'base64')), tags = Object.fromEntries(proof.tags);
  assert.equal(target, url); assert.equal(tags.u, url); assert.equal(tags.method, 'POST'); assert.equal(tags.service, scope.serviceKey); assert.equal(tags.payload, hash(expected));
  assert.deepEqual(init.body, expected); assert.equal(init.headers['content-type'], serialized.contentType); assert.equal(init.credentials, 'omit'); assert.equal(init.redirect, 'error'); assert.equal(init.cache, 'no-store');
  const head = new TextDecoder().decode(expected);
  assert.match(head, /^--finite-replacement-boundary-12345\r\nContent-Disposition: form-data; name="expectedVersion"\r\n\r\n17\r\n--finite-replacement-boundary-12345\r\nContent-Disposition: form-data; name="file";/);
  assert.equal(head.match(/name="expectedVersion"/g).length, 1); assert.equal(head.match(/name="file"/g).length, 1);
});
