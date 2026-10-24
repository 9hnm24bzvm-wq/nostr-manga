// Passive proof observer for actual browser HTTP. It never substitutes a route,
// response, signer, application state, or durable fixture outcome.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { signedEventEnvelope } from './browser-signed-event-envelope.mjs';
export const featureDigest = value => createHash('sha256').update(value).digest('hex');
export const featureUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export function validateFeatureProof({ authorization, url, method, body, origin, service, owner, verify, now = Date.now() }) {
  assert.ok(['GET', 'POST', 'PUT', 'DELETE'].includes(method));
  assert.equal(typeof authorization, 'string'); assert.ok(authorization.startsWith('Nostr ') && authorization.length <= 16384);
  const encoded = authorization.slice(6); assert.match(encoded, /^[A-Za-z0-9+/]+={0,2}$/);
  const bytes = Buffer.from(encoded, 'base64'); assert.equal(bytes.toString('base64'), encoded);
  let event; try { event = JSON.parse(bytes.toString('utf8')); } finally { bytes.fill(0); }
  signedEventEnvelope(event);
  const parsed = new URL(url); assert.equal(parsed.origin, origin); assert.equal(parsed.href, url); assert.equal(parsed.hash, '');
  assert.match(parsed.pathname, /^\/api\/v1\/(?:nostr\/account-context|report(?:\/reasons\/manga)?|manga(?:\/draft)?(?:\/[0-9a-f-]+(?:\/commit)?)?|chapter\/[0-9a-f-]+)$/);
  assert.equal(event.kind, 27235); assert.equal(event.content, ''); assert.equal(event.pubkey, owner);
  assert.ok(Math.abs(event.created_at - Math.floor(now / 1000)) <= 120);
  const digest = featureDigest(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]));
  assert.equal(event.id, digest); assert.equal(verify(event.sig, digest, owner), true);
  const names = ['u', 'method', 'service', 'nonce', ...(method === 'GET' ? [] : ['payload'])];
  assert.equal(event.tags.length, names.length);
  const tags = new Map();
  for (const tag of event.tags) { assert.equal(tag.length, 2); assert.ok(names.includes(tag[0]) && !tags.has(tag[0])); tags.set(tag[0], tag[1]); }
  assert.equal(tags.get('u'), url); assert.equal(tags.get('method'), method); assert.equal(tags.get('service'), service);
  assert.match(tags.get('nonce'), /^[A-Za-z0-9_-]{16,128}$/);
  if (method === 'GET') assert.equal(body?.length ?? 0, 0);
  else { assert.ok(Buffer.isBuffer(body) && body.length > 0 && body.length <= 8192); assert.equal(tags.get('payload'), featureDigest(body)); assert.equal(parsed.search, ''); }
  return { id: digest, nonce: tags.get('nonce'), method, path: parsed.pathname };
}
export function observeSignedFeatures(page, scope) {
  const requests = new Map(), events = new Set(), nonces = new Set(), tasks = new Set(), cancellations = new Set();
  let invalid = false, closed = false;
  async function capture(value, entry) {
    let bytes, timer, cancel;
    try {
      // headers() deliberately omits security/cookie headers in Playwright.
      // Keep the complete-header read pending until it succeeds or fails closed.
      const headers = await Promise.race([value.allHeaders(), new Promise((_, reject) => {
        cancel = () => reject(Error('Complete request headers unavailable'));
        cancellations.add(cancel); timer = setTimeout(cancel, Math.min(scope.headersTimeoutMs ?? 2000, 2000));
      })]);
      assert.equal(closed, false); assert.equal(headers.cookie, undefined);
      const method = value.method();
      if (method === 'GET' && headers.authorization === undefined) {
        assert.ok(!/^\/api\/v1\/(?:nostr\/account-context|manga\/draft(?:\/|$)|report(?:\/|$))/.test(new URL(value.url()).pathname), 'Private/account/reason reads must be signed'); return;
      }
      bytes = value.postDataBuffer();
      const proof = validateFeatureProof({ ...scope, authorization: headers.authorization, url: value.url(), method, body: bytes });
      assert.ok(!events.has(proof.id) && !nonces.has(proof.nonce)); events.add(proof.id); nonces.add(proof.nonce);
      Object.assign(entry.row, { method: proof.method, path: proof.path, body: bytes ? JSON.parse(bytes.toString('utf8')) : undefined });
      if(proof.method==='GET')entry.row.query=new URL(value.url()).search;
      entry.signed = true; assert.equal(entry.failed, false);
    } catch { invalid = true; }
    finally { clearTimeout(timer); cancellations.delete(cancel); bytes?.fill(0); }
  }
  const request = value => {
    try {
      const url = new URL(value.url());
      if (url.origin !== scope.origin) return;
      if (/^\/api\/report(?:\/|$)/.test(url.pathname)) throw Error('Native report fallback forbidden');
      if (value.method() !== 'GET' && /^\/api\/(?:report|manga|chapter)(?:\/|$)/.test(url.pathname)) throw Error('Native mutation fallback forbidden');
      if (!url.pathname.startsWith('/api/v1/')) return;
      assert.equal(closed, false); assert.ok(requests.size < 128); assert.equal(requests.has(value), false);
      const entry = { signed: false, failed: false, row: { status: null, finished: false } };
      requests.set(value, entry); // Response/terminal events may precede allHeaders().
      const task = capture(value, entry); tasks.add(task); void task.finally(() => tasks.delete(task));
    } catch { invalid = true; }
  };
  const response = value => { const entry = requests.get(value.request()); if (entry) entry.row.status = value.status(); };
  const finished = value => { const entry = requests.get(value); if (entry) { if (entry.row.finished) invalid = true; entry.row.finished = true; } };
  const failed = value => { const entry = requests.get(value); if (entry) { entry.failed = true; if (entry.signed) invalid = true; } };
  page.on('request', request); page.on('response', response); page.on('requestfinished', finished); page.on('requestfailed', failed);
  const healthy = () => assert.equal(invalid, false, 'Signed feature HTTP proof or terminal invalid');
  const rows = () => [...requests.values()].filter(entry => entry.signed).map(entry => entry.row);
  return {
    healthy,
    settled() { healthy(); return tasks.size === 0 && rows().every(row => row.finished && Number.isInteger(row.status)); },
    writes() { healthy(); return rows().filter(row => row.method !== 'GET'); },
    assertWrite(index, expected) { healthy(); assert.equal(tasks.size, 0); const writes = rows().filter(row => row.method !== 'GET'); assert.equal(writes.length, index + 1); assert.deepEqual(writes[index], { ...expected, finished: true }); },
    assertRead({path,query,status=200}) { healthy();assert.equal(tasks.size,0);const expected=[...new URLSearchParams(query)].sort();assert.ok(rows().some(row=>row.method==='GET'&&row.path===path&&row.status===status&&row.finished&&JSON.stringify([...new URLSearchParams(row.query)].sort())===JSON.stringify(expected)), 'Exact signed report inspection read missing'); },
    count() { healthy(); return rows().length; },
    async close() { if (closed) return; closed = true; page.off('request', request); page.off('response', response); page.off('requestfinished', finished); page.off('requestfailed', failed); for (const cancel of cancellations) cancel(); await Promise.all(tasks); for (const entry of requests.values()) delete entry.row.body; },
  };
}
