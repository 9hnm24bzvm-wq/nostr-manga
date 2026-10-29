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
  const modules = new Map(), evaluated = [], requests = [], responses = [], storage = new Map(), fetchRequests = [], fetchResponses = [], fences = new Map();
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
    console, URL, URLSearchParams, setTimeout, clearTimeout, AbortSignal, AbortController, TextEncoder, TextDecoder, Response,
    FormData, XMLHttpRequest: FakeXHR,
    // Real native scope/client/library execute; only the HTTP and per-tab storage
    // boundaries are finite. Every unexpected fetch fails instead of networking.
    fetch: async (url, options) => {
      const request = { url: new URL(url), options }; fetchRequests.push(request);
      const reply = fetchResponses.shift(); assert.ok(reply, 'Unexpected account HTTP: ' + url);
      return typeof reply === 'function' ? reply(request) : new Response(JSON.stringify(reply.body), { status: reply.status ?? 200, headers: { 'content-type': 'application/json' } });
    },
    sessionStorage: { getItem: key => fences.get(key) ?? null, setItem: (key, value) => fences.set(key, value), removeItem: key => fences.delete(key) },
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
  return { modules, evaluated, requests, responses, read, session, storage, fetchRequests, fetchResponses, fences };
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
// Real gate-on bootstrap keeps guest/native IDs separate and now uses the real
// canonical account scope, store and versioned client after ordinary sign-in.
// HTTP and per-tab storage are finite boundaries; no account module is stubbed.
const preview = await composedGraph(entries[1], { publicCatalogue: '1' });
const previewStore = preview.modules.get('svelte/store').namespace;
const previewLibrary = preview.read('platform-integrations/local-library/library.ts');
const accountState = preview.read('platform-integrations/local-library/native-account-library.ts');
assert.equal(preview.read('platform-integrations/public-api/library-staging.ts').publicAccountLibraryUnavailable, false);
const preserved = { family: 'series', id: 'SeedManga1', status: 'reading', notification: true };
const publicId = '11111111-1111-8111-8111-111111111111', accountId = '22222222-2222-4222-8222-222222222222';
previewLibrary.localLibraryState.update(state => ({ ...state, ready: true, scopeStatus: 'verified', records: [preserved,{...preserved,id:publicId}] }));
const guestRecords = JSON.stringify(previewStore.get(previewLibrary.localLibraryState).records);
assert.equal(previewStore.get(previewLibrary.libraryIdentity).mode, 'device');
assert.equal(previewStore.get(previewLibrary.libraryIdentity).canUse, true);
assert.deepEqual(Object.keys(previewStore.get(previewLibrary.libraryIdentity).follows),[publicId]);
const requestsBefore = preview.requests.length;
await assert.rejects(() => previewLibrary.apiRead('/manga',new URLSearchParams()),error => error.code === 501);
await assert.rejects(() => previewLibrary.apiRead('/user/follows/manga',undefined,true),error => error.code === 409);
await assert.rejects(() => previewLibrary.accountRequest('/user/follows/manga/'+publicId,'POST',{}),error => error.code === 401);
assert.equal(preview.fetchRequests.length,0,'Anonymous private actions cannot choose a different identity or send a request');
const user = name => ({identity:{id:name,username:name},roles:[],permissions:[],follows:{SeedManga1:['reading',1]}});
preview.session.set(user('account-A'));
assert.equal(previewStore.get(previewLibrary.localLibraryState).mode,'account');
assert.equal(previewStore.get(previewLibrary.libraryIdentity).canUse,true);
assert.deepEqual(Object.keys(previewStore.get(previewLibrary.libraryIdentity).follows),[],'Legacy Session follows do not become canonical account saves');
const me = {result:'ok',response:'entity',data:{id:accountId,type:'user',attributes:{username:'account-A',isVerified:false,permissions:[]}},nostr:{scope:'instance-account',identity:'native-session-account',authentication:'native-session',nativeSessionAuthority:true}};
const envelope = data => ({result:'ok',authentication:'native-session',scope:'instance-account',accountId,revision:3,data});
preview.fetchResponses.push({body:me},{body:{...envelope([]),total:0,limit:20,page:1}});
const saved = await previewLibrary.apiRead('/user/follows/manga',new URLSearchParams({page:'1',limit:'20'}),true,'account-A');
assert.equal(saved.total,0);assert.equal(saved.data.length,0);
assert.deepEqual(preview.fetchRequests.map(row=>row.url.pathname),['/api/v1/account/me','/api/v1/account/library/follows']);
assert.equal(preview.fetchRequests[1].url.searchParams.get('limit'),'20');
assert.equal(preview.fetchRequests[1].url.searchParams.get('page'),'1');
const savedRow={mangaId:publicId,saved:true,status:'reading',notification:true,followedAt:'2026-10-28T00:00:00Z'};
preview.fetchResponses.push({body:envelope([{mangaId:publicId,saved:false}])},{body:{...envelope(savedRow),revision:4}});
const receipt=await previewLibrary.accountRequest('/user/follows/manga/'+publicId,'POST',{status:'reading',notification:true},'account-A');
assert.equal(receipt.status,204);
assert.deepEqual(preview.fetchRequests[2].url.searchParams.getAll('ids[]'),[publicId],'Save preflight is an exact selected-title read, not a library scan');
assert.equal(preview.fetchRequests[2].url.pathname,'/api/v1/account/library/follows/state');
const write=preview.fetchRequests[3];assert.equal(write.options.method,'POST');assert.equal(write.url.pathname,'/api/v1/account/library/follows/'+publicId);
assert.deepEqual(JSON.parse(new TextDecoder().decode(write.options.body)),{status:'reading',notification:true,expectedAccountId:accountId,revision:3});
assert.equal(previewStore.get(previewLibrary.libraryIdentity).follows[publicId][0],'reading');
assert.equal(preview.fences.size,0,'Acknowledged mutation clears only its uncertainty fence');
for(const request of preview.fetchRequests){assert.equal(request.options.credentials,'same-origin');assert.equal(request.options.cache,'no-store');assert.equal(request.options.redirect,'error');assert.equal(request.options.headers['X-Weebdex-Expected-Account'],'account-A');assert.equal(request.options.headers.Authorization,undefined);}
// A real in-flight account read cannot attach after a newer Session owner.
let release, began;const started=new Promise(resolve=>{began=resolve;});
preview.fetchResponses.push(()=>new Promise(resolve=>{release=()=>resolve(new Response(JSON.stringify({...envelope([]),total:0,limit:20,page:1}),{headers:{'content-type':'application/json'}}));began();}));
const pending=previewLibrary.apiRead('/user/follows/manga',new URLSearchParams(),true,'account-A');
const rejected=assert.rejects(pending,error=>error.outcome==='uncertain'&&/account could not be checked/.test(error.message));await started;
preview.session.set(user('account-B'));
assert.equal(preview.fetchRequests.at(-1).options.signal.aborted,true,'Authority changes abort the original native scope');
assert.deepEqual(Object.keys(previewStore.get(previewLibrary.libraryIdentity).follows),[]);
assert.equal(previewStore.get(accountState.nativeAccountLibraryState).owner,'account-B');
release();await rejected;
const privateCount=preview.fetchRequests.length;
await assert.rejects(()=>previewLibrary.apiRead('/user/follows/manga',new URLSearchParams(),true,'account-A'),error=>error.code===409);
assert.equal(preview.fetchRequests.length,privateCount,'A stale explicit owner cannot dispatch or retry as B');
preview.session.set(undefined);
assert.equal(previewStore.get(previewLibrary.localLibraryState).mode,'device');
assert.equal(previewStore.get(previewLibrary.libraryIdentity).follows[publicId][0],'reading');
assert.equal(JSON.stringify(previewStore.get(previewLibrary.localLibraryState).records),guestRecords,'Account use never adopted or changed guest records');
assert.equal(preview.requests.length,requestsBefore,'No native catalogue or extra sign-in fallback');
assert.equal(preview.fetchResponses.length,0);
results.push({entry:'gate-on guest preservation and real bounded canonical account integration',passed:true,storage_opened:false,private_requests:privateCount,native_catalogue_requests:0,records_preserved:true});
console.log(JSON.stringify({ schema: 1, baseline_reproduction: { error_class: baselineError.name, property: 'LimitHeight', source: 'frontend/src/lib/stores/prefs.ts' }, preferences_only_reproduction: { error_class: sessionFirstError.name, source: 'platform-integrations/local-library/library.ts', binding: 'Session' }, results, package_browser_apis: 'explicit stand-ins', browser_e2e: false, network: false, original_files_written: false }, null, 2));
