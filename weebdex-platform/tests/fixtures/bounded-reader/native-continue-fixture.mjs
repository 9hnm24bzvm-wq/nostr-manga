// Actual current controller, native adapter, scope/DTO, bounded transport and
// selector client. Authority observations and HTTP/SQL results are synthetic.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash, webcrypto } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { row } from './controller-fixture.mjs';
const root = fileURLToPath(new URL('../../../', import.meta.url)), fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
export async function nativeContinueFixture() {
  const calls = [], navigated = [], timers = new Map(), authority = { actor: 'actor:one', epoch: 7, generation: 1 };
  const source = { context: { generation: 1, key: 'account-one', mode: 'account', accountId: authority.actor }, mode: 'account', selectionPending: false, selectionEpoch: 0, ready: false, revision: 4, accountRevision: 8, scopeStatus: 'pending', scope: undefined, reason: 'Device database is unavailable', records: [{ completed: true, privateNote: 'NEVER_UPLOAD_LOCAL_HISTORY' }] };
  const identity = { origin: 'https://native-api.test', catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
  let endpoint = { origin: identity.origin, base: '/custom/api', profile: 'native-aggregate-v1' }, cookieActor = authority.actor, responder, actionEpoch = 0, digestHook, digests = 0, nonce = 0, filters = 'tlang=en', navigateHook;
  let selection = { winner: row(false, 1), reason: 'oldest-unread', hasMultipleEditions: false };
  const snapshot = () => ({ ...source, admission: { selectionEpoch: source.selectionEpoch, authority: { ...authority } } });
  const canonicalScope = call => {
    const values = new Map(); for (const [raw, value] of call.url.searchParams) {
      if (['accountEpoch', 'catalogNamespace', 'serviceKey', 'requestId'].includes(raw)) continue;
      const key = raw.replace(/\[\]$/, ''); if (!values.has(key)) values.set(key, []); values.get(key).push(value);
    }
    const effective = Object.fromEntries([...values].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([key, values]) => [key, [...new Set(values)].sort()]));
    return createHash('sha256').update(JSON.stringify([{ catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey }, 'native-aggregate-v1', 'native-continue', 'manga:one', cookieActor, effective])).digest('hex');
  };
  const envelope = (call, change = {}) => {
    const selected = { ...selection, ...change }, winner = selected.winner;
    return { version: 1, operation: 'native-continue', orderingProfile: 'native-aggregate-v1', mangaId: 'manga:one', catalogIdentity: { catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey }, account: { id: cookieActor, epoch: authority.epoch }, requestId: call.url.searchParams.get('requestId'), scope: canonicalScope(call), consistency: 'live-request-v1', continueProof: 'native-account-snapshot-v1', crossRequestSnapshot: false, state: winner ? 'ready' : 'boundary', reason: winner ? selected.reason : 'empty', target: winner ? { volume: winner.volume, chapter: winner.chapter, anchorChapterId: winner.id } : null, winner, hasMultipleEditions: winner ? selected.hasMultipleEditions : false };
  };
  const selector = (call, change = {}) => ({ version: 1, mangaId: 'manga:one', operation: call.url.searchParams.get('operation'), orderingProfile: 'native-aggregate-v1', catalogIdentity: { catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey }, scope: 'b'.repeat(64), consistency: 'live-keyset-v1', crossRequestSnapshot: false, history: 'device-local-only', continueProof: 'observed-only', state: 'ready', restartRequired: false, limit: Number(call.url.searchParams.get('limit')), data: [selection.winner], nextCursor: null, groupAnchor: selection.winner, winner: selection.winner, hasMultipleEditions: selection.hasMultipleEditions, ...change });
  const context = vm.createContext({ console, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, AbortController,
    crypto: { randomUUID: () => '00000000-0000-4000-8000-' + String(++nonce).padStart(12, '0'), subtle: { digest(algorithm, bytes) { digests++; return digestHook ? digestHook(algorithm, bytes) : webcrypto.subtle.digest(algorithm, bytes); } } },
    setTimeout(fn, ms) { const key = {}; timers.set(key, { fn, ms }); return key; }, clearTimeout(key) { timers.delete(key); },
    fetch: async (url, options) => { const call = { url: new URL(url), options }; calls.push(call);
      if (call.url.pathname.endsWith('/continue') && options.headers['X-WeebDex-Expected-Account'] !== cookieActor) return new Response('', { status: 409 });
      const body = responder ? await responder(call) : call.url.pathname.endsWith('/continue') ? envelope(call) : selector(call);
      return body instanceof Response || body?.body?.getReader ? body : new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } }); }
  });
  const modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (relative === 'platform-integrations/local-library/library.ts') {
      const values = { readerLibrarySource: snapshot, assertLibraryAdmission(expected) { if (expected.selectionEpoch !== source.selectionEpoch || expected.authority.actor !== authority.actor || expected.authority.epoch !== authority.epoch) throw Error('Library admission changed'); } };
      const mod = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context }); modules.set(relative, mod); return mod;
    }
    const local = path.join(root, relative), file = fs.existsSync(local) ? local : path.join(fallback, relative), text = fs.readFileSync(file, 'utf8');
    const mod = new vm.SourceTextModule(relative.endsWith('.ts') ? stripTypeScriptTypes(text, { mode: 'transform' }) : text, { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts')))))); links.set(relative, linked); await linked; return mod;
  }
  const module = async relative => { const mod = await load(relative); if (mod.status !== 'evaluated') await mod.evaluate(); return mod.namespace; };
  const native = await module('platform-integrations/bounded-reader/native-continue.ts'), client = await module('platform-integrations/bounded-reader/client.ts'), core = await module('platform-integrations/bounded-reader/controller.ts');
  const factory = () => native.createNativeContinueAccess(() => endpoint), access = factory();
  const destination = () => ({ ...endpoint, mangaId: 'manga:one', filters, identity: { catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey } });
  const reader = client.createReaderClient({ origin: () => endpoint.origin, fetch: context.fetch, setTimer: context.setTimeout, clearTimer: context.clearTimeout }, () => endpoint);
  const controller = new core.BoundedReaderController(() => ({ destination: destination(), epoch: String(actionEpoch), history: { key: 'unavailable-badge-history', status: 'unavailable', completed() { throw Error('Automatic native Continue must not read badge/device completion'); } }, nativeContinue: access('manga:one', identity) }), async (target, lastPage) => { navigated.push({ target: JSON.parse(JSON.stringify(target)), lastPage }); if (navigateHook) await navigateHook(); }, reader.read);
  const budget = () => { const epoch = actionEpoch; return new client.ReaderBudget(() => { if (actionEpoch !== epoch) throw Error('Action scope changed'); }); };
  return { source, identity, authority, calls, timers, navigated, controller, snapshot, module, envelope, selector, canonicalScope, budget, access: () => access('manga:one', identity), remountAccess: () => factory()('manga:one', identity), get digests() { return digests; },
    set selection(value) { selection = value; }, get selection() { return selection; }, set responder(value) { responder = value; }, set endpoint(value) { endpoint = value; }, get endpoint() { return endpoint; }, set cookieActor(value) { cookieActor = value; }, set filters(value) { filters = value; }, set digestHook(value) { digestHook = value; }, set navigateHook(value) { navigateHook = value; },
    invalidate() { actionEpoch++; controller.sync(); }, expire(ms) { for (const item of [...timers.values()]) if (item.ms === ms) item.fn(); },
    async until(predicate) { for (let i = 0; i < 100; i++) { if (predicate()) return; await new Promise(resolve => setTimeout(resolve, 2)); } assert.fail('Expected bounded native fixture boundary was not observed'); }
  };
}
