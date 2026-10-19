// Actual passive observer with explicit fake requests/verifier. No browser,
// signature verification, HTTP, service, database or application state.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { observePublicUpload, uploadDigest } from '../../scripts/public-upload-recovery-observer.mjs';

const origin = 'http://127.0.0.1:50123', owner = 'a'.repeat(64), service = 'b'.repeat(64);
const id = '11111111-1111-8111-8111-111111111111';
function observation({ method = 'POST', path = '/api/v1/upload/' + id,
  status = null, error = 'net::ERR_ABORTED', native = 'failed', count = 1 } = {}) {
  const page = new EventEmitter(), observer = observePublicUpload(page, { origin, owner, service, verify: () => true });
  try {
    for (let index = 0; index < count; index++) {
      const body = method === 'GET' ? null : Buffer.from('{}');
      const event = { kind: 27235, created_at: Math.floor(Date.now() / 1000), content: '', pubkey: owner,
        tags: [['u', origin + path], ['method', method], ['service', service], ['nonce', 'explicit_fake_native_' + index],
          ...(method === 'GET' ? [] : [['payload', uploadDigest(body)]])], sig: 'c'.repeat(128) };
      event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content]));
      const request = { url: () => origin + path, method: () => method,
        headers: () => ({ authorization: 'Nostr ' + Buffer.from(JSON.stringify(event)).toString('base64') }),
        postDataBuffer: () => body, failure: () => ({ errorText: error }) };
      page.emit('request', request);
      if (status !== null) page.emit('response', { request: () => request, status: () => status });
      if (native === 'failed') page.emit('requestfailed', request);
      if (native === 'finished') page.emit('requestfinished', request);
    }
    assert.equal(observer.rows.length, count);
  } finally { observer.close(); }
  return observer.snapshot();
}

const cases = {
  aborted_before_headers: observation(),
  aborted_after_200_headers: observation({ status: 200 }),
  other_failure_after_503_headers: observation({ method: 'GET', path: `/api/v1/nostr/uploads/${id}/files/${id}/preview`, status: 503, error: 'reserved-example.invalid/private-error' }),
  finished_write_503: observation({ status: 503, native: 'finished' }),
  finished_discovery_404: observation({ method: 'GET', path: '/api/v1/upload', status: 404, native: 'finished' }),
  pending_write: observation({ native: 'pending' }),
  seventeen_failures: observation({ count: 17 }),
};
process.stdout.write(JSON.stringify(cases));
