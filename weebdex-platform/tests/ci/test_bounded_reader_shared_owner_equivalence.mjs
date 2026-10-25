// Authored, not run. Both variants execute their real source and validators.
// Run with Node --experimental-vm-modules --test after resource admission.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
const fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
const files = new Map();
function source(relative) { const local = path.join(root, relative); return fs.readFileSync(fs.existsSync(local) ? local : path.join(fallback, relative), 'utf8'); }
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const turns = async () => { for (let i = 0; i < 80; i++) await Promise.resolve(); };
const body = { result: 'ok', data: { kind: 'manga', id: '11111111-1111-8111-8111-111111111111', threadId: 'thread' }, nostr: { source: 'persisted-native-discussion-target' } };
async function fixture(old, options = {}) {
  const timers = new Map(), allocations = [], calls = [], observed = { read: 0, cancel: 0, release: 0 };
  const context = vm.createContext({ URL, URLSearchParams, TextEncoder, TextDecoder, AbortController, setTimeout, clearTimeout,
    Uint8Array: new Proxy(Uint8Array, { construct(target, args) { allocations.push(args[0]); return Reflect.construct(target, args); } }) });
  const modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    const text = old && relative.endsWith('/native-forum/reader.ts') ? source('tests/fixtures/bounded-reader/native-forum-reader-before.ts.txt') : source(relative);
    const mod = new vm.SourceTextModule(stripTypeScriptTypes(text, { mode: 'transform' }), { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + '.ts'))))); links.set(relative, linked); await linked; return mod;
  }
  const mod = await load('platform-integrations/public-api/native-forum/reader.ts'); await mod.evaluate();
  let index = 0; const parts = options.parts ?? [new TextEncoder().encode(JSON.stringify(body))];
  const reader = { read() { observed.read++; return options.read?.promise ?? Promise.resolve(index < parts.length ? { done: false, value: parts[index++] } : { done: true }); },
    cancel() { observed.cancel++; return options.cancel?.promise ?? Promise.resolve(); }, releaseLock() { observed.release++; } };
  const read = mod.namespace.createNativeDiscussionTargetReader({ origin: () => 'https://reader.test', setTimer(fn, ms) { const token = {}; timers.set(token, { fn, ms }); return token; }, clearTimer(token) { timers.delete(token); },
    async fetch(url, init) { calls.push({ url, init }); await options.fetch?.promise; return { status: options.status ?? 200, headers: new Headers({ 'content-type': options.mime ?? 'application/json', ...(options.declared ? { 'content-length': options.declared } : {}) }), body: options.missing ? null : { getReader: () => reader } }; } });
  return { read, timers, calls, observed, allocations, ErrorClass: modules.get('platform-integrations/public-api/validation.ts').namespace.PublicApiContractError, expire(ms) { for (const timer of [...timers.values()]) if (timer.ms === ms) timer.fn(); } };
}
for (const old of [true, false]) {
  const label = old ? 'before extraction' : 'shared owner';
  test(label + ': exact forum defaults, healthy EOF, no body fallback or healthy abort', async () => {
    const f = await fixture(old), abort = new AbortController(); assert.equal((await f.read('thread', abort.signal)).threadId, 'thread'); abort.abort();
    assert.equal(f.calls[0].url, 'https://reader.test/api/v1/nostr/native-forum/threads/thread/context'); assert.equal(f.calls[0].init.credentials, 'omit'); assert.equal(f.calls[0].init.signal.aborted, false);
    assert.deepEqual(f.allocations, [16384]); assert.equal(f.observed.cancel, 0); assert.equal(f.observed.release, 1); assert.equal(f.timers.size, 0);
  });
  for (const stage of ['fetch', 'read', 'cancel']) test(label + ': late ' + stage + ' retains one active slot and observes rejected cleanup', async () => {
    const gate = deferred(), options = { [stage]: gate, ...(stage === 'cancel' ? { status: 404 } : {}) }, f = await fixture(old, options), abort = new AbortController();
    const result = f.read('thread', abort.signal), rejected = assert.rejects(result, error => error.unsettled === true && error.status === 499);
    await turns(); abort.abort(); await turns(); f.expire(250); await rejected;
    await assert.rejects(f.read('other'), error => error.unsettled === true); assert.equal(f.calls.length, 1);
    if (stage === 'read') gate.reject(Error('Late read rejection')); else if (stage === 'cancel') gate.reject(Error('Late cancel rejection')); else gate.resolve();
    await turns(); assert.equal(f.observed.release, 1);
  });
  test(label + ': deadline error remains 504 and 404 body remains owned without abort', async () => {
    const f = await fixture(old, { fetch: deferred() }), result = f.read('thread'), rejected = assert.rejects(result, error => error.status === 504);
    await turns(); f.expire(10000); await turns(); f.expire(250); await rejected;
    const cancel = deferred(), g = await fixture(old, { status: 404, cancel }), pending = g.read('thread'); await turns(); assert.equal(g.observed.cancel, 1);
    await assert.rejects(g.read('other'), error => error.unsettled === true); cancel.resolve(); assert.equal(await pending, null); assert.equal(g.calls[0].init.signal.aborted, false);
  });
  for (const empty of [true, false]) test(label + ': independent raw byte, fragment and empty-fragment limits', async () => {
    const count = empty ? 129 : 4097, f = await fixture(old, { parts: Array.from({ length: count }, () => new Uint8Array(empty ? 0 : 1)) });
    await assert.rejects(f.read('thread'), /fragment budget/); assert.equal(f.observed.read, empty ? 129 : 4096); assert.equal(f.observed.cancel, 1); assert.deepEqual(f.allocations, [16384]);
    const g = await fixture(old, { parts: [new Uint8Array(16385)] }); await assert.rejects(g.read('thread'), /budget/); assert.equal(g.observed.cancel, 1);
  });
}

for (const old of [true, false]) for (const refusal of ['media', 'declared', 'missing', 'fragments', 'emptyFragments', 'overflow']) test((old ? 'before extraction' : 'shared owner') + ': exact 502 contract error for ' + refusal, async () => {
  const options = refusal === 'media' ? { mime: 'text/plain' } : refusal === 'declared' ? { declared: '16385' } : refusal === 'missing' ? { missing: true } : refusal === 'fragments' ? { parts: Array.from({ length: 4097 }, () => new Uint8Array([32])) } : refusal === 'emptyFragments' ? { parts: Array.from({ length: 129 }, () => new Uint8Array()) } : { parts: [new Uint8Array(16385)] };
  const f = await fixture(old, options); await assert.rejects(f.read('thread'), error => error instanceof f.ErrorClass && error.status === 502);
});
