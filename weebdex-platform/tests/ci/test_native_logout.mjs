// Actual native logout coordinator with finite browser-HTTP boundaries.
// No native credentials, server, browser, signer or external request is used.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const frontend = process.env.WEEBDEX_FRONTEND_ROOT; assert(frontend);
const require = createRequire(path.join(frontend, 'package.json')), ts = require('typescript');
const source = fs.readFileSync(new URL('../../platform-integrations/frontend-behavior/native-logout.ts', import.meta.url), 'utf8');
const api = await import('data:text/javascript;base64,' + Buffer.from(ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText).toString('base64'));
const json = (value, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json' } });
function fixture(options = {}) {
  const calls = []; let alive = true, active = !options.absent, cancellations = 0;
  const token = 't'.repeat(43), session = 'synthetic-session-A', account = 'SyntheticAccountA';
  const deps = { origin: 'https://site.invalid', authBase: '/auth', apiBase: '/api', account, assertCurrent() { if (!alive) throw Error('Synthetic account or route changed'); },
    async fetch(url, init) {
      calls.push({ path: new URL(url).pathname, url, init });
      assert.equal(new URL(url).origin, deps.origin); assert.equal(init.credentials, 'same-origin'); assert.equal(init.redirect, 'error'); assert.equal(init.cache, 'no-store'); assert.equal(init.referrerPolicy, 'no-referrer');
      assert.equal(init.headers['X-Weebdex-Expected-Account'], account);
      if (options.changeAt === calls.length) alive = false;
      if (options.respond) { const custom = options.respond({ url, init, calls, cancelled: () => cancellations++ }); if (custom) return custom; }
      if (url.endsWith('/sessions/whoami')) return active ? json({ active: true, id: session, identity: { id: account } }) : json({}, 401);
      if (url.endsWith('/logout/browser')) return json({ logout_token: token, session_id: options.changedFlow ? 'synthetic-session-B' : session, account_id: account, logout_url: deps.origin + '/ignored?token=' + token });
      if (url.endsWith('/self-service/logout')) {
        assert.equal(init.method, 'POST'); assert.equal(init.headers['Content-Type'], 'application/json'); assert.deepEqual(JSON.parse(init.body), { token, expected_session_id: session });
        if (!options.keepActive) active = false;
        if (options.lost) throw Error('Synthetic dispatch response lost');
        return json({}, options.postStatus ?? 200);
      }
      assert(url.endsWith('/api/auth/check')); assert.equal(init.method, 'GET'); return json({}, active ? 200 : 401);
    },
  };
  return { deps, calls, token, cancellations: () => cancellations, change() { alive = false; } };
}
test('native POST works with no-referrer and confirms server absence before success; token never enters URL', async () => {
  const f = fixture(), result = await api.performNativeLogout(f.deps);
  assert.equal(result.state, 'signed-out'); assert.deepEqual(f.calls.map(row => row.init.method), ['GET', 'GET', 'POST', 'GET']);
  assert(f.calls.every(row => !row.url.includes(f.token))); assert.equal(JSON.stringify(result).includes(f.token), false);
});
test('already absent browser must pass the authoritative native status check without a logout mutation', async () => {
  const f = fixture({ absent: true }); assert.equal((await api.performNativeLogout(f.deps)).state, 'signed-out'); assert.deepEqual(f.calls.map(row => row.path), ['/auth/sessions/whoami', '/api/auth/check']);
});
test('same-account reauthentication between whoami and flow creation refuses stale session without POST', async () => {
  const f = fixture({ changedFlow: true }); assert.equal((await api.performNativeLogout(f.deps)).state, 'not-confirmed'); assert.equal(f.calls.some(row => row.init.method === 'POST'), false);
});
for (const options of [{ keepActive: true }, { keepActive: true, postStatus: 403 }, { keepActive: true, lost: true }]) test('active or newer native session is never called logged out: ' + JSON.stringify(options), async () => {
  const f = fixture(options); assert.equal((await api.performNativeLogout(f.deps)).state, 'not-confirmed'); assert.equal(f.calls.filter(row => row.init.method === 'POST').length, 1);
});
test('lost POST response reconciles by one read only; no automatic logout replay', async () => {
  const f = fixture({ lost: true }); assert.equal((await api.performNativeLogout(f.deps)).state, 'signed-out'); assert.equal(f.calls.filter(row => row.init.method === 'POST').length, 1);
});
for (const status of [200, 403, 409, 500]) test('read-only check does not treat native status ' + status + ' as absence', async () => {
  const f = fixture({ respond: () => json({}, status) }); assert.equal((await api.checkNativeLogout(f.deps)).state, 'not-confirmed'); assert.deepEqual(f.calls.map(row => row.init.method), ['GET']);
});
for (const changeAt of [1, 2, 3, 4]) test('account/navigation change at response boundary ' + changeAt + ' cannot return local success or replay', async () => {
  const f = fixture({ changeAt }); await assert.rejects(api.performNativeLogout(f.deps), /changed/); assert(f.calls.filter(row => row.init.method === 'POST').length <= 1);
});
for (const kind of ['refused', 'wrong-content-type', 'oversized']) test('owned response body is cancelled on ' + kind, async () => {
  const f = fixture({ respond: ({ cancelled }) => new Response(new ReadableStream({ start(controller) { controller.enqueue(new TextEncoder().encode(kind === 'oversized' ? 'x'.repeat(17000) : '{}')); }, cancel: cancelled }), { status: kind === 'refused' ? 403 : 200, headers: { 'content-type': kind === 'wrong-content-type' ? 'text/plain' : 'application/json' } }) });
  assert.equal((await api.performNativeLogout(f.deps)).state, 'not-confirmed'); assert.equal(f.cancellations(), 1); assert.equal(f.calls.length, 1);
});
test('malformed verified-session JSON cannot start a logout request', async () => {
  const f = fixture({ respond: () => new Response('{broken', { headers: { 'content-type': 'application/json' } }) }); assert.equal((await api.performNativeLogout(f.deps)).state, 'not-confirmed'); assert.equal(f.calls.length, 1);
});
test('late event-loop deadline is checked before continuing to flow creation', async () => {
  const f = fixture({ respond: () => { const until = performance.now() + 15; while (performance.now() < until) {} return json({ active: true, id: 'session', identity: { id: 'SyntheticAccountA' } }); } });
  assert.equal((await api.performNativeLogout({ ...f.deps, timeoutMs: 5 })).state, 'not-confirmed'); assert.equal(f.calls.length, 1);
});
test('external or query-bearing auth configuration is refused without network contact', async () => {
  for (const authBase of ['https://other.invalid/auth', '/auth?token=bad', '/auth#bad']) { const f = fixture(); assert.equal((await api.performNativeLogout({ ...f.deps, authBase })).state, 'not-confirmed'); assert.equal(f.calls.length, 0); }
});
