// A small passive observer for the disposable public-upload journey. No route
// interception, synthetic application receipt, retry, or response substitution.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { signedEventEnvelope } from './browser-signed-event-envelope.mjs';
export const uploadDigest = value => createHash('sha256').update(value).digest('hex');
const route = /^\/api\/v1\/(?:upload(?:\/.*)?|nostr\/(?:account-context|uploads\/[0-9a-f-]+(?:\/files\/[0-9a-f-]+\/(?:preview|replace))?))$/;
const accountIdPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export function validateUploadHttpEvent({ authorization, url, method, body, origin, service, owner, expectedAccountId, verify, now = Date.now() }) {
  assert.ok(['GET', 'POST', 'PATCH', 'DELETE'].includes(method));
  assert.equal(typeof authorization, 'string'); assert.ok(authorization.length <= 16384 && authorization.startsWith('Nostr '));
  const encoded = authorization.slice(6); assert.match(encoded, /^[A-Za-z0-9+/]+={0,2}$/);
  const bytes = Buffer.from(encoded, 'base64'); assert.equal(bytes.toString('base64'), encoded);
  let event; try { event = JSON.parse(bytes.toString('utf8')); } finally { bytes.fill(0); }
  signedEventEnvelope(event);
  const parsed = new URL(url); assert.equal(parsed.origin, origin); assert.equal(parsed.href, url); assert.ok(!url.includes('#') && route.test(parsed.pathname));
  // The real upload client binds its initial discovery GET to the account it
  // just verified. Admit only that exact canonical URL extension, never a
  // decoded/rebuilt substitute for the full URL covered by the signed u tag.
  if (url.includes('?')) {
    assert.equal(method, 'GET'); assert.equal(parsed.pathname, '/api/v1/upload');
    assert.equal(typeof expectedAccountId, 'string'); assert.match(expectedAccountId, accountIdPattern);
    assert.equal(parsed.search, '?expectedAccountId=' + expectedAccountId);
  }
  assert.equal(event.kind, 27235); assert.equal(event.content, ''); assert.equal(event.pubkey, owner);
  assert.ok(Math.abs(event.created_at - Math.floor(now / 1000)) <= 120);
  const id = uploadDigest(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]));
  assert.equal(event.id, id); assert.equal(verify(event.sig, id, owner), true);
  const wanted = new Set(['u', 'method', 'service', 'nonce', ...(method === 'GET' ? [] : ['payload'])]);
  assert.equal(event.tags.length, wanted.size);
  const tags = new Map(); for (const tag of event.tags) { assert.equal(tag.length, 2); assert.ok(wanted.has(tag[0]) && !tags.has(tag[0])); tags.set(tag[0], tag[1]); }
  assert.equal(tags.get('u'), url); assert.equal(tags.get('method'), method); assert.equal(tags.get('service'), service);
  assert.match(tags.get('nonce'), /^[A-Za-z0-9_-]{16,128}$/);
  if (body !== null && method !== 'GET') assert.equal(tags.get('payload'), uploadDigest(body));
  if (method === 'GET') assert.equal(body?.length ?? 0, 0);
  return { event_id: id, method, path: parsed.pathname, signed_payload_sha256: tags.get('payload') ?? null,
    observed_body_bytes: body === null ? null : body.length, observed_body_sha256: body === null ? null : uploadDigest(body),
    body_observation: body === null ? 'unavailable' : 'actual-playwright-bytes', nonce: tags.get('nonce') };
}
export function observePublicUpload(page, scope) {
  const rows = [], requests = new Map(), events = new Set(), nonces = new Set(), failures = [];
  let closed = false, expectedAccountId;
  const fail = code => { if (failures.length < 16) failures.push(code); };
  const onRequest = request => {
    try {
      const url = new URL(request.url()); if (url.origin !== scope.origin || !route.test(url.pathname)) return;
      assert.equal(closed, false); assert.ok(rows.length < 128, 'Signed upload browser request cap');
      const headers = request.headers(); assert.equal(headers.cookie, undefined, 'Signed upload sent cookie authority');
      const body = request.postDataBuffer(); if (body !== null) assert.ok(body.length <= 32768, 'Observed request body cap');
      const row = validateUploadHttpEvent({ ...scope, expectedAccountId, url: request.url(), method: request.method(), authorization: headers.authorization,
        body });
      assert.ok(!events.has(row.event_id) && !nonces.has(row.nonce), 'Duplicate signed request or nonce');
      events.add(row.event_id); nonces.add(row.nonce); delete row.nonce;
      if (request.method() !== 'GET') assert.ok(rows.filter(value => value.method !== 'GET').length < 32, 'Mutation cap');
      row.native = 'pending'; row.status = null; rows.push(row); requests.set(request, row);
    } catch { fail('signed_request_invalid'); }
  };
  const onResponse = response => { const row = requests.get(response.request()); if (row) row.status = response.status(); };
  const onFinished = request => { const row = requests.get(request); if (row) { if (row.native !== 'pending') fail('duplicate_native_terminal'); row.native = 'finished'; } };
  const onFailed = request => { const row = requests.get(request); if (row) { if (row.native !== 'pending') fail('duplicate_native_terminal'); row.native = 'failed'; row.native_error = request.failure()?.errorText === 'net::ERR_ABORTED' ? 'net::ERR_ABORTED' : 'other'; fail('signed_native_failed'); } };
  page.on('request', onRequest); page.on('response', onResponse); page.on('requestfinished', onFinished); page.on('requestfailed', onFailed);
  const assertHealthy = () => { assert.equal(failures.length, 0, failures.join(',')); };
  return {
    bindAccount(value) { assert.equal(closed, false); assert.equal(expectedAccountId, undefined); assert.equal(typeof value, 'string'); assert.match(value, accountIdPattern); expectedAccountId = value; },
    rows, assertHealthy, mutationCount: () => rows.filter(row => row.method !== 'GET').length,
    settled: () => { assertHealthy(); return rows.every(row => row.native === 'finished' && Number.isInteger(row.status) && row.status >= 200 && row.status <= 599); },
    snapshot: () => ({ closed, failures: [...failures], rows: rows.map(row => ({ ...row })) }),
    close() { if (closed) return; closed = true; page.off('request', onRequest); page.off('response', onResponse); page.off('requestfinished', onFinished); page.off('requestfailed', onFailed); },
  };
}
// Independent real HTTP reads use the same ephemeral synthetic identity, with
// a fresh exact event per read. Only the visible application performs mutations.
export function createUploadIndependentReader({ origin, owner, service, sign, fetch: transport = fetch }) {
  const rows = []; let closed = false, pending = 0;
  async function get(path) {
    assert.equal(closed, false); assert.equal(pending, 0); assert.ok(rows.length < 96);
    const url = new URL(path, origin); assert.equal(url.origin, origin); assert.equal(url.pathname, path); assert.ok(route.test(path));
    const template = { kind: 27235, created_at: Math.floor(Date.now() / 1000), tags: [['u', url.href], ['method', 'GET'], ['service', service], ['nonce', randomUUID()]], content: '' };
    pending++; // Reserve before the signer callback can re-enter this owner.
    const controller = new AbortController(), timer = setTimeout(() => controller.abort(), 5000);
    const chunks = []; let reader, response, bytes;
    try {
      const event = await sign(template); assert.equal(closed, false); assert.equal(controller.signal.aborted, false);
      response = await transport(url, { headers: { authorization: 'Nostr ' + Buffer.from(JSON.stringify(event)).toString('base64'), accept: 'application/json' },
        method: 'GET', redirect: 'error', cache: 'no-store', credentials: 'omit', signal: controller.signal });
      assert.match(response.headers.get('content-type') ?? '', /^application\/json(?:;|$)/);
      reader = response.body?.getReader(); let size = 0;
      if (reader) for (;;) { const part = await reader.read(); if (part.done) break; assert.ok(part.value.byteLength <= 65536 - size); size += part.value.byteLength; chunks.push(Buffer.from(part.value)); }
      bytes = Buffer.concat(chunks); const body = JSON.parse(bytes.toString('utf8'));
      rows.push({ event_id: event.id, method: 'GET', path, status: response.status, bytes: bytes.length, sha256: uploadDigest(bytes), complete: true });
      return { status: response.status, body };
    } finally {
      clearTimeout(timer); if (reader) { try { await reader.cancel(); } catch {} reader.releaseLock(); }
      else { try { await response?.body?.cancel(); } catch {} }
      for (const chunk of chunks) chunk.fill(0); bytes?.fill(0); pending--;
    }
  }
  return { get, snapshot: () => ({ closed, pending, rows: rows.map(row => ({ ...row })) }), close() { assert.equal(pending, 0); closed = true; } };
}
