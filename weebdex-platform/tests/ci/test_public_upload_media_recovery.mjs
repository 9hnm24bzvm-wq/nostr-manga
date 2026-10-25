#!/usr/bin/env node
// Finite, dependency-free helper seams. Actual original image bytes and Blobs;
// loader/auth/document lifetimes and object-URL methods are explicit finite
// fakes. No server, browser, signer, image decoder, network, E2E or acceptance
// evidence is claimed. Embedded bytes never stand in for a remote response proof.
import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';

async function helper(name) {
  const source = await readFile(new URL('../../platform-integrations/public-api/' + name, import.meta.url), 'utf8');
  // These two audited helper modules have no imports and no module-init effects
  // beyond local data. Only their actual source executes, with Node's TS stripper.
  assert.doesNotMatch(source, /^\s*import\b/m);
  return import('data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(source, { mode: 'strip' })).toString('base64'));
}
const { serializeUploadMultipart, serializeUploadReplacementMultipart, MAX_UPLOAD_IMAGE_BYTES } = await helper('upload-multipart.ts');
const { createUploadPreviewLease, uploadPreviewKey, revokeUploadPreviewUrl } = await helper('upload-preview.ts');
const png = Uint8Array.from(Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aH3kAAAAASUVORK5CYII=', 'base64'));
const boundary = 'finite-upload-boundary_123456';
const image = (name = 'original-page.png', bytes = png, type = 'image/png') => new File([bytes], name, { type });
const encode = value => new TextEncoder().encode(value);
const hash = [...new Uint8Array(await crypto.subtle.digest('SHA-256', png))].map(byte => byte.toString(16).padStart(2, '0')).join('');
const metadata = (overrides = {}) => ({ name: 'page.png', type: 'image/png', size: png.byteLength, arrayBuffer: async () => png.slice().buffer, ...overrides });

test('one original PNG becomes exact multipart bytes with original filename, MIME, size and explicit boundary', async () => {
  const file = image('漫画 01.png');
  const result = await serializeUploadMultipart(file, { boundary });
  const head = encode(`--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="漫画 01.png"\r\nContent-Type: image/png\r\n\r\n`);
  const tail = encode(`\r\n--${boundary}--\r\n`);
  assert(result.body instanceof Uint8Array);
  assert.deepEqual(result.body, new Uint8Array(Buffer.concat([head, png, tail])));
  assert.deepEqual(result.body.slice(head.length, head.length + file.size), png);
  assert.equal(result.contentType, `multipart/form-data; boundary=${boundary}`);
  assert.equal(result.boundary, boundary);
  assert.deepEqual(result.file, { name: '漫画 01.png', type: 'image/png', size: png.length });
  assert(Object.isFrozen(result));
  assert(Object.isFrozen(result.file));
});

test('replacement has exactly one canonical plain expectedVersion followed by one exact original file', async () => {
  const file = image('漫画 replacement.png'), result = await serializeUploadReplacementMultipart(file, 2147483647, { boundary });
  const head = encode(`--${boundary}\r\nContent-Disposition: form-data; name="expectedVersion"\r\n\r\n2147483647\r\n--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="漫画 replacement.png"\r\nContent-Type: image/png\r\n\r\n`);
  assert.deepEqual(result.body, new Uint8Array(Buffer.concat([head, png, encode(`\r\n--${boundary}--\r\n`)])));
  assert.deepEqual(result.fileBytes, png);
  assert.equal(result.fileBytes.buffer, result.body.buffer);
  const versionHeaders = new TextDecoder().decode(result.body).split(`\r\n--${boundary}\r\n`)[0];
  assert.doesNotMatch(versionHeaders, /filename=|Content-Type:/);
  assert.equal(new TextDecoder().decode(result.body).match(/name="expectedVersion"/g).length, 1);
  assert.equal(new TextDecoder().decode(result.body).match(/name="file"/g).length, 1);
  assert.equal(result.contentType, `multipart/form-data; boundary=${boundary}`);
});

test('replacement rejects noncanonical and out-of-range versions before reading image bytes', async () => {
  for (const version of [undefined, null, 0, -1, 1.5, NaN, Infinity, '1', '01', '1e2', 2147483648]) {
    let reads = 0;
    await assert.rejects(serializeUploadReplacementMultipart(metadata({ arrayBuffer: async () => { reads++; return png.buffer; } }), version, { boundary }), /version/);
    assert.equal(reads, 0);
  }
});

test('serializer snapshots original metadata and copies original bytes without rebuilding FormData', async () => {
  const original = png.slice();
  const file = metadata({ arrayBuffer: async () => { file.name = 'changed.png'; file.type = 'image/gif'; file.size = 1; return original.buffer; } });
  const result = await serializeUploadMultipart(file, { boundary });
  const before = result.body.slice();
  original.fill(0);
  assert.deepEqual(result.body, before);
  assert.deepEqual(result.file, { name: 'page.png', type: 'image/png', size: png.length });
  assert(new TextDecoder().decode(result.body).includes('filename="page.png"'));
});

test('canonical image MIME format guards preserve each supplied byte sequence', async () => {
  // These finite JPEG/GIF/WebP header probes exercise the format guard only;
  // they are deliberately not decoder fixtures or remote-upload success proof.
  const probes = [
    ['image/jpeg', [255, 216, 255, 224, 0, 2, 255, 217]],
    ['image/gif', [71, 73, 70, 56, 55, 97, 0, 0]],
    ['image/gif', [71, 73, 70, 56, 57, 97, 0, 0]],
    ['image/webp', [82, 73, 70, 70, 4, 0, 0, 0, 87, 69, 66, 80]],
  ];
  for (const [type, value] of probes) {
    const bytes = Uint8Array.from(value), result = await serializeUploadMultipart(image('probe.image', bytes, type), { boundary });
    const text = new TextDecoder().decode(result.body), offset = encode(text.slice(0, text.indexOf('\r\n\r\n') + 4)).length;
    assert.deepEqual(result.body.slice(offset, offset + bytes.length), bytes);
    assert.equal(result.file.type, type);
  }
});

test('filename bounds count original UTF-8 bytes and reject header injection before reading the file', async () => {
  for (const name of ['', ' ', 'bad\rname.png', 'bad\nContent-Type: text/plain', 'bad".png', 'bad\\.png', 'bad/name.png', 'bad\0.png', 'bad\x7f.png', 'a'.repeat(256), '界'.repeat(86)]) {
    let reads = 0;
    await assert.rejects(serializeUploadMultipart(metadata({ name, arrayBuffer: async () => { reads++; return png.buffer; } }), { boundary }), /filename/);
    assert.equal(reads, 0);
  }
  for (const name of ['a'.repeat(255), '界'.repeat(85)]) {
    const result = await serializeUploadMultipart(image(name), { boundary });
    assert.equal(result.file.name, name);
  }
});

test('MIME and nonempty 25 MiB bounds reject invalid metadata before reading', async () => {
  for (const changes of [{ type: '' }, { type: 'image/jpg' }, { type: 'IMAGE/PNG' }, { type: 'image/png\r\nX: injected' }, { type: 'application/octet-stream' }, { size: 0 }, { size: -1 }, { size: 1.5 }, { size: NaN }, { size: Infinity }, { size: MAX_UPLOAD_IMAGE_BYTES + 1 }]) {
    let reads = 0;
    await assert.rejects(serializeUploadMultipart(metadata({ ...changes, arrayBuffer: async () => { reads++; return png.buffer; } }), { boundary }), /MIME|1 to 25 MiB/);
    assert.equal(reads, 0);
  }
  const bytes = new Uint8Array(MAX_UPLOAD_IMAGE_BYTES); bytes.set(png);
  const result = await serializeUploadMultipart(metadata({ size: bytes.length, arrayBuffer: async () => bytes.buffer }), { boundary });
  assert.equal(result.file.size, MAX_UPLOAD_IMAGE_BYTES);
  assert(result.body.length > MAX_UPLOAD_IMAGE_BYTES);
});

test('actual byte count, ArrayBuffer and MIME signature must agree with original metadata', async () => {
  await assert.rejects(serializeUploadMultipart(null, { boundary }), /original image/);
  await assert.rejects(serializeUploadMultipart(metadata({ size: png.length + 1 }), { boundary }), /byte length/);
  await assert.rejects(serializeUploadMultipart(metadata({ arrayBuffer: async () => png }), { boundary }), /byte length/);
  await assert.rejects(serializeUploadMultipart(image('wrong.gif', png, 'image/gif'), { boundary }), /signature/);
  await assert.rejects(serializeUploadMultipart(image('broken.png', encode('not an image')), { boundary }), /signature/);
  const failure = Error('Original file read failed');
  await assert.rejects(serializeUploadMultipart(metadata({ arrayBuffer: async () => { throw failure; } }), { boundary }), error => error === failure);
});

test('multipart boundary is bounded/canonical and cannot collide with original image bytes', async () => {
  for (const value of ['', 'a'.repeat(15), 'a'.repeat(71), 'boundary has spaces', 'bad\r\nheader-injection', 'bad"boundary-123456', 'bad;boundary-123456', null])
    await assert.rejects(serializeUploadMultipart(image(), { boundary: value }), /boundary/);
  for (const value of ['a'.repeat(16), 'Z_-9'.repeat(17) + 'a_']) assert.equal((await serializeUploadMultipart(image(), { boundary: value })).boundary, value);
  const collision = new Uint8Array(Buffer.concat([png, encode('--' + boundary)]));
  await assert.rejects(serializeUploadMultipart(image('collision.png', collision), { boundary }), /collides/);
  const first = await serializeUploadMultipart(image()), second = await serializeUploadMultipart(image());
  assert.match(first.boundary, /^weebdex-upload-[0-9a-f]{48}$/);
  assert.notEqual(first.boundary, second.boundary);
});

function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
let sequence = 0;
const context = (overrides = {}) => ({ sessionId: 'finite-session', fileId: 'finite-file', fileHash: hash, fileSize: png.length, mimeType: 'image/png', owner: {}, ...overrides });
const blobFor = value => new Blob([new Uint8Array(value.fileSize)], { type: value.mimeType });
function fixture({ onCreate, onChange, isCurrent, isBlob } = {}) {
  const id = ++sequence, calls = { loads: [], created: [], revoked: [], states: [] };
  let alive = true;
  const lease = createUploadPreviewLease({
    load(value) { const gate = deferred(); calls.loads.push({ context: value, gate }); return gate.promise; },
    isCurrent: value => alive && (isCurrent?.(value) ?? true),
    onChange(value) { calls.states.push(value); onChange?.(value); },
    url: {
      createObjectURL(blob) { const url = `blob:finite-${id}-${calls.created.length + 1}`; calls.created.push({ url, blob }); onCreate?.(); return url; },
      revokeObjectURL(url) { calls.revoked.push(url); },
    },
    ...(isBlob ? { isBlob } : {}),
  });
  return { lease, calls, expire() { alive = false; } };
}
const start = async (value, file, local) => { const operation = value.lease.update(file, local); await Promise.resolve(); return { operation }; };

test('current local thumbnail remains until a validated signed Blob replaces it; transferred cleanup happens once', async () => {
  const value = fixture(), file = context(), local = { url: 'blob:finite-local-success', ownership: 'transferred' };
  try {
    const { operation } = await start(value, file, local);
    assert.equal(value.lease.getState().status, 'loading');
    assert.equal(value.lease.getState().url, local.url);
    assert.equal(value.lease.owns(local.url), true);
    assert.deepEqual(value.calls.revoked, []);
    assert.equal(value.calls.loads.length, 1);
    const original = new Blob([png], { type: 'image/png' });
    value.calls.loads[0].gate.resolve(original); await operation;
    const ready = value.lease.getState();
    assert.equal(ready.status, 'ready');
    assert.equal(ready.key, uploadPreviewKey(file));
    assert.equal(value.calls.created[0].blob, original);
    assert.equal(ready.url, value.calls.created[0].url);
    assert.equal(value.lease.owns(ready.url), true);
    assert.equal(value.lease.owns(local.url), false);
    assert.deepEqual(value.calls.revoked, [local.url]);
    value.lease.dispose(); value.lease.dispose();
    assert.equal(value.lease.owns(ready.url), false);
    assert.deepEqual(value.calls.revoked, [local.url, ready.url]);
  } finally { value.lease.dispose(); }
});

test('borrowed local thumbnail is never revoked by its borrowing lease', async () => {
  const value = fixture(), file = context(), local = { url: 'blob:finite-borrowed', ownership: 'borrowed' };
  try {
    const { operation } = await start(value, file, local);
    assert.equal(value.lease.owns(local.url), false);
    value.calls.loads[0].gate.resolve(blobFor(file)); await operation;
    value.lease.dispose();
    assert.deepEqual(value.calls.revoked, [value.calls.created[0].url]);
  } finally { value.lease.dispose(); }
});

test('same exact key and owner are idempotent while loading and after readiness', async () => {
  const value = fixture(), file = context();
  try {
    const first = value.lease.update(file), second = value.lease.update({ ...file });
    assert.equal(first, second);
    await Promise.resolve(); assert.equal(value.calls.loads.length, 1);
    value.calls.loads[0].gate.resolve(blobFor(file)); await first;
    assert.equal(value.lease.update({ ...file }), first);
    assert.equal(value.calls.loads.length, 1);
    assert.equal(value.calls.created.length, 1);
  } finally { value.lease.dispose(); }
});

test('a preview read failure is visible, preserves a current local thumbnail, resolves without a fake upload failure and never retries', async () => {
  const value = fixture(), file = context(), failure = Error('Signed preview GET denied');
  const local = { url: 'blob:finite-local-failure', ownership: 'transferred' };
  try {
    const { operation } = await start(value, file, local);
    value.calls.loads[0].gate.reject(failure);
    await assert.doesNotReject(operation);
    assert.equal(value.lease.getState().status, 'error');
    assert.equal(value.lease.getState().error, failure);
    assert.equal(value.lease.getState().url, local.url);
    assert.deepEqual(value.calls.revoked, []);
    await value.lease.update({ ...file }); await Promise.resolve();
    assert.equal(value.calls.loads.length, 1);
    assert.equal(value.calls.created.length, 0);
    value.lease.dispose(); assert.deepEqual(value.calls.revoked, [local.url]);
  } finally { value.lease.dispose(); }
});

for (const changes of [{ sessionId: 'new-session' }, { fileId: 'new-file' }, { fileHash: 'b'.repeat(64) }, { fileSize: png.length + 1 }, { mimeType: 'image/webp' }]) {
  test('stable-key change ' + Object.keys(changes)[0] + ' discards a late fulfilled preview without assigning it', async () => {
    const value = fixture(), first = context(), next = { ...first, ...changes };
    try {
      const a = await start(value, first);
      const b = await start(value, next);
      assert.equal(value.calls.loads.length, 2);
      value.calls.loads[0].gate.resolve(blobFor(first)); await a.operation;
      assert.equal(value.calls.created.length, 0);
      assert.equal(value.lease.getState().key, uploadPreviewKey(next));
      value.calls.loads[1].gate.resolve(blobFor(next)); await b.operation;
      assert.equal(value.calls.created.length, 1);
      assert.equal(value.lease.getState().status, 'ready');
      assert.equal(value.lease.getState().key, uploadPreviewKey(next));
    } finally { value.lease.dispose(); }
  });
}

test('same file with a different owner invalidates the original load and ignores its late rejection', async () => {
  const value = fixture(), first = context(), next = { ...first, owner: {} };
  try {
    const a = await start(value, first), b = await start(value, next);
    value.calls.loads[1].gate.resolve(blobFor(next)); await b.operation;
    const state = value.lease.getState(), changes = value.calls.states.length;
    value.calls.loads[0].gate.reject(Error('Expired owner response')); await a.operation;
    assert.equal(value.lease.getState(), state);
    assert.equal(value.calls.states.length, changes);
    assert.equal(value.calls.created.length, 1);
  } finally { value.lease.dispose(); }
});

test('owner/document invalidation before signed GET invocation prevents the read and releases owned thumbnails', async () => {
  const value = fixture(), file = context(), local = { url: 'blob:finite-before-read', ownership: 'transferred' };
  try {
    const pending = value.lease.update(file, local);
    value.expire(); await pending;
    assert.equal(value.calls.loads.length, 0);
    assert.equal(value.calls.created.length, 0);
    assert.deepEqual(value.calls.revoked, [local.url]);
    assert.equal(value.lease.getState().status, 'idle');
    assert.equal(value.calls.states.length, 1);
  } finally { value.lease.dispose(); }
});

test('owner/document invalidation during GET discards the Blob and emits no stale readiness or failure', async () => {
  const value = fixture(), file = context(), local = { url: 'blob:finite-owner-late', ownership: 'transferred' };
  try {
    const { operation } = await start(value, file, local), changes = value.calls.states.length;
    value.expire(); value.calls.loads[0].gate.resolve(blobFor(file)); await operation;
    assert.equal(value.calls.created.length, 0);
    assert.equal(value.calls.states.length, changes);
    assert.deepEqual(value.calls.revoked, [local.url]);
    assert.equal(value.lease.getState().url, null);
  } finally { value.lease.dispose(); }
});

test('a URL created at the exact owner-loss boundary is revoked and never assigned', async () => {
  let value;
  value = fixture({ onCreate: () => value.expire() });
  const file = context();
  try {
    const { operation } = await start(value, file);
    value.calls.loads[0].gate.resolve(blobFor(file)); await operation;
    assert.deepEqual(value.calls.revoked, [value.calls.created[0].url]);
    assert.equal(value.lease.getState().url, null);
    assert(!value.calls.states.some(row => row.status === 'ready'));
  } finally { value.lease.dispose(); }
});

for (const outcome of ['resolve', 'reject']) test('unmount disposes local resources and ignores late preview ' + outcome, async () => {
  const value = fixture(), file = context(), local = { url: 'blob:finite-unmount-' + outcome, ownership: 'transferred' };
  const { operation } = await start(value, file, local), changes = value.calls.states.length;
  value.lease.dispose();
  value.calls.loads[0].gate[outcome](outcome === 'resolve' ? blobFor(file) : Error('Late network failure'));
  await operation;
  await value.lease.update(file, local);
  assert.equal(value.calls.loads.length, 1);
  assert.equal(value.calls.created.length, 0);
  assert.equal(value.calls.states.length, changes);
  assert.deepEqual(value.calls.revoked, [local.url]);
});

test('legitimate context replacement releases the previous signed URL and clear releases the replacement exactly once', async () => {
  const value = fixture(), first = context(), second = { ...first, fileId: 'replacement' };
  try {
    const a = await start(value, first); value.calls.loads[0].gate.resolve(blobFor(first)); await a.operation;
    const original = value.lease.getState().url;
    const b = await start(value, second);
    assert.deepEqual(value.calls.revoked, [original]);
    assert.equal(value.lease.getState().url, null);
    value.calls.loads[1].gate.resolve(blobFor(second)); await b.operation;
    const replacement = value.lease.getState().url;
    await value.lease.update(null); await value.lease.update(null); value.lease.dispose();
    assert.deepEqual(value.calls.revoked, [original, replacement]);
    assert.equal(value.lease.getState().status, 'idle');
  } finally { value.lease.dispose(); }
});

test('a second live lease may borrow a URL but cannot steal or revoke its original ownership', async () => {
  const a = fixture(), b = fixture(), file = context();
  try {
    const first = await start(a, file); a.calls.loads[0].gate.resolve(blobFor(file)); await first.operation;
    const url = a.lease.getState().url;
    assert.throws(() => b.lease.update(file, { url, ownership: 'transferred' }), /another live/);
    const second = await start(b, file, { url, ownership: 'borrowed' });
    b.calls.loads[0].gate.resolve(blobFor(file)); await second.operation; b.lease.dispose();
    assert.deepEqual(a.calls.revoked, []);
    assert(!b.calls.revoked.includes(url));
    a.lease.dispose(); assert.deepEqual(a.calls.revoked, [url]);
  } finally { a.lease.dispose(); b.lease.dispose(); }
});

test('non-Blob, wrong-size and wrong-MIME signed preview results remain truthful errors with no object URL', async () => {
  for (const response of [{ size: png.length, type: 'image/png' }, new Blob([png.slice(1)], { type: 'image/png' }), new Blob([png], { type: 'image/gif' })]) {
    const value = fixture(), file = context();
    try {
      const { operation } = await start(value, file);
      value.calls.loads[0].gate.resolve(response); await operation;
      assert.equal(value.lease.getState().status, 'error');
      assert.match(value.lease.getState().error.message, /expected original image Blob/);
      assert.equal(value.calls.created.length, 0);
      await value.lease.update(file); assert.equal(value.calls.loads.length, 1);
    } finally { value.lease.dispose(); }
  }
});

test('invalid preview context or implicit local URL ownership never starts a request', () => {
  const value = fixture(), file = context();
  try {
    for (const changes of [{ sessionId: '' }, { sessionId: 's'.repeat(129) }, { fileId: '' }, { fileHash: 'invalid' }, { fileHash: [hash] }, { fileSize: 0 }, { fileSize: 25 * 1024 * 1024 + 1 }, { fileSize: 1.5 }, { mimeType: 'image/jpg' }, { owner: null }])
      assert.throws(() => value.lease.update({ ...file, ...changes }), /exact acknowledged upload/);
    assert.throws(() => value.lease.update(file, { url: 'https://invalid.example/page.png', ownership: 'borrowed' }), /ownership/);
    assert.throws(() => value.lease.update(file, { url: 'blob:finite-implicit' }), /ownership/);
    assert.equal(value.calls.loads.length, 0);
  } finally { value.lease.dispose(); }
});

test('cleanup dispatcher releases a lease-owned URL once and declines native, borrowed or already-released URLs', async () => {
  const value = fixture(), file = context(), local = { url: 'blob:finite-dispatch-local', ownership: 'transferred' };
  try {
    const { operation } = await start(value, file, local);
    assert.equal(revokeUploadPreviewUrl('blob:finite-native-owned'), false);
    assert.equal(revokeUploadPreviewUrl(local.url), true);
    assert.equal(value.lease.owns(local.url), false);
    assert.equal(revokeUploadPreviewUrl(local.url), false);
    assert.deepEqual(value.calls.revoked, [local.url]);
    value.lease.dispose();
    value.calls.loads[0].gate.resolve(blobFor(file)); await operation;
    assert.equal(value.calls.created.length, 0);
    assert.deepEqual(value.calls.revoked, [local.url]);
  } finally { value.lease.dispose(); }
  const borrowed = fixture(), borrowedLocal = { url: 'blob:finite-dispatch-borrowed', ownership: 'borrowed' };
  try {
    const { operation } = await start(borrowed, context(), borrowedLocal);
    assert.equal(revokeUploadPreviewUrl(borrowedLocal.url), false);
    borrowed.lease.dispose(); borrowed.calls.loads[0].gate.reject(Error('Unmounted')); await operation;
    assert.deepEqual(borrowed.calls.revoked, []);
  } finally { borrowed.lease.dispose(); }
});

test('cleanup dispatcher targets the owning live lease and cannot release another lease by a stale URL', async () => {
  const a = fixture(), b = fixture(), first = context(), replacement = { ...first, fileId: 'dispatch-replacement' };
  try {
    const firstA = await start(a, first), firstB = await start(b, first);
    a.calls.loads[0].gate.resolve(blobFor(first)); b.calls.loads[0].gate.resolve(blobFor(first));
    await firstA.operation; await firstB.operation;
    const stale = a.lease.getState().url, other = b.lease.getState().url;
    const nextA = await start(a, replacement);
    a.calls.loads[1].gate.resolve(blobFor(replacement)); await nextA.operation;
    const current = a.lease.getState().url;
    assert.equal(revokeUploadPreviewUrl(stale), false);
    assert.equal(a.lease.owns(current), true);
    assert.equal(b.lease.owns(other), true);
    assert.equal(revokeUploadPreviewUrl(current), true);
    assert.equal(a.lease.owns(current), false);
    assert.equal(b.lease.owns(other), true);
    assert.deepEqual(b.calls.revoked, []);
    assert.equal(revokeUploadPreviewUrl(current), false);
    a.lease.dispose();
    assert.deepEqual(a.calls.revoked, [stale, current]);
    assert.equal(revokeUploadPreviewUrl(other), true);
    assert.equal(revokeUploadPreviewUrl(other), false);
    b.lease.dispose(); assert.deepEqual(b.calls.revoked, [other]);
  } finally { a.lease.dispose(); b.lease.dispose(); }
});

test('confirmed replacement with same stable file ID and new hash releases the old preview lease and rejects its late Blob', async () => {
  const value = fixture(), original = context(), replacement = { ...original, fileHash: 'd'.repeat(64) };
  try {
    const old = await start(value, original, { url: 'blob:finite-old-page', ownership: 'transferred' });
    const next = await start(value, replacement);
    assert.equal(original.fileId, replacement.fileId); assert.notEqual(uploadPreviewKey(original), uploadPreviewKey(replacement));
    assert.deepEqual(value.calls.revoked, ['blob:finite-old-page']);
    value.calls.loads[0].gate.resolve(blobFor(original)); await old.operation;
    assert.equal(value.calls.created.length, 0); assert.equal(value.lease.getState().key, uploadPreviewKey(replacement));
    value.calls.loads[1].gate.resolve(blobFor(replacement)); await next.operation;
    assert.equal(value.calls.created.length, 1); assert.equal(value.lease.getState().status, 'ready');
  } finally { value.lease.dispose(); }
});
