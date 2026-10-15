// Reconstructed helper seams, finite adapters only; no browser or network claim.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
const file = new URL('../../platform-integrations/local-library/enrollment-policy.ts', import.meta.url);
const source = fs.readFileSync(file, 'utf8');
const origin = 'https://fixture.example.invalid';
const key = '1'.repeat(64);
const document = enabled => ({ enabled, mode: enabled ? 'self_registration' : 'operator_only', endpoint: '/enrollment', scope: 'personal_account_only' });
function fixture({ body = document(true), available = true, allowed = true, ok = true, oversized = false, staleAt, proofFails = false } = {}) {
  const calls = []; let current = true;
  const check = () => { if (!current) throw Error('stale'); };
  const context = vm.createContext({ Error, JSON, Array, TextDecoder, AbortSignal,
    capabilities: async () => { calls.push('capabilities'); if (staleAt === 'capabilities') current = false; return { available }; },
    assertAllowedRelay: value => { calls.push('allowed'); if (!allowed) throw Error('denied'); assert.equal(value, origin); return value; },
    proveService: async (value, expected) => { calls.push('proof'); assert.equal(value, origin); assert.equal(expected, key); if (proofFails) throw Error('proof'); if (staleAt === 'proof') current = false; },
    fetch: async (url, options) => { calls.push('fetch'); assert.equal(url, origin + '/enrollment'); assert.equal(options.method, 'GET'); assert.equal(options.credentials, 'omit'); assert.equal(options.redirect, 'error'); assert.equal(options.cache, 'no-store'); assert.ok(options.signal); if (staleAt === 'fetch') current = false; return { ok }; },
    bytesResponse: async (response, limit) => { calls.push('bytes'); assert.equal(limit, 4096); if (oversized) throw Error('oversized'); if (staleAt === 'bytes') current = false; return new TextEncoder().encode(JSON.stringify(body)); },
  });
  const script = source.replace(/^import[^;]+;\s*$/gm, '').replace('export async function', 'async function');
  const run = vm.runInContext(stripTypeScriptTypes(script, { mode: 'transform' }) + '; enrollmentPolicy', context);
  return { calls, read: () => run({ origin, serviceKey: key, selected: false }, check) };
}
for (const enabled of [true, false]) test('validated enrollment policy is read-only: ' + enabled, async () => {
  const f = fixture({ body: document(enabled) }); assert.equal((await f.read()).enabled, enabled); assert.deepEqual(f.calls, ['capabilities','allowed','proof','fetch','bytes']);
});
test('unavailable instance does not perform proof or policy fetch', async () => { const f = fixture({ available: false }); await assert.rejects(f.read()); assert.deepEqual(f.calls, ['capabilities']); });
test('disallowed recipient is rejected before service proof or network', async () => { const f = fixture({ allowed: false }); await assert.rejects(f.read()); assert.deepEqual(f.calls, ['capabilities','allowed']); });
test('failed service proof cannot fetch policy', async () => { const f = fixture({ proofFails: true }); await assert.rejects(f.read()); assert.equal(f.calls.includes('fetch'), false); });
test('HTTP failure and oversized documents fail closed', async () => { for (const options of [{ ok: false }, { oversized: true }]) await assert.rejects(fixture(options).read()); });
test('malformed or mismatched policy contract fails closed', async () => {
  for (const body of [null, [], {}, { ...document(true), enabled: 'true' }, { ...document(true), mode: 'operator_only' }, { ...document(true), endpoint: '/other' }, { ...document(true), scope: 'publisher' }]) await assert.rejects(fixture({ body }).read());
});
test('every asynchronous boundary rejects an abandoned owner', async () => { for (const staleAt of ['capabilities','proof','fetch','bytes']) await assert.rejects(fixture({ staleAt }).read(), /stale/); });
test('helper has no signer, admission, storage or private-key authority', () => { assert.doesNotMatch(source, /selectedSigner|signEvent|signedRequest|admitAccount|localStorage|sessionStorage|indexedDB|console\./); });
