#!/usr/bin/env node
// Finite module-order regression seam. No browser, server, network or real storage.
// Project modules, anchored overlay transforms and the native-library resolver are
// real. External package/browser APIs are explicit stand-ins, so this is not E2E.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays, createLocalLibraryResolver } from '../../platform-integrations/local-library/overlays.mjs';

if (!vm.SourceTextModule) throw Error('Run with node --experimental-vm-modules');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const overlays = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)];
const nativeResolver = createLocalLibraryResolver(frontend, path.join(root, 'platform-integrations/local-library'));
const originalPreferencesImport = 'import { Constants } from "$lib";';
const repairedPreferencesImport = 'import * as Constants from "$lib/constants";';
const bootstrapRepairs = [
  ['frontend/src/lib/stores/prefs.ts', repairedPreferencesImport, originalPreferencesImport],
  ['frontend/src/lib/stores/Session.ts', 'import * as Auth from "$lib/API/Auth";', 'import API from "$lib/API";'],
  ['frontend/src/lib/stores/Session.ts', 'Auth.Check()', 'API.Auth.Check()'],
  ['frontend/src/lib/API/Auth.ts', 'import HTTP from "$lib/HTTP";', 'import { HTTP } from "$lib";'],
];
const storeStandIn = `
export function writable(value) {
  const listeners = new Set();
  const set = next => { value = next; for (const callback of [...listeners]) callback(value); };
  return { subscribe(callback) { listeners.add(callback); callback(value); return () => listeners.delete(callback); }, set, update(callback) { set(callback(value)); } };
}
export function get(store) { let value; const off = store.subscribe(next => value = next); off(); return value; }
export function derived(stores, fn) {
  if (!Array.isArray(stores)) stores = [stores];
  return { subscribe(callback) {
    const values = [], ready = new Set();
    const offs = stores.map((store, index) => store.subscribe(value => {
      values[index] = value; ready.add(index);
      if (ready.size === stores.length) callback(fn(values));
    }));
    return () => offs.forEach(off => off());
  } };
}`;
const externalStandIns = Object.freeze({
  'svelte/store': storeStandIn,
  'svelte': 'export const onDestroy = () => {}, createContext = () => [() => ({}), value => value];',
  '$app/state': 'export const page = { url: new URL("http://localhost:1/"), state: {} };',
  '$app/navigation': 'export const beforeNavigate = () => {}, afterNavigate = () => {};',
  '$app/environment': 'export const browser = true;',
  '$env/static/public': 'export const PUBLIC_MANGADEX_CATALOGUE = "0", PUBLIC_API_URL = "/api", PUBLIC_KRATOS_PUBLIC_API = "/auth", PUBLIC_COVER_URL = "/covers", PUBLIC_DATA_URL = "/data", PUBLIC_UPLOAD_URL = "/uploads", PUBLIC_PROXY_URL = "/proxy", PUBLIC_OPENGRAPH_URL = "/og-image", PUBLIC_SITE_NAME = "WeebDex";',
  'svelte/reactivity': 'export class MediaQuery { current = false; constructor(value) { this.value = value; } }',
  'svelte-sonner': 'export const toast = { error() {}, success() {}, info() {} };',
  'is-mobile': 'export default () => false;',
  'http-status-codes': 'export const StatusCodes = { OK: 200, UNAUTHORIZED: 401, FORBIDDEN: 403, TOO_MANY_REQUESTS: 429 };',
  'limiter': 'export class RateLimiter { tryRemoveTokens() { return true; } }',
  '@ory/client': 'export class Configuration {}; export class FrontendApi {};',
  'clsx': 'export const clsx = (...values) => values.join(" ");',
  'dompurify': 'export default { sanitize: value => value };',
  'isbot': 'export const isbot = () => false;',
  'marked': 'export const marked = { parse: value => value };',
  'slugify': 'export default value => value;',
  'tailwind-merge': 'export const twMerge = value => value;',
  '@noble/hashes/sha2.js': 'export const sha256 = value => value;',
});

async function composedGraph(entry, { regressImport = false, publicCatalogue = '0' } = {}) {
  const modules = new Map(), evaluated = [], requests = [], responses = [], storage = new Map();
  class FakeXHR extends EventTarget {
    withCredentials = false;
    open(method, url) { this.method = method; this.url = url; }
    setRequestHeader() {}
    getResponseHeader() { return 'application/json'; }
    send() {
      requests.push({ method: this.method, url: this.url, withCredentials: this.withCredentials });
      const response = responses.shift() ?? { status: 401 };
      this.status = response.status; this.response = response.body ? JSON.stringify(response.body) : '';
      this.responseText = this.response;
      queueMicrotask(() => this.dispatchEvent(new Event('load')));
    }
    abort() { this.dispatchEvent(new Event('abort')); }
  }
  const context = vm.createContext({
    console, URL, URLSearchParams, setTimeout, clearTimeout, AbortSignal, TextEncoder, TextDecoder,
    FormData, XMLHttpRequest: FakeXHR,
    localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) },
    window: { location: { href: 'http://localhost:1/', origin: 'http://localhost:1' }, navigator: { webdriver: true, userAgent: 'test' }, setTimeout, clearTimeout },
    __evaluated: evaluated,
  });
  function resolve(specifier, importer) {
    const native = nativeResolver.resolveId(specifier, importer);
    if (native) return native;
    if (specifier === '$lib') return path.join(frontend, 'src/lib/index.ts');
    if (specifier.startsWith('$lib/')) specifier = path.join(frontend, 'src/lib', specifier.slice(5));
    else if (specifier.startsWith('$platform/')) specifier = path.join(root, 'platform-integrations/frontend-behavior', specifier.slice(10));
    else if (specifier.startsWith('$localLibrary/')) specifier = path.join(root, 'platform-integrations/local-library', specifier.slice(14));
    else if (specifier.startsWith('.')) specifier = path.resolve(path.dirname(importer), specifier);
    else if (!specifier.startsWith('/')) {
      if (!Object.hasOwn(externalStandIns, specifier)) throw Error('Unreviewed external stand-in: ' + specifier);
      return specifier;
    }
    // The production Vite wrapper has this exact retained-case repair.
    if (specifier === path.join(frontend, 'src/lib/stores/session')) return path.join(frontend, 'src/lib/stores/Session.ts');
    for (const candidate of [specifier, specifier + '.ts', specifier + '.js', specifier + '.svelte.ts', path.join(specifier, 'index.ts')]) {
      if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate;
    }
    throw Error('Unresolved project module: ' + specifier);
  }
  function moduleFor(identifier) {
    if (modules.has(identifier)) return modules.get(identifier);
    let code = externalStandIns[identifier];
    if (identifier === '$env/static/public') code = code.replace('PUBLIC_MANGADEX_CATALOGUE = "0"', 'PUBLIC_MANGADEX_CATALOGUE = '+JSON.stringify(publicCatalogue));
    if (code === undefined) {
      code = fs.readFileSync(identifier, 'utf8');
      for (const overlay of overlays) code = overlay.transform.handler(code, identifier)?.code ?? code;
      if (regressImport) for (const [relative, repaired, original] of bootstrapRepairs) {
        if (identifier !== path.join(root, relative) || regressImport === 'session-edges' && relative === 'frontend/src/lib/stores/prefs.ts') continue;
        assert.equal(code.split(repaired).length - 1, 1);
        code = code.replace(repaired, original);
      }
      // Native Node transformation supports enums/parameter properties. Svelte
      // rune/UI execution is outside this module-order seam.
      if (identifier.endsWith('.ts')) code = stripTypeScriptTypes(code, { mode: 'transform' });
    }
    code += '\n__evaluated.push(' + JSON.stringify(path.isAbsolute(identifier) ? path.relative(root, identifier) : identifier) + ');';
    const module = new vm.SourceTextModule(code, { context, identifier });
    modules.set(identifier, module); return module;
  }
  // Start with each real entry, then connect its normal shared consumers. A
  // Session-first leaf no longer eagerly imports library/API as a side effect.
  const main = new vm.SourceTextModule([
    path.join(root, entry),
    path.join(root, 'platform-integrations/local-library/library.ts'),
    path.join(root, 'frontend/src/lib/stores/index.ts'),
  ].map(identifier => 'import ' + JSON.stringify(identifier) + ';').join('\n'), { context, identifier: path.join(root, 'module-order-test-entry.mjs') });
  await main.link((specifier, parent) => moduleFor(resolve(specifier, parent.identifier)));
  await main.evaluate();
  const read = relative => {
    const module = modules.get(path.join(root, relative));
    assert.ok(module && module.status === 'evaluated', 'Expected initialized module: ' + relative);
    return module.namespace;
  };
  const session = read('frontend/src/lib/stores/Session.ts').default;
  // Drain the real Session initialize/Mutex/HTTP composition using the
  // synthetic anonymous response; no real request ever leaves this process.
  assert.equal(await session.isValid(), false);
  return { modules, evaluated, requests, responses, read, session, storage };
}

const entries = [
  'frontend/src/lib/stores/index.ts',
  'frontend/src/lib/API/index.ts',
  'frontend/src/lib/stores/Session.ts',
  'frontend/src/lib/index.ts',
  'platform-integrations/local-library/library.ts',
];
let baselineError;
try { await composedGraph(entries[0], { regressImport: true }); }
catch (error) { baselineError = error; }
assert.equal(baselineError?.name, 'TypeError');
assert.match(baselineError?.message ?? '', /undefined.*LimitHeight/);
assert.match(baselineError?.stack ?? '', /frontend\/src\/lib\/stores\/prefs\.ts:/);

// A preferences-only repair is insufficient for a Session-first bundle.
let sessionFirstError;
try { await composedGraph(entries[2], { regressImport: 'session-edges' }); }
catch (error) { sessionFirstError = error; }
assert.equal(sessionFirstError?.name, 'ReferenceError');
assert.match(sessionFirstError?.message ?? '', /Session.*before initialization/);
assert.match(sessionFirstError?.stack ?? '', /platform-integrations\/local-library\/library\.ts:/);

const results = [];
for (const entry of entries) {
  const graph = await composedGraph(entry);
  const store = graph.modules.get('svelte/store').namespace;
  const preferences = graph.read('frontend/src/lib/stores/prefs.ts');
  const constants = graph.read('frontend/src/lib/constants/Reader.ts');
  assert.equal(store.get(preferences.mangaPrefs).scale, constants.Scale.LimitHeight);
  assert.equal(store.get(preferences.webtoonPrefs).scale, constants.Scale.LimitWidth);
  const authority = graph.read('platform-integrations/frontend-behavior/mutation-authority.ts');
  const library = graph.read('platform-integrations/local-library/library.ts');
  const before = library.captureLibraryContext(), anonymous = authority.captureNativeAuthority();
  graph.responses.push({ status: 200, body: { identity: { id: 'account-A', username: 'actor-A' }, roles: [], permissions: [], preferences: { items_per_page: '17' } } });
  await graph.session.initialize(true);
  assert.equal(store.get(preferences.prefs).items_per_page, '17');
  assert.equal(library.captureLibraryContext().accountId, 'account-A');
  assert.equal(authority.sameNativeAuthority(anonymous), false);
  const savedSeries = { family: 'series', id: 'SeedManga1', status: 'reading', notification: true };
  library.localLibraryState.update(state => ({ ...state, ready: true, scopeStatus: 'verified', records: [savedSeries] }));
  assert.equal(store.get(library.libraryIdentity).mode, 'device');
  assert.equal(store.get(library.libraryIdentity).canUse, true);
  assert.equal(store.get(library.libraryIdentity).follows.SeedManga1[0], 'reading');
  library.localLibraryState.update(state => ({ ...state, mode: 'account' }));
  assert.equal(store.get(library.libraryIdentity).mode, 'account');
  assert.equal(store.get(library.libraryIdentity).canUse, true);
  assert.throws(() => library.assertLibraryContext(before), error => error.code === 409 && error.title === 'Library context changed');
  const contextA = library.captureLibraryContext(), actorA = authority.captureNativeAuthority();
  graph.session.set({ identity: { id: 'account-B', username: 'actor-B' }, roles: [], permissions: [] });
  assert.equal(library.captureLibraryContext().accountId, 'account-B');
  assert.equal(authority.sameNativeReadAuthority(actorA), false);
  assert.throws(() => library.assertLibraryContext(contextA), error => error.code === 409 && error.title === 'Library context changed');
  graph.responses.push({ status: 401 }); await graph.session.initialize(true);
  assert.equal(library.captureLibraryContext().accountId, null);
  assert.equal(store.get(library.localLibraryState).mode, 'account');
  assert.equal(store.get(library.localLibraryState).ready, true);
  assert.equal(store.get(library.libraryIdentity).canUse, false);
  assert.equal(store.get(library.localLibraryState).records[0].id, 'SeedManga1');
  library.localLibraryState.update(state => ({ ...state, mode: 'device' }));
  assert.equal(store.get(library.libraryIdentity).canUse, true);
  assert.equal(store.get(library.libraryIdentity).follows.SeedManga1[0], 'reading');
  assert.equal(graph.requests.length, 3);
  assert.ok(graph.requests.every(request => request.method === 'GET' && request.url === '/api/auth/check' && request.withCredentials));
  results.push({ entry, composed_modules_evaluated: graph.evaluated.length, passed: true });
}
// Real composed library/API exports reject the staged UUID view before any
// device open, native request, or mutation. Existing rows remain inspectable.
const preview = await composedGraph(entries[1], { publicCatalogue: '1' });
const previewStore = preview.modules.get('svelte/store').namespace;
const previewLibrary = preview.read('platform-integrations/local-library/library.ts');
const preserved = { family: 'series', id: 'SeedManga1', status: 'reading', notification: true };
previewLibrary.localLibraryState.update(state => ({ ...state, ready: true, scopeStatus: 'verified', records: [preserved] }));
assert.equal(previewStore.get(previewLibrary.libraryIdentity).canUse, false);
assert.equal(Object.keys(previewStore.get(previewLibrary.libraryIdentity).follows).length, 0);
const requestsBefore = preview.requests.length;
const unavailable = error => error.code === 501 && /Saved-library changes are unavailable/.test(error.error);
for (const [name,args] of [['assertCatalogueScope',[]],['apiRead',['/manga',new URLSearchParams()]],['localAction',[()=>{throw Error('A staged mutation must not run');}]],['saveSeries',['11111111-1111-8111-8111-111111111111','reading']],['removeSeries',['11111111-1111-8111-8111-111111111111',()=>{}]],['updateChapters',['11111111-1111-8111-8111-111111111111',[],[],()=>{}]],['saveProgress',['11111111-1111-8111-8111-111111111111','22222222-2222-8222-8222-222222222222',1,false,()=>{}]]]) await assert.rejects(() => previewLibrary[name](...args),unavailable);
for (const family of ['MangaFollow','ChapterHistory']) {
 const api = preview.read('platform-integrations/local-library/'+family+'.ts');
 for (const name of Object.keys(api)) if(typeof api[name]==='function') await assert.rejects(() => api[name](new URLSearchParams(),{}),unavailable);
}
assert.equal(preview.requests.length,requestsBefore);
assert.equal(previewStore.get(previewLibrary.localLibraryState).records[0].id,'SeedManga1');
results.push({entry:'gate-on library preview',passed:true,storage_opened:false,native_requests:0,records_preserved:true});
console.log(JSON.stringify({ schema: 1, baseline_reproduction: { error_class: baselineError.name, property: 'LimitHeight', source: 'frontend/src/lib/stores/prefs.ts' }, preferences_only_reproduction: { error_class: sessionFirstError.name, source: 'platform-integrations/local-library/library.ts', binding: 'Session' }, results, package_browser_apis: 'explicit stand-ins', browser_e2e: false, network: false, original_files_written: false }, null, 2));
