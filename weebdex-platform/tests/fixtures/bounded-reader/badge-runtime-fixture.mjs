// SOURCE-AUTHORED, UNEXECUTED. The actual runtime module, controller, transport,
// validators, endpoint and navigation-intent modules execute. Svelte lifecycle,
// authority stores and local-library observations are explicit controlled seams;
// this is not compiled Svelte, real IndexedDB, cookies or browser qualification.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash, webcrypto } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { row, id, deferred, turns } from './controller-fixture.mjs';
const root = fileURLToPath(new URL('../../../', import.meta.url)), frozen = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
function store(value) { const listeners = new Set(); return { get value() { return value; }, subscribe(listener) { listeners.add(listener); listener(value); return () => listeners.delete(listener); }, set(next) { value = next; for (const listener of listeners) listener(value); }, touch() { for (const listener of listeners) listener(value); } }; }
export async function badgeRuntimeFixture(publicMode = false, title = false) {
  const contexts = new Map(), mounts = [], destroys = [], effects = [], calls = [], navigated = [], timers = new Map(), native = store({ actor: null, epoch: 0 }), local = store({}), prefs = Object.assign(store({}), { applyChapterPreferences(query) { query.append('tlang', 'en'); } });
  const source = { admission: { selectionEpoch: 0, authority: { actor: undefined, epoch: 0 } }, selectionPending: false, context: { generation: 1, mode: 'device' }, mode: 'device', ready: false, revision: 0, accountRevision: 0, scopeStatus: 'pending', scope: undefined, reason: undefined, observedCatalogue: undefined };
  let dbReads = 0; Object.defineProperty(source, 'db', { get() { dbReads++; throw Error('Ordinary navigation must not open device history'); } });
  const current = row(publicMode, 2), chapter = { ...current, relationships: { groups: [], manga: { id: id(publicMode, 999), title: 'Fixture' } } }, catalogIdentity = { catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: null };
  const pageHref = 'https://reader.test/chapter/' + current.id + '/1', api = 'https://native-api.test/custom/api', capabilityUrl = publicMode ? 'https://reader.test/api/protocol/capabilities' : api + '/protocol/capabilities';
  let responder;
  const envelope = url => { const operation = url.searchParams.get('operation'), target = operation === 'next' || operation === 'last' ? row(publicMode, 3) : operation === 'previous' || operation === 'first' ? row(publicMode, 1) : current; return { version: 1, mangaId: chapter.relationships.manga.id, operation, orderingProfile: publicMode ? 'public-retained-v1' : 'native-aggregate-v1', catalogIdentity, scope: 'a'.repeat(64), consistency: 'live-keyset-v1', crossRequestSnapshot: false, history: 'device-local-only', continueProof: 'observed-only', state: 'ready', restartRequired: false, data: [target], ...(url.searchParams.has('current') ? { current } : {}), nextCursor: null, limit: Number(url.searchParams.get('limit')), ...(operation === 'around' ? { next: row(publicMode, 3), previous: row(publicMode, 1) } : {}), ...(['first', 'last'].includes(operation) ? { hasMultipleEditions: false } : {}) }; };
  responder = url => url.href === capabilityUrl ? { catalog_namespace: catalogIdentity.catalogNamespace, service_key: catalogIdentity.serviceKey } : envelope(url);
  const context = vm.createContext({ console, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, AbortController, queueMicrotask, crypto: webcrypto,
    window: { location: { href: pageHref } }, $effect(fn) { effects.push(fn); fn(); },
    setTimeout(fn, ms) { const key = {}; timers.set(key, { fn, ms }); return key; }, clearTimeout(key) { timers.delete(key); },
    async fetch(url, options) { const parsed = new URL(url); calls.push({ url: parsed, options }); const value = await responder(parsed); return value instanceof Response ? value : new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } }); } });
  const stubs = new Map([
    ['svelte', { onMount: fn => mounts.push(fn), onDestroy: fn => destroys.push(fn), getContext: key => contexts.get(key), setContext: (key, value) => { contexts.set(key, value); return value; } }],
    ['svelte/store', { readable: value => store(value) }],
    ['$app/navigation', { goto: async target => { navigated.push(target); }, beforeNavigate() {} }], ['$app/paths', { resolve: value => value }],
    ['$env/static/public', { PUBLIC_API_URL: api }], ['$lib/stores', { prefs }],
    ['platform-integrations/local-library/library.ts', { localLibraryState: local, readerLibrarySelection: local, readerLibrarySource: () => source, assertLibraryAdmission(expected) { assert.equal(expected.selectionEpoch, source.admission.selectionEpoch); assert.equal(expected.authority.actor, source.admission.authority.actor); assert.equal(expected.authority.epoch, source.admission.authority.epoch); } }],
    ['platform-integrations/public-api/library-staging.ts', { publicUuidLibraryEnabled: publicMode }],
    ['platform-integrations/frontend-behavior/mutation-authority.ts', { nativeAuthority: native, captureNativeAuthority: () => native.value }]
  ]), modules = new Map(), links = new Map();
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (stubs.has(relative)) { const values = stubs.get(relative), mod = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context }); modules.set(relative, mod); return mod; }
    const localPath = path.join(root, relative), file = fs.existsSync(localPath) ? localPath : path.join(frozen, relative), input = fs.readFileSync(file, 'utf8');
    const mod = new vm.SourceTextModule(relative.endsWith('.ts') ? stripTypeScriptTypes(input, { mode: 'transform' }) : input, { context, identifier: relative }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(specifier.startsWith('.') ? path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (specifier.endsWith('.svelte') || !path.extname(specifier) ? '.ts' : ''))) : specifier))); links.set(relative, linked); await linked; return mod;
  }
  const mod = await load('platform-integrations/bounded-reader/runtime.svelte.ts'); await mod.evaluate();
  const client = await load('platform-integrations/bounded-reader/client.ts'); await client.evaluate();
  const navigation = await load('platform-integrations/frontend-behavior/navigation-authority.ts'); await navigation.evaluate();
  const controller = mod.namespace.createBoundedReader({ manga: () => chapter.relationships.manga.id, ...(title ? {} : { current: () => chapter }), filters: () => new URLSearchParams({ tlang: 'en' }), queryEpoch: () => pageHref });
  const settle = async () => { for (let i = 0; i < 20; i++) { await turns(); if (!controller.snapshot.pending) return; } assert.fail('Owned runtime did not settle'); };
  return { navigationStart() { navigation.namespace.invalidateNavigationEpoch(); effects.forEach(fn => fn()); }, async badges(options) { const module = await load('platform-integrations/bounded-reader/visible-badges.svelte.ts'); await module.evaluate(); return module.namespace.createVisibleChapterBadges(options); }, effects() { effects.forEach(fn => fn()); }, mangaId: chapter.relationships.manga.id, chapter, identity: mod.namespace.verifyReaderIdentity, budget: assertCurrent => new client.namespace.ReaderBudget(assertCurrent ?? (() => {})), controller, source, local, native, calls, navigated, catalogIdentity, capabilityUrl, envelope, get dbReads() { return dbReads; }, set responder(value) { responder = value; }, async mount() { mounts.forEach(fn => fn()); await settle(); }, settle, dispose() { destroys.forEach(fn => fn()); }, observe(scope) { source.observedCatalogue = { url: capabilityUrl, scope, generation: 1 }; local.touch(); } };
}
