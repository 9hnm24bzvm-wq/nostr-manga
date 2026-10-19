// Finite EventEmitter/fake-verifier fixture. No browser, network or genuine
// signature is used. --json exports only actual fixed observer failure codes.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { observePublicUpload, uploadDigest } from '../../scripts/public-upload-recovery-observer.mjs';

export const origin = 'http://127.0.0.1:50123', owner = 'a'.repeat(64), service = 'b'.repeat(64);
export const accountId = '12345678-1234-8234-8234-123456789abc';
export const otherAccountId = 'abcdef12-1234-8234-8234-123456789abc';
export const scope = { origin, owner, service, verify: () => true };
export const discoveryPath = '/api/v1/upload?expectedAccountId=' + accountId;
export const encode = event => 'Nostr ' + Buffer.from(JSON.stringify(event)).toString('base64');
export function seal(event) {
  event.id = uploadDigest(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]));
  return event;
}
export function eventFor({ method = 'GET', pathname = discoveryPath, body = null, index = 1 } = {}) {
  return seal({ kind: 27235, created_at: Math.floor(Date.now() / 1000), content: '', pubkey: owner,
    tags: [['u', origin + pathname], ['method', method], ['service', service], ['nonce', 'explicit_fake_query_nonce_' + index],
      ...(method === 'GET' ? [] : [['payload', uploadDigest(body ?? Buffer.alloc(0))]])], sig: 'c'.repeat(128) });
}
export function requestFor(options = {}) {
  const { method = 'GET', pathname = discoveryPath, body = null, headers = {}, nativeError = 'net::ERR_ABORTED' } = options;
  const event = options.event ?? eventFor(options);
  return { url: () => origin + pathname, method: () => method, headers: () => ({ authorization: encode(event), ...headers }),
    postDataBuffer: () => body, failure: () => ({ errorText: nativeError }) };
}
export function harness({ bind = true, observe = observePublicUpload, scopeOverride = {} } = {}) {
  const page = new EventEmitter(), observerScope = { ...scope, ...scopeOverride }, observer = observe(page, observerScope);
  if (bind) observer.bindAccount(accountId);
  return { page, observer, observerScope, emit: options => { const request = requestFor(options); page.emit('request', request); return request; } };
}
export function finish(page, request, status = 200) {
  page.emit('response', { request: () => request, status: () => status }); page.emit('requestfinished', request);
}

// Execute only the actual checked-in client's signedPath/request tail, with a
// recording request stand-in. No independently re-created query construction.
export const clientSource = readFileSync(new URL('../../platform-integrations/public-api/upload-client.ts', import.meta.url), 'utf8');
export async function actualClientDiscovery() {
  const start = clientSource.indexOf('    const signedPath = '), end = clientSource.indexOf('\n  }\n  function basicSession', start);
  assert.ok(start > 0 && end > start);
  const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
  let observed;
  const request = async value => { assert.equal(observed, undefined); observed = value; return { status: 404 }; };
  const result = await new AsyncFunction('method', 'path', 'owned', 'request', 'guard', 'payload', 'encoder', 'responseType', clientSource.slice(start, end))(
    'GET', '/upload', { account: { actor: accountId, scope: { origin } } }, request, () => {}, undefined, new TextEncoder(), 'json');
  assert.equal(result.status, 404); assert.equal(observed.url, origin + discoveryPath);
  return observed;
}

export function observerFailureCases() {
  const result = {};
  for (const name of ['valid_discovery', 'malformed_query', 'duplicate_native_terminal', 'native_abort']) {
    const h = harness();
    try {
      const request = h.emit(name === 'malformed_query' ? { pathname: discoveryPath + '&extra=1' } : {});
      if (name === 'valid_discovery' || name === 'duplicate_native_terminal') finish(h.page, request);
      if (name === 'duplicate_native_terminal') h.page.emit('requestfinished', request);
      if (name === 'native_abort') h.page.emit('requestfailed', request);
      result[name] = h.observer.snapshot().failures;
    } finally { h.observer.close(); }
  }
  return result;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  assert.deepEqual(process.argv.slice(2), ['--json']);
  process.stdout.write(JSON.stringify(observerFailureCases()) + '\n');
}
