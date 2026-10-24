import test from 'node:test';
import assert from 'node:assert/strict';
import { Readable, Writable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { Credits, Exchange, exactOrigin, targetFor, endToEnd, meter, ownTerminal, settleTerminals } from '../../scripts/integrated-load/native-proxy.mjs';
import { validateInput, phases, reconcileRequests, latencySummary, observeProgress, matchingProgressCommit, settleActions } from '../../scripts/integrated-load/native-browser.mjs';

const origin = 'http://127.0.0.1:32123', allowed = new Set([origin]);
test('numeric owned origin and exact Host preserve encoded targets; redirects re-enter the same fence', () => {
  const target = origin + '/api/v1/manga?ids%5B%5D=x';
  assert.equal(targetFor(target, ['Host', '127.0.0.1:32123'], allowed).href, target);
  for (const raw of ['http://localhost:32123', 'https://127.0.0.1:32123', origin + '/', 'http://127.0.0.1:543', 'http://127.0.0.1:65536']) assert.throws(() => exactOrigin(raw));
  for (const raw of ['http://example.invalid/a', 'http://user@127.0.0.1:32123/a', origin + '/a#fragment', origin + '/x/../a', origin + '/a b', '/relative']) assert.throws(() => targetFor(raw, ['Host', '127.0.0.1:32123'], allowed));
  assert.throws(() => targetFor(target, ['Host', '127.0.0.1:32124'], allowed));
});
test('ambiguous framing and policy-stripping hop tokens fail closed', () => {
  for (const headers of [
    ['Host', '127.0.0.1:32123', 'host', '127.0.0.1:32123'],
    ['Host', '127.0.0.1:32123', 'Content-Length', '1', 'Content-Length', '2'],
    ['Host', '127.0.0.1:32123', 'Transfer-Encoding', 'chunked'],
    ['Host', '127.0.0.1:32123', 'Content-Length', '+1']
  ]) assert.throws(() => targetFor(origin + '/a', headers, allowed));
  assert.throws(() => endToEnd({ connection: 'authorization', authorization: 'not-exported' }));
  assert.deepEqual(endToEnd({ connection: 'keep-alive', 'content-encoding': 'gzip', 'content-security-policy': "default-src 'self'", etag: 'value', 'transfer-encoding': 'chunked' }), { 'content-encoding': 'gzip', 'content-security-policy': "default-src 'self'", etag: 'value' });
});
test('global attempts, inflight, queue and monotonic deadline cannot reset', () => {
  let now = 0; const credits = new Credits({ attempts: 2, inflight: 1, queue: 1, seconds: 1, now: () => now });
  const a = credits.offer('api', 'GET'); assert.throws(() => credits.offer('api', 'GET'), /queue-limit/);
  credits.dispatch(a); const b = credits.offer('media', 'GET'); assert.throws(() => credits.dispatch(b), /inflight-limit/);
  credits.settle(a, 'complete'); credits.dispatch(b); credits.settle(b, 'complete');
  assert.throws(() => credits.offer('api', 'GET'), /attempt-limit/); assert.throws(() => credits.settle(b, 'complete'), /duplicate-terminal/);
  now = 1000; assert.throws(() => credits.check(), /deadline/);
  assert.equal(credits.snapshot().active, 0); assert.equal(credits.snapshot().waiting, 0);
});
async function stream(chunks, { declared = null, limit = 100, disconnect = false } = {}) {
  const credits = new Credits({ response: limit }), row = credits.offer('media', 'GET'); credits.dispatch(row);
  const result = [];
  await pipeline(Readable.from(chunks.map(value => Buffer.from(value))), meter(credits, row, 'response', declared), new Writable({ write(value, _, callback) { result.push(value); callback(disconnect ? Error('disconnected') : null); } }));
  credits.settle(row, 'complete'); return { credits, result: Buffer.concat(result).toString() };
}
test('split streaming chunks preserve bytes and debit before forwarding', async () => {
  const result = await stream(['ab', 'c', 'def'], { declared: 6, limit: 6 });
  assert.equal(result.result, 'abcdef'); assert.equal(result.credits.snapshot().browser_response_payload_forwarded, 6);
  assert.equal(result.credits.snapshot().upstream_response_payload_received, 6);
});
test('declared overrun, incomplete bodies, total-byte overflow and disconnect reject', async () => {
  await assert.rejects(stream(['ab', 'c'], { declared: 2 }), /length-overrun/);
  await assert.rejects(stream(['ab'], { declared: 3 }), /length-incomplete/);
  await assert.rejects(stream(['ab', 'cd'], { limit: 3 }), /response-limit/);
  await assert.rejects(stream(['ab'], { disconnect: true }), /disconnected/);
});
test('no extra chunk reaches the destination after the budget refuses it', async () => {
  const credits = new Credits({ response: 2 }), row = credits.offer('media', 'GET'); credits.dispatch(row); const observed = [];
  await assert.rejects(pipeline(Readable.from([Buffer.from('ab'), Buffer.from('c')]), meter(credits, row, 'response'), new Writable({ write(bytes, _, callback) { observed.push(bytes.toString()); callback(); } })), /response-limit/);
  assert.deepEqual(observed, ['ab']); assert.equal(credits.snapshot().browser_response_payload_forwarded, 2); assert.equal(credits.snapshot().upstream_response_payload_received, 3);
  credits.settle(row, 'failed'); assert.equal(credits.snapshot().active, 0);
});
test('native adapter imports without loading Playwright, service or browser; scope is bounded', () => {
  assert.equal(phases.reduce((sum, phase) => sum + phase.seconds, 0), 80);
  assert.equal(Math.max(...phases.map(phase => phase.devices)), 4);
  assert.throws(() => validateInput({ origin: 'https://example.invalid', chapter: 'x', manga: 'y', report: '/tmp/not-used' }));
  assert.throws(() => validateInput({ origin, chapter: 'x', manga: 'y', report: '/tmp/not-used', target: 'anything' }));
});
test('native reconciliation requires exact method/URL multiplicities and independently observed cache-only terminals', () => {
  assert.equal(reconcileRequests(new Map([['GET a', 3], ['GET b', 1]]), new Map([['GET a', 2], ['GET b', 1]]), new Map([['GET a', 1]])), true);
  assert.throws(() => reconcileRequests(new Map([['GET a', 3]]), new Map([['GET a', 2]]), new Map()), /reconciliation/);
  assert.throws(() => reconcileRequests(new Map([['GET a', 3]]), new Map([['GET b', 3]]), new Map()), /reconciliation/);
  assert.throws(() => reconcileRequests(new Map([['GET a', 1]]), new Map([['GET a', 2]]), new Map()), /reconciliation/);
});
test('latency empty/missing observations are not zero latency or an SLO', () => {
  assert.deepEqual(latencySummary([]), { observations: 0, p50: null, p95: null, p99: null });
  assert.deepEqual(latencySummary([5, 1, 3, 2, 4]), { observations: 5, p50: 3, p95: 5, p99: 5 });
});
test('fresh save requires exact chapter/manga/page-two put after the intended gesture and transaction completion', () => {
  const previousStore = globalThis.IDBObjectStore, previousBinding = globalThis.__capacityCommit;
  const observed = [], result = { nativeRequest: true };
  class Store { put() { return result; } }
  globalThis.IDBObjectStore = Store; globalThis.__capacityCommit = value => observed.push({ ...value, device: 0 });
  try {
    observeProgress({ database: 'expected-db', chapter: 'expected-chapter', manga: 'expected-manga' });
    const put = (record, database = 'expected-db', terminal = 'complete') => {
      const tx = new EventTarget(); tx.db = { name: database }; tx.mode = 'readwrite';
      const store = new Store(); store.transaction = tx; store.name = 'records';
      assert.equal(store.put(record), result);
      assert.equal(matchingProgressCommit(observed, 0, 1), false);
      tx.dispatchEvent(new Event(terminal));
    };
    const target = { family: 'chapters', id: 'expected-chapter', mangaId: 'expected-manga', page: 2 };
    put(target); // Former matching row before this action has no matching intent.
    globalThis.__capacityBeginProgressIntent(1);
    for (const record of [{ ...target, id: 'unrelated' }, { ...target, mangaId: 'unrelated' }, { ...target, page: 1 }]) put(record);
    put(target, 'other-db'); put(target, 'expected-db', 'abort');
    assert.equal(matchingProgressCommit(observed, 0, 1), false);
    put(target);
    assert.equal(matchingProgressCommit(observed, 0, 1), true);
    assert.equal(matchingProgressCommit(observed, 1, 1), false);
    assert.equal(matchingProgressCommit(observed, 0, 2), false);
    assert.ok(!JSON.stringify(observed).includes('expected-chapter'));
  } finally {
    globalThis.IDBObjectStore = previousStore; globalThis.__capacityCommit = previousBinding;
    delete globalThis.__capacityBeginProgressIntent;
  }
});
test('global action ownership awaits rejection and delayed cancellation, or fails boundedly', async () => {
  let rejectAction, cancelled = false;
  const action = new Promise((_, reject) => { rejectAction = reject; });
  const receipt = await settleActions([Promise.resolve(), action], async () => {
    await new Promise(resolve => setTimeout(resolve, 5)); cancelled = true; rejectAction(Error('context closed'));
  });
  assert.equal(cancelled, true);
  assert.deepEqual(receipt, { all_settled: true, actions: 2, fulfilled: 1, rejected: 1 });
  let end;
  const pending = new Promise(resolve => { end = resolve; });
  await assert.rejects(settleActions([pending], async () => {}, 5), /settlement-timeout/);
  end(); await pending;
});
test('cleanup awaits delayed upload pipeline terminal callback and refuses missing terminal', async () => {
  const terminals = new Set(); let uploadEnd, callbacks = 0;
  ownTerminal(terminals, done => { uploadEnd = () => { callbacks++; done(); }; });
  let cleaned = false;
  const cleanup = settleTerminals(terminals).then(() => { cleaned = true; });
  await new Promise(resolve => setTimeout(resolve, 5));
  assert.equal(cleaned, false); assert.equal(callbacks, 0);
  uploadEnd(); await cleanup;
  assert.equal(callbacks, 1); assert.equal(terminals.size, 0);
  await new Promise(resolve => setTimeout(resolve, 5)); assert.equal(callbacks, 1);
  let release;
  ownTerminal(terminals, done => { release = done; });
  await assert.rejects(settleTerminals(terminals, [], 5), /close-timeout/);
  release(); await settleTerminals(terminals);
});
test('early response cannot pass or release its slot before successful upload terminal', () => {
  const credits = new Credits({ inflight: 1 }), row = credits.offer('api', 'POST'); credits.dispatch(row);
  const exchange = new Exchange(credits, row), next = credits.offer('api', 'GET');
  assert.equal(exchange.finish('response', 'complete'), false);
  assert.equal(row.state, 'active'); assert.equal(credits.active, 1);
  assert.equal(row.upload_state, 'pending'); assert.throws(() => credits.dispatch(next), /inflight-limit/);
  assert.equal(exchange.finish('upload', 'complete'), true);
  assert.equal(row.state, 'complete'); assert.equal(credits.active, 0);
  credits.dispatch(next); credits.settle(next, 'complete');
});
test('early response plus failed or cancelled upload never becomes complete, even during close', () => {
  for (const closing of [false, true]) {
    const credits = new Credits({ inflight: 1 }), row = credits.offer('api', 'POST'); credits.dispatch(row);
    const exchange = new Exchange(credits, row);
    exchange.finish('response', 'complete');
    if (closing) exchange.stop('cancelled');
    assert.equal(row.state, 'active'); assert.equal(credits.active, 1);
    exchange.finish('upload', 'failed');
    assert.equal(row.upload_state, 'failed'); assert.equal(row.response_state, 'complete');
    assert.equal(row.state, closing ? 'cancelled' : 'failed'); assert.equal(credits.active, 0);
    assert.equal(credits.failure, 'upload-stream-failed');
    assert.throws(() => credits.offer('api', 'GET'), /upload-stream-failed/);
    assert.throws(() => exchange.finish('upload', 'complete'), /duplicate-stream-terminal/);
  }
});
