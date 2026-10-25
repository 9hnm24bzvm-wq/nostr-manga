// Source-extracted finalization and real observer state; fake signatures/events,
// no browser, network, service, database, hosted execution or prior-run rewrite.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { restoreUploadFinalization } from './upload-finalization-source-association.mjs';
import { harness, finish } from './public-upload-observer-query-fixture.mjs';

const source = fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs', import.meta.url), 'utf8');
const prior = restoreUploadFinalization(source);
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
function finalization(text) {
  const marker = '  } finally {\n    result.transport_refusal_captures.push';
  const at = text.indexOf(marker); assert(at > 0 && text.indexOf(marker, at + 1) === -1);
  const end = text.indexOf('    // The outer safe-error serializer', at); assert(end > at);
  const until = text.slice(text.indexOf('  async function until('), text.indexOf('\n  const unique ='));
  return new AsyncFunction('env', `with (env) { ${until}\n${text.slice(at + '  } finally {\n'.length, end)} }`);
}
const complete = finalization(source), oldComplete = finalization(prior);

function fixture({ completed = true, at = 'request-capture', late = 'read', terminal = 'finish' } = {}) {
  const h = harness(), calls = [];
  for (let index = 1; index <= 21; index++) finish(h.page, h.emit({ method: 'POST', pathname: '/api/v1/upload/begin', body: Buffer.from('{}'), index }));
  assert.equal(h.observer.settled(), true); // The successful final journey check.
  let request, now = 0, ticks = 0, saved = 0, readerClosed = false;
  const addLate = () => {
    if (late === 'none') return;
    request = h.emit({ method: late === 'mutation' ? 'POST' : 'GET', pathname: late === 'mutation' ? '/api/v1/upload/begin' : '/api/v1/nostr/account-context', body: late === 'mutation' ? Buffer.from('{}') : null, index: 22,
      ...(late === 'invalid' ? { headers: { cookie: 'synthetic-forbidden' } } : {}) });
    if (terminal === 'already-finished') finish(h.page, request);
  };
  const result = { completed, transport_refusal_captures: [] };
  const buffers = [Buffer.from('synthetic-a'), Buffer.from('synthetic-b'), Buffer.from('synthetic-c')];
  const originalClose = h.observer.close;
  h.observer.close = () => { calls.push('close'); originalClose(); };
  const env = {
    result, observer: h.observer, assert, Boolean,
    Date: { now: () => now }, healthy: h.observer.assertHealthy,
    page: { waitForTimeout: async milliseconds => {
      calls.push('wait'); now += milliseconds; ticks++;
      if (ticks === 1 && request) {
        if (terminal === 'finish') finish(h.page, request);
        if (terminal === 'fail') h.page.emit('requestfailed', request);
        if (terminal === 'bad-status') finish(h.page, request, 100);
        if (terminal === 'duplicate') { finish(h.page, request); h.page.emit('requestfinished', request); }
      }
    } },
    captureUploadRequestRefusals: async () => { calls.push('request-capture'); if (at === 'request-capture') addLate(); return { capture_valid: true, snapshot: {} }; },
    captureUploadPreviewRefusals: async () => { calls.push('preview-capture'); if (at === 'preview-capture') addLate(); return { record_valid: true, truncated: false, codes: [] }; },
    reader: { close() { readerClosed = true; calls.push('reader-close'); }, snapshot: () => ({ closed: readerClosed, pending: 0, rows: [] }) },
    secret: Buffer.from('synthetic secret'), secretHex: 'synthetic secret', images: [{ buffer: buffers[0] }], cancelImage: { buffer: buffers[1] }, replacementImage: { buffer: buffers[2] },
    save: () => { saved++; calls.push('save'); },
  };
  return { h, env, result, calls, buffers, ticks: () => ticks, saved: () => saved };
}

function assertClean(f) {
  assert.equal(f.result.browser_http.closed, true); assert.equal(f.result.independent_http.closed, true);
  assert.equal(f.env.secret, undefined); assert.equal(f.env.secretHex, '');
  assert(f.buffers.every(buffer => buffer.every(byte => byte === 0))); assert.equal(f.saved(), 1);
  for (const event of ['request', 'response', 'requestfinished', 'requestfailed']) assert.equal(f.h.page.listenerCount(event), 0);
}

for (const at of ['request-capture', 'preview-capture']) test('published predecessor closes with an unobserved late terminal during ' + at, async () => {
  const f = fixture({ at }); await oldComplete(f.env); assertClean(f);
  assert.equal(f.ticks(), 0); assert.equal(f.result.browser_http.rows.at(-1).native, 'pending');
  assert.deepEqual(f.result.browser_http.failures, []);
  assert.equal(f.result.browser_http.rows.length, 22); assert.equal(f.h.observer.mutationCount(), 21);
});
for (const at of ['request-capture', 'preview-capture']) test('late read drains to its genuine native terminal before successful close: ' + at, async () => {
  const f = fixture({ at }); await complete(f.env); assertClean(f);
  assert.equal(f.ticks(), 1); assert(f.result.browser_http.rows.every(row => row.native === 'finished' && row.status === 200));
  assert(f.calls.indexOf('close') > f.calls.indexOf('preview-capture'));
  assert(f.calls.indexOf('close') > f.calls.indexOf('wait'));
});
test('settled completion closes synchronously inside the poll before its promise yields', async () => {
  const f = fixture({ late: 'none' });
  const settled = f.h.observer.settled;
  f.h.observer.settled = () => {
    const value = settled();
    queueMicrotask(() => { assert.equal(f.h.observer.snapshot().closed, true); });
    return value;
  };
  await complete(f.env); assertClean(f); assert.equal(f.ticks(), 0);
});
for (const terminal of ['pending', 'bad-status']) test('unsettled ' + terminal + ' expires at the unchanged 15000ms bound and remains non-success', async () => {
  const f = fixture({ terminal });
  await assert.rejects(complete(f.env), error => error.code === 'UPLOAD_UI_READINESS_TIMEOUT'); assertClean(f);
  assert.equal(f.ticks(), 150);
  assert(f.result.browser_http.rows.some(row => row.native !== 'finished' || row.status < 200));
});
for (const terminal of ['fail', 'duplicate']) test('late native ' + terminal + ' remains fatal and is preserved during cleanup', async () => {
  const f = fixture({ terminal }); await assert.rejects(complete(f.env)); assertClean(f);
  assert.deepEqual(f.result.browser_http.failures, [terminal === 'fail' ? 'signed_native_failed' : 'duplicate_native_terminal']);
});
test('late signed-invalid request is fatal, not discarded by successful closure', async () => {
  const f = fixture({ late: 'invalid' }); await assert.rejects(complete(f.env), /signed_request_invalid/); assertClean(f);
});
test('late successful mutation cannot change the exact 21-write acceptance', async () => {
  const f = fixture({ late: 'mutation', terminal: 'already-finished' });
  await assert.rejects(complete(f.env), /Unexpected final UI mutation/); assertClean(f); assert.equal(f.h.observer.mutationCount(), 22);
});
test('failed journey cleanup keeps pending rows without claiming completed success or waiting', async () => {
  const f = fixture({ completed: false }); await complete(f.env); assertClean(f);
  assert.equal(f.ticks(), 0); assert.equal(f.result.completed, false); assert.equal(f.result.browser_http.rows.at(-1).native, 'pending');
});
test('only exact finalization bytes restore the published predecessor', () => {
  for (const changed of [source + '\n', source.replace("observer.close(); return true;", 'return true;'), source.replace('Unexpected final UI mutation', 'ignored')]) assert.throws(() => restoreUploadFinalization(changed));
});
