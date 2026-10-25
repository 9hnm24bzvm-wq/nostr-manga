// SOURCE-AUTHORED, UNEXECUTED. Real controller -> query -> owned transport ->
// validator. Synthetic HTTP bytes, never a stubbed controller query/order path.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
const plain = value => JSON.parse(JSON.stringify(value));
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const turns = async () => { for (let i = 0; i < 60; i++) await Promise.resolve(); };
const id = (publicMode, n) => publicMode ? '00000000-0000-8000-8000-' + String(n).padStart(12, '0') : 'chapter' + n;
const row = (publicMode, n, chapter = String(n), overrides = {}) => ({ id: id(publicMode, n), volume: '1', chapter, language: 'en', publishedAt: '2026-10-22 12:00:00.000001+00', version: 1, groupIds: [], sharedGroupScore: 0, witness: n.toString(16).padStart(64, '0'), ...overrides });
async function fixture(publicMode = false, pinnedEndpoint) {
  const calls = [], navigated = [], timers = new Map(), allocations = [];
  let epoch = 'A', historyStatus = 'complete-local', completed = new Set(), completionRead, navigateHook, responder, current = row(publicMode, 2);
  const profile = publicMode ? 'public-retained-v1' : 'native-aggregate-v1', mangaId = id(publicMode, 999), base = pinnedEndpoint?.base ?? (publicMode ? '/api/v1' : '/api'), origin = pinnedEndpoint?.origin ?? 'https://reader.test';
  const catalogIdentity = { catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
  const context = vm.createContext({ URL, URLSearchParams, TextEncoder, TextDecoder, AbortController, console,
    Uint8Array: new Proxy(Uint8Array, { construct(target, args) { allocations.push(args[0]); return Reflect.construct(target, args); } }),
    setTimeout(fn, ms) { const token = {}; timers.set(token, { fn, ms }); return token; }, clearTimeout(token) { timers.delete(token); } });
  const modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    const local = path.join(root, relative), file = fs.existsSync(local) ? local : path.join(fallback, relative);
    const mod = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts')))))); links.set(relative, linked); await linked; return mod;
  }
  const module = await load('platform-integrations/bounded-reader/controller.ts'); await module.evaluate();
  const client = modules.get('platform-integrations/bounded-reader/client.ts').namespace;
  function envelope(url, updates = {}) {
    const q = url.searchParams, operation = q.get('operation'), target = operation === 'next' || operation === 'last' ? row(publicMode, 3) : operation === 'previous' || operation === 'first' ? row(publicMode, 1) : current;
    return { version: 1, catalogIdentity, mangaId, operation, orderingProfile: profile, scope: 'a'.repeat(64), consistency: 'live-keyset-v1', crossRequestSnapshot: false, history: 'device-local-only', continueProof: 'observed-only', state: 'ready', restartRequired: false,
      data: [target], limit: Number(q.get('limit')), nextCursor: null, ...(q.has('current') ? { current } : {}),
      ...(operation === 'around' ? { previous: row(publicMode, 1), next: row(publicMode, 3) } : {}),
      ...(['first', 'last'].includes(operation) ? { hasMultipleEditions: false } : {}),
      ...(['editions', 'resolve-group'].includes(operation) ? { groupAnchor: current, winner: current, hasMultipleEditions: false } : {}), ...updates };
  }
  responder = url => envelope(url);
  const pinned = client.createReaderClient({ origin: () => origin,
    setTimer(fn, ms) { const token = {}; timers.set(token, { fn, ms }); return token; }, clearTimer(token) { timers.delete(token); },
    async fetch(url, init) { const parsed = new URL(url); calls.push({ url: parsed, init }); const value = await responder(parsed); return value instanceof Response ? value : new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } }); } }, pinnedEndpoint ? () => pinnedEndpoint : undefined);
  const destination = { origin, identity: catalogIdentity, base, profile, mangaId, filters: client.readerFilters(profile, new URLSearchParams({ tlang: 'en' })) };
  const controller = new module.namespace.BoundedReaderController(() => ({ destination, epoch, current, history: { key: epoch, status: historyStatus, completed: async (ids, budget) => { const value = completionRead ? await completionRead(ids, budget) : ids.some(id => completed.has(id)); budget.check(); budget.accept(ids.length); return value; }, reason: 'History is incomplete' } }), async (target, lastPage) => { navigated.push({ target: plain(target), lastPage }); if (navigateHook) await navigateHook(); }, pinned.read);
  return { controller, client, pinned, calls, navigated, timers, allocations, envelope, destination, catalogIdentity, async module(relative) { const module = await load(relative); if (module.status !== 'evaluated') await module.evaluate(); return module.namespace; }, set scopeEpoch(value) { epoch = value; },
    set navigateHook(value) { navigateHook = value; }, set completionRead(value) { completionRead = value; }, set responder(value) { responder = value; }, set current(value) { current = value; }, get current() { return current; }, set historyStatus(value) { historyStatus = value; }, set completed(value) { completed = value; },
    changeEpoch() { epoch += '!'; controller.sync(); }, expire(ms) { for (const timer of [...timers.values()]) if (timer.ms === ms) timer.fn(); } };
}
export { fixture, row, id, deferred, turns };
