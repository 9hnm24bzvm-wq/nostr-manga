// Mounted retained reader seams, not a native browser or the whole application.
// Network-shaped requests use stdio: either the retained owned-HTTP runner or
// the separately admitted bounded fixture's pipe-only fixed response owner.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import readline from 'node:readline';
import assert from 'node:assert/strict';
import { types as nodeUtilTypes } from 'node:util';
import { mountedSources } from './bounded-source-map.mjs';
import { runBoundedMountedCases, BOUNDED_MOUNTED_CASES } from './bounded-mounted-cases.mjs';
import { createHash } from 'node:crypto';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
// BEGIN MOUNTED NODE DIAGNOSTICS
// Capture trusted builtins before any application/overlay imports execute.
// Warning text/stack is not provenance. An owned real provider call must cause
// original emitWarning to schedule its exact native Error and callback. Only
// that owned callback dispatch may pair the warning event with console output.
const diagnosticError = Error, diagnosticPrototype = Error.prototype;
const diagnosticDescriptor = Object.getOwnPropertyDescriptor, diagnosticApply = Reflect.apply;
const diagnosticEmitWarning = process.emitWarning, diagnosticNextTick = process.nextTick, diagnosticEmit = process.emit;
const diagnosticStrip = stripTypeScriptTypes, diagnosticSourceModule = vm.SourceTextModule;
const diagnosticRawListeners = process.rawListeners;
const diagnosticNativeListeners = diagnosticApply(diagnosticRawListeners, process, ['warning']);
const diagnosticIsError = nodeUtilTypes.isNativeError, diagnosticIsProxy = nodeUtilTypes.isProxy;
const diagnosticStackGetter = diagnosticDescriptor(new diagnosticError(''), 'stack')?.get;
const diagnosticStackLimit = diagnosticDescriptor(diagnosticError, 'stackTraceLimit')?.value;
const diagnosticPrepareChain = [diagnosticError, Function.prototype, Object.prototype];
const diagnosticMessages = Object.freeze({
  'node-type-stripping': 'stripTypeScriptTypes is an experimental feature and might change at any time',
  'node-vm-modules': 'VM Modules is an experimental feature and might change at any time'
});
function diagnosticLocalStack() {
  // Only OUR fresh Error is read. Never inspect an argument accessor, or call
  // an installed prepareStackTrace (including an inherited hook).
  if (globalThis.Error !== diagnosticError || diagnosticError.prototype !== diagnosticPrototype
      || typeof diagnosticStackGetter !== 'function' || !Number.isInteger(diagnosticStackLimit)
      || diagnosticStackLimit < 6 || diagnosticStackLimit > 20
      || diagnosticDescriptor(diagnosticError, 'stackTraceLimit')?.value !== diagnosticStackLimit
      || diagnosticPrepareChain.some(value => diagnosticDescriptor(value, 'prepareStackTrace'))
      || Object.getPrototypeOf(diagnosticError) !== diagnosticPrepareChain[1]
      || Object.getPrototypeOf(diagnosticPrepareChain[1]) !== diagnosticPrepareChain[2]
      || Object.getPrototypeOf(diagnosticPrepareChain[2]) !== null) return '';
  const error = new diagnosticError('');
  Object.defineProperty(error, 'name', { value: 'Error', writable: true, configurable: true });
  if (diagnosticDescriptor(error, 'stack')?.get !== diagnosticStackGetter) return '';
  try { const text = diagnosticApply(diagnosticStackGetter, error, []); return typeof text === 'string' ? text.slice(0, 4096) : ''; } catch { return ''; }
}
function diagnosticWarningShape(error, category) {
  // These checks inspect descriptors only, not getters. Shape alone NEVER
  // authorizes a diagnostic: even a perfectly forged Error has no dispatch token.
  try {
    if (!error || typeof error !== 'object' || diagnosticIsProxy(error) || !diagnosticIsError(error)
        || Object.getPrototypeOf(error) !== diagnosticPrototype || !Object.hasOwn(diagnosticMessages, category)) return false;
    const keys = Reflect.ownKeys(error), name = diagnosticDescriptor(error, 'name'), message = diagnosticDescriptor(error, 'message');
    return keys.length === 3 && ['stack', 'name', 'message'].every(key => keys.includes(key))
      && name?.value === 'ExperimentalWarning' && name.writable === true && name.enumerable === true && name.configurable === true
      && message?.value === diagnosticMessages[category] && message.writable === true && message.enumerable === false && message.configurable === true;
  } catch { return false; }
}
function diagnosticDataOptions(options, names) {
  if (options === undefined) return true;
  if (!options || typeof options !== 'object' || diagnosticIsProxy(options) || Object.getPrototypeOf(options) !== Object.prototype) return false;
  return Reflect.ownKeys(options).every(key => typeof key === 'string' && names.includes(key) && Object.hasOwn(diagnosticDescriptor(options, key), 'value'));
}
function createMountedNodeDiagnostics(enabled = process.env.WEEBDEX_BOUNDED_MOUNTED_FIXTURE === '1') {
  const previousError = console.error;
  const accepted = { 'node-type-stripping': 0, 'node-vm-modules': 0 };
  const attempted = new Set(), queued = new Set(), drains = new Set();
  let setup = true, closed = false, cleanup = false, failures = 0, scheduled = 0, completed = 0;
  let dispatch = null, pendingWarning = null;
  const reject = () => { failures++; semanticFailure('node-warning', []); };
  const listenersIntact = () => {
    const list = diagnosticApply(diagnosticRawListeners, process, ['warning']);
    return diagnosticNativeListeners.length === 1 && diagnosticDescriptor(diagnosticNativeListeners[0], 'name')?.value === 'onWarning'
      && list.length === 3 && list[0] === beginWarning && list[1] === diagnosticNativeListeners[0] && list[2] === endWarning;
  };
  function provider(category, operation) {
    if (!enabled || !setup || attempted.has(category)) return operation();
    attempted.add(category);
    if (closed || process.emitWarning !== diagnosticEmitWarning || process.nextTick !== diagnosticNextTick
        || process.emit !== diagnosticEmit || !listenersIntact()) { reject(); return operation(); }
    let insideNativeEmit = false, providerScheduled = 0;
    function captureEmit(...args) {
      // The caller is the captured builtin provider, not evaluated user source.
      const expected = args.length === 4 && args[0] === diagnosticMessages[category] && args[1] === 'ExperimentalWarning'
        && args[2] === undefined && args[3] === undefined;
      if (!expected || insideNativeEmit) return diagnosticApply(diagnosticEmitWarning, this, args);
      insideNativeEmit = true;
      try { return diagnosticApply(diagnosticEmitWarning, this, args); } finally { insideNativeEmit = false; }
    }
    function captureTick(callback, ...args) {
      if (!insideNativeEmit || typeof callback !== 'function' || diagnosticDescriptor(callback, 'name')?.value !== 'doEmitWarning'
          || args.length !== 1 || !diagnosticWarningShape(args[0], category)) return diagnosticApply(diagnosticNextTick, this, [callback, ...args]);
      if (++providerScheduled !== 1) { reject(); return diagnosticApply(diagnosticNextTick, this, [callback, ...args]); }
      const token = { error: args[0], category, observed: false, validated: false };
      queued.add(token); scheduled++;
      return diagnosticApply(diagnosticNextTick, this, [function ownedWarningDispatch(...values) {
        const valid = !closed && dispatch === null && queued.has(token) && values.length === 1 && values[0] === token.error
          && process.emitWarning === diagnosticEmitWarning && process.nextTick === diagnosticNextTick && process.emit === diagnosticEmit;
        if (valid) dispatch = token; else reject();
        try { return diagnosticApply(callback, this, values); }
        finally {
          if (!token.validated) reject();
          if (dispatch === token) dispatch = null;
          queued.delete(token); completed++;
          if (queued.size === 0) { for (const resolve of drains) resolve(); drains.clear(); }
        }
      }, ...args]);
    }
    process.emitWarning = captureEmit; process.nextTick = captureTick;
    try { return operation(); }
    finally {
      if (process.emitWarning === captureEmit) process.emitWarning = diagnosticEmitWarning; else reject();
      if (process.nextTick === captureTick) process.nextTick = diagnosticNextTick; else reject();
      if (providerScheduled !== 1) reject();
    }
  }
  function beginWarning(value) {
    if (pendingWarning) { reject(); pendingWarning = null; }
    const token = dispatch;
    const eligible = enabled && !closed && setup && mountedCases.length === 0 && listenersIntact()
      && token !== null && !token.observed && token.error === value && accepted[token.category] === 0
      && diagnosticWarningShape(value, token.category);
    if (token) token.observed = true;
    pendingWarning = { value, token, eligible, calls: 0, withheld: 0, failedCalls: 0 };
  }
  function interceptedError(...args) {
    const current = pendingWarning;
    if (current) {
      current.calls++;
      const expected = current.token ? `(node:${process.pid}) ExperimentalWarning: ${diagnosticMessages[current.token.category]}` : null;
      const hint = '\n(Use `node --trace-warnings ...` to show where the warning was created)';
      if (current.eligible && current.calls === 1 && dispatch === current.token && listenersIntact()
          && args.length === 1 && typeof args[0] === 'string' && (args[0] === expected || args[0] === expected + hint)) {
        current.withheld++;
        return;
      }
      current.failedCalls++;
    }
    semanticFailure('console', args);
  }
  function endWarning(value) {
    const current = pendingWarning;
    pendingWarning = null;
    if (current && current.value === value && current.eligible && dispatch === current.token && current.calls === 1
        && current.withheld === 1 && current.failedCalls === 0 && listenersIntact()) {
      accepted[current.token.category]++;
      current.token.validated = true;
    } else {
      failures++;
      // Count every withheld call if the pair fails; also count unforwarded
      // events (including --no-warnings). Ordinary console calls already count.
      const missing = current?.withheld || (current?.failedCalls ? 0 : 1);
      for (let index = 0; index < missing; index++) semanticFailure('node-warning', []);
    }
  }
  process.prependListener('warning', beginWarning);
  process.on('warning', endWarning);
  console.error = interceptedError;
  return {
    // These are the actual captured builtins, with original source/options and
    // results. No replacement warning object, parsed program or VM is supplied.
    strip(source, options) {
      if (typeof source !== 'string' || !diagnosticDataOptions(options, ['mode'])) { reject(); return diagnosticStrip(source, options); }
      return provider('node-type-stripping', () => diagnosticStrip(source, options));
    },
    module(source, options) {
      if (typeof source !== 'string' || !diagnosticDataOptions(options, ['context', 'identifier', 'importModuleDynamically'])) { reject(); return new diagnosticSourceModule(source, options); }
      return provider('node-vm-modules', () => new diagnosticSourceModule(source, options));
    },
    settle() { return queued.size ? new Promise(resolve => drains.add(resolve)) : Promise.resolve(); },
    finishSetup() {
      if (!setup || closed || pendingWarning || queued.size || dispatch || !listenersIntact()
          || enabled && Object.values(accepted).some(count => count !== 1)) reject();
      setup = false;
    },
    close() {
      if (closed) return;
      // Keep ownership until the already-scheduled callbacks have completed.
      if (queued.size || dispatch) { reject(); return; }
      if (pendingWarning) {
        const missing = pendingWarning.withheld || (pendingWarning.failedCalls ? 0 : 1);
        failures++;
        for (let index = 0; index < missing; index++) semanticFailure('node-warning', []);
        pendingWarning = null;
      }
      if (queued.size || dispatch || console.error !== interceptedError || !listenersIntact()
          || process.emitWarning !== diagnosticEmitWarning || process.nextTick !== diagnosticNextTick || process.emit !== diagnosticEmit) reject();
      process.removeListener('warning', beginWarning);
      process.removeListener('warning', endWarning);
      if (console.error === interceptedError) console.error = previousError;
      const remaining = diagnosticApply(diagnosticRawListeners, process, ['warning']);
      cleanup = remaining.length === diagnosticNativeListeners.length && diagnosticNativeListeners.every((listener, index) => remaining[index] === listener)
        && console.error === previousError && queued.size === 0 && dispatch === null;
      if (!cleanup) reject();
      closed = true;
    },
    ready: () => closed && cleanup && !setup && failures === 0 && (!enabled || scheduled === 2 && completed === 2 && Object.values(accepted).every(count => count === 1)),
    result: () => ({ accepted: { ...accepted }, provider_scheduled: scheduled, provider_completed: completed, failures, setup_closed: !setup, listener_cleanup: cleanup })
  };
}
// END MOUNTED NODE DIAGNOSTICS

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const mapped = mountedSources(root);
const { createFrontendOverlays } = await mapped.load('platform-integrations/frontend-overlays.mjs');
const { createReaderOverlays } = await mapped.load('platform-integrations/frontend-reader-overlays.mjs');
const { createLocalLibraryOverlays } = await mapped.load('platform-integrations/local-library/overlays.mjs');
const { createPublicApiOverlays } = await mapped.load('platform-integrations/public-api/overlays.mjs');
const { createPublicUploadOverlays } = await mapped.load('platform-integrations/public-api/upload-overlays.mjs');
const { createBoundedReaderOverlays } = await mapped.load('platform-integrations/bounded-reader/overlays.mjs');
const pending = new Map(); let sequence = 0, fixtureSequence = 0, active = true, window, generation, library, runtime;
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
const lines = readline.createInterface({ input: process.stdin });
let initialize;
const initialization = new Promise(resolve => initialize = resolve);
lines.on('line', line => {
  const data = JSON.parse(line);
  if (data.type === 'init') initialize(data);
  else if (data.type === 'fixture-ack') { const item = pending.get('fixture:' + data.id); if (!item) throw Error('Unexpected fixture acknowledgement'); pending.delete('fixture:' + data.id); item.resolve(); }
  else if (data.type === 'response') {
    const item = pending.get(data.id); if (!item) return;
    pending.delete(data.id); item.cleanup();
    data.error ? item.reject(Error('Shared transport refused')) : item.resolve(data);
  } else throw Error('Unexpected coordinator message');
});
lines.on('close', () => { active = false; for (const item of pending.values()) item.reject(Error('Coordinator closed')); });
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sources = new Map();
const sourceStatus = () => ({ ...(process.env.WEEBDEX_BOUNDED_MOUNTED_FIXTURE === '1' ? { source_manifest_sha256: process.env.WEEBDEX_MOUNTED_SOURCE_MANIFEST_SHA256, loaded_source_names: [...sources.keys()] } : { source_hashes: Object.fromEntries(sources) }), source_unchanged: [...sources].every(([name, digest]) => hash(mapped.read(name)) === digest) });
function read(file) { const relative = mapped.relative(file), bytes = mapped.read(relative); sources.set(relative, hash(bytes)); return bytes.toString(); }
const uuid = value => '00000000-0000-8000-8000-' + String(value).padStart(12, '0');
const counts = { logical_actions_offered: 0, logical_actions_started: 0, logical_actions_completed: 0,
  page_positions_selected: 0, progress_save_attempts: 0, progress_saves_completed: 0, progress_failures: 0,
  semantic_failures: 0, media_failures: 0, image_bodies_verified: 0 };
const actions = [], progress = [], mountedCases = [], navigationDispatches = [], generated = new Map(), ownedWork = new Set();
const tracked = operation => async (...args) => { const token = {}; ownedWork.add(token); try { return await operation(...args); } finally { ownedWork.delete(token); } };
let currentAction;
const semanticObservations = [];
let semanticObservationOverflow = 0;
function semanticFailure(channel, args) {
  counts.semantic_failures++; // Unknown categories and overflow are still failures.
  if (semanticObservations.length >= 8) { semanticObservationOverflow++; return; }
  const observed = mountedCases.at(-1);
  const phase = observed?.outcome === 'started' && BOUNDED_MOUNTED_CASES.includes(observed.name) ? observed.name : 'setup-or-between-cases';
  let evidence = '';
  // Local inspection only: at most four arguments and 4,096 code units in total. Never
  // invoke argument getters or export arbitrary error text, stack, URL or body.
  for (const value of args.slice(0, 4)) {
    if (evidence.length >= 4096) break;
    try {
      const text = typeof value === 'string' ? value : value && typeof value === 'object' && !diagnosticIsProxy(value) ? Object.getOwnPropertyDescriptor(value, 'stack')?.value : undefined;
      if (typeof text === 'string') evidence += text.slice(0, 4096 - evidence.length);
    } catch {} // Uninspectable input stays unknown, not successful.
  }
  const category = evidence.includes('effect_update_depth_exceeded') ? 'svelte-effect-depth'
    : evidence.includes('Not implemented:') ? 'jsdom-not-implemented'
    : ['ReferenceError', 'TypeError', 'RangeError', 'SyntaxError'].find(name => evidence.startsWith(name)) ?? (evidence.startsWith('Error:') ? 'other-error' : 'unknown');
  const inspected = evidence + diagnosticLocalStack();
  const sites = [['reader-analytics', '/Analytics.mjs:'], ['reader-provider', '/Provider.mjs:'], ['reader-queue', '/Queue.mjs:'], ['reader-history', '/History.mjs:'], ['reader-context', '/context.mjs:'], ['bounded-runtime', '/bounded-runtime.mjs:'], ['mutation-feedback', '/mutation-feedback.ts:'], ['svelte-runtime', '/svelte/src/'], ['jsdom-runtime', '/jsdom/lib/']];
  const callsite = sites.find(([, marker]) => inspected.includes(marker))?.[0] ?? 'unknown';
  // Phase is observation time, not attribution of an asynchronous origin.
  semanticObservations.push({ observed_phase: phase, channel, category, callsite });
}
const fail = (...args) => semanticFailure('toast', args);
// Counts and fixed categories only. Zero semantic errors remains mandatory.
const previousConsole = { info: console.info, warn: console.warn };
const nodeDiagnostics = createMountedNodeDiagnostics(true);
console.info = () => {}; console.warn = () => {};
let finalResult;

try {
  const { origin, config, generation_dir } = await initialization;
  if (typeof generation_dir !== 'string' || !path.isAbsolute(generation_dir) || !path.basename(generation_dir).startsWith('weebdex-mounted-load-')) throw Error('Coordinator-owned temporary directory required');
  generation = generation_dir;
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(origin)) throw Error('Owned origin required');
  async function fetchOwned(input, options = {}) {
    if (!active) throw Error('Run stopped');
    if (config.bounded_controls && pending.size >= 4) throw Error('Mounted request concurrency cap');
    const url = new URL(input, origin);
    if (url.origin !== origin || url.username || url.password || url.hash) { send({type:'fatal'}); throw Error('Fixture escape'); }
    if (options.signal?.aborted) throw Error('Request aborted');
    const id = ++sequence;
    if (currentAction) currentAction.http_calls++;
    const result = await new Promise((resolve, reject) => {
      const abort = () => { pending.delete(id); send({ type: 'cancel', id }); reject(Error('Request aborted')); };
      pending.set(id, { resolve, reject, cleanup: () => options.signal?.removeEventListener('abort', abort) });
      options.signal?.addEventListener('abort', abort, { once: true });
      send({ type: 'request', id, url: url.href, method: options.method ?? 'GET', body: Buffer.from(options.body ?? '').toString('base64') });
    });
    const response = new Response(result.status === 204 ? null : Buffer.from(result.body, 'base64'), { status: result.status, headers: result.headers });
    return response;
  }
  const deps = process.env.WEEBDEX_LOAD_NODE_MODULES;
  if (!deps || !path.isAbsolute(deps)) throw Error('Explicit pinned dependencies required');
  const domDeps = config.bounded_controls ? process.env.WEEBDEX_LOAD_DOM_NODE_MODULES : deps;
  if (!domDeps || !path.isAbsolute(domDeps)) throw Error('Explicit pinned DOM dependencies required');
  const require = createRequire(path.join(deps, '../package.json'));
  const pins = { svelte: '5.55.1', jsdom: '27.0.1', 'fake-indexeddb': '6.2.4', typescript: '5.9.3' };
  for (const [name, version] of Object.entries(pins)) if (JSON.parse(fs.readFileSync(path.join(['jsdom', 'fake-indexeddb'].includes(name) ? domDeps : deps, name, 'package.json'))).version !== version) throw Error('Dependency version mismatch');
  const { JSDOM } = require(path.join(domDeps, 'jsdom'));
  ({ window } = new JSDOM('<main></main>', { url: origin + '/chapter/' + uuid(100) + '/1', pretendToBeVisual: true }));
  for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLMediaElement', 'HTMLInputElement', 'HTMLFormElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle', 'sessionStorage']) globalThis[key] = window[key];
  globalThis.fetch = fetchOwned;
  const { compile, compileModule } = require(path.join(deps, 'svelte/compiler'));
  const ts = require(path.join(deps, 'typescript'));
  const svelteUrl = pathToFileURL(path.join(deps, 'svelte/src/index-client.js')).href;
  runtime = await import(svelteUrl);
  const stores = await import(pathToFileURL(path.join(deps, 'svelte/src/store/index-client.js')));
  const { writable, get } = stores;
  const fake = await import(pathToFileURL(path.join(domDeps, 'fake-indexeddb/build/esm/index.js')));
  const indexedDB = new fake.IDBFactory();
  const Session = Object.assign(writable(null), { isValid: async () => false });
  class HTTPError extends Error { constructor(status, error, title) { super(error); Object.assign(this, { status, error, title }); } }
  const context = vm.createContext({ console, window, document, URL, URLSearchParams, AbortController, AbortSignal,
    TextEncoder, TextDecoder, Uint8Array, ArrayBuffer, structuredClone, Date, crypto, indexedDB,
    IDBKeyRange: fake.IDBKeyRange, navigator: {}, fetch: fetchOwned, setTimeout, clearTimeout, btoa, atob });
  const modules = new Map();
  const native = (key, values) => {
    if (!modules.has(key)) modules.set(key, new vm.SyntheticModule(Object.keys(values), function () { for (const [k, value] of Object.entries(values)) this.setExport(k, value); }, { context }));
    return modules.get(key);
  };
  function moduleFor(file) {
    if (modules.has(file)) return modules.get(file);
    const source = read(file);
    const module = nodeDiagnostics.module(file.endsWith('.ts') ? nodeDiagnostics.strip(source, { mode: 'transform' }) : source,
      { context, identifier: file, importModuleDynamically: async (specifier, importer) => { const child = resolve(specifier, importer); await evaluate(child); return child; } });
    modules.set(file, module); return module;
  }
  function resolve(specifier, importer) {
    if (specifier === '$app/navigation') return native(specifier, { goto() { throw Error('Navigation outside scenario'); }, beforeNavigate() { throw Error('Guard not mounted in seam'); } });
    if (specifier === 'svelte/store') return native(specifier, stores);
    if (specifier === 'svelte') return native(specifier, runtime);
    if (specifier === '$lib/stores/Session') return native(specifier, { default: Session });
    if (specifier === '$lib/HTTP') return native(specifier, { HTTPError });
    if (specifier === '$env/static/public') return native(specifier, { PUBLIC_API_URL: '/api', PUBLIC_MANGADEX_CATALOGUE: '1' });
    if (specifier === 'svelte-sonner') return native(specifier, { toast: { error: fail } });
    // Hash adapter only. No signature or publication operation is implemented here.
    if (specifier === '@noble/hashes/sha2.js') return native(specifier, { sha256: data => new Uint8Array(createHash('sha256').update(data).digest()) });
    if (specifier === '@noble/curves/secp256k1.js') return native(specifier, { schnorr: new Proxy({}, { get() { throw Error('Public sync not admitted'); } }) });
    let file = specifier.startsWith('$platform/') ? path.join(root, 'platform-integrations/frontend-behavior', specifier.slice(10))
      : specifier === '$lib/constants/Error' ? path.join(root, 'frontend/src/lib/constants/Error.ts')
      : specifier.startsWith('.') ? path.resolve(path.dirname(importer.identifier), specifier) : null;
    if (!file) throw Error('Unreviewed module dependency');
    if (!path.extname(file)) file += '.ts';
    return moduleFor(file);
  }
  async function evaluate(module) {
    if (module.status === 'unlinked') await module.link(resolve);
    if (module.status !== 'evaluated') await module.evaluate();
    return module.namespace;
  }
  const ns = relative => evaluate(moduleFor(path.join(root, relative)));
  // Common leaves first: avoid concurrent sibling-link graph overlap.
  for (const name of ['validation', 'query', 'normalize', 'client']) await ns('platform-integrations/public-api/' + name + '.ts');
  const site = await ns('platform-integrations/public-api/site-client.ts');
  library = await ns('platform-integrations/local-library/library.ts');
  const actionOwner = await ns('platform-integrations/local-library/action-owner.ts');
  const authority = await ns('platform-integrations/frontend-behavior/mutation-authority.ts');
  const feedback = await ns('platform-integrations/frontend-behavior/mutation-feedback.ts');
  const navigation = await ns('platform-integrations/frontend-behavior/reader-navigation.ts');
  const safeRead = await ns('platform-integrations/frontend-behavior/safe-read-owner.ts');
  const media = await ns('platform-integrations/public-api/media.ts');
  const boundedContract = await ns('platform-integrations/bounded-reader/contract.ts');
  const boundedController = await ns('platform-integrations/bounded-reader/controller.ts');
  const boundedClient = await ns('platform-integrations/bounded-reader/client.ts');
  const boundedEndpoint = await ns('platform-integrations/bounded-reader/endpoint.ts');
  const boundedTitleQuery = await ns('platform-integrations/bounded-reader/title-query.ts');
  const boundedHistory = await ns('platform-integrations/bounded-reader/history.ts');
  const boundedNativeHistory = await ns('platform-integrations/bounded-reader/native-history.ts');
  const boundedNativeContinue = await ns('platform-integrations/bounded-reader/native-continue.ts');
  const boundedPresentation = await ns('platform-integrations/bounded-reader/presentation.ts');
  const navigationAuthority = await ns('platform-integrations/frontend-behavior/navigation-authority.ts');
  const frontend = mapped.frontend;
  const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled: true }), createPublicUploadOverlays(frontend, { enabled: true }), createBoundedReaderOverlays(frontend)];
  const composed = relative => { const file = path.join(frontend, relative); let code = read(file); for (const plugin of plugins) code = plugin.transform.handler(code, file)?.code ?? code; return code; };
  // Source inventory includes every overlay/anchor, not only its transformed inputs.
  for (const relative of ['platform-integrations/frontend-overlays.mjs', 'platform-integrations/frontend-reader-overlays.mjs',
    'platform-integrations/local-library/overlays.mjs', 'platform-integrations/public-api/overlays.mjs', 'platform-integrations/public-api/anchors.json', 'platform-integrations/public-api/upload-overlays.mjs', 'platform-integrations/public-api/upload-anchors.json', 'platform-integrations/bounded-reader/overlays.mjs', 'platform-integrations/bounded-reader/anchors.json']) read(path.join(root, relative));
  fs.symlinkSync(deps, path.join(generation, 'node_modules'));
  const env = globalThis.__mountedLoad = { ...stores, ...actionOwner, ...authority, ...feedback, ...safeRead, ...media, ...navigation, ...boundedContract, ...boundedController, ...boundedClient, ...boundedEndpoint, ...boundedTitleQuery, ...boundedHistory, ...boundedNativeHistory, ...boundedNativeContinue, ...boundedPresentation, ...navigationAuthority,
    localLibraryState: library.localLibraryState, readerLibrarySource: library.readerLibrarySource, readerLibrarySelection: library.readerLibrarySelection, publicUuidLibraryEnabled: true, canSaveLocalProgress: library.canSaveLocalProgress, message: library.message,
    saveProgress: async (...args) => { const began = performance.now(); counts.progress_save_attempts++; const row = {outcome:'started'}; progress.push(row); try { await library.saveProgress(...args); counts.progress_saves_completed++; row.outcome = 'committed'; } catch (error) { counts.progress_failures++; row.outcome = 'failed'; throw error; } finally { row.elapsed_ms = performance.now() - began; } },
    Session, HTTPError, HTTP: class { constructor() { throw Error('Native transport forbidden'); } },
    publicCatalogueRead: site.publicCatalogueRead, blockNativeCatalogueWrite: site.blockNativeCatalogueWrite,
    PUBLIC_API_URL: '/api', StatusCodes: { OK: 200 }, toast: { error: fail },
    Constants: { Tag: { Group: { Format: 'format' } }, Reader: { Layout: {}, Direction: { LTR: 'ltr', RTL: 'rtl' } } }, Utils: { isBot: () => false },
    // Stable fixture preference seam; actual provider language/query translation remains intact.
    prefs: Object.assign(writable({}), { applyChapterPreferences() {} }),
    dataSaver: writable(false), skipEndOfChapterPage: writable(false),
    page: { params: { page: '1' }, state: {}, url: new URL(window.location.href) },
    replaceState: (url, state) => { window.history.replaceState(state, '', url); env.page.state = state; },
    resolve: value => value, goto: async target => { if (!config.bounded_controls || !/^\/chapter\/[0-9a-f-]{36}\/1$/.test(target)) throw Error('Cross-chapter navigation not admitted'); navigationDispatches.push(target); } };
  env.createReaderClient = (...args) => { const client = boundedClient.createReaderClient(...args); return { read: tracked(client.read), verifyIdentity: tracked(client.verifyIdentity) }; };
  env.localHistoryAccess = (...args) => { const access = boundedHistory.localHistoryAccess(...args); return { ...access, completed: tracked(access.completed) }; };
  const preferences = writable({ offset: 0, preload: 0, parallel: 2, layout: 'single', direction: 'ltr' });
  env.mangaPrefs = env.webtoonPrefs = preferences;
  function rewrite(script) {
    const ast = ts.createSourceFile('source.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    for (const item of [...ast.statements].reverse()) {
      if (!ts.isImportDeclaration(item)) continue;
      const specifier = item.moduleSpecifier.text, clause = item.importClause;
      let replacement = '';
      if (specifier === 'svelte') replacement = item.getText(ast).replace(/(['"])svelte\1/, JSON.stringify(svelteUrl));
      else if (clause && !clause.isTypeOnly) {
        const names = [];
        if (clause.name) names.push(clause.name.text);
        if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) names.push(clause.namedBindings.name.text);
        else if (clause.namedBindings) for (const row of clause.namedBindings.elements) if (!row.isTypeOnly) names.push(row.name.text);
        if (names.length) replacement = `const { ${names.join(', ')} } = globalThis.__mountedLoad;`;
      }
      script = script.slice(0, item.getStart(ast)) + replacement + script.slice(item.end);
    }
    return script;
  }
  const generatedNames = new Set(['Mutex', 'bounded-runtime', 'context', 'Chapter', 'AnalyticsAPI', 'Empty', 'Queue', 'History', 'Analytics', 'Provider', 'Harness', 'Status', 'GroupPage', 'EditionPage', 'ChapterSelector', 'EditionSelector', 'TitleReadingActions', 'ControlHarness']);
  function writeGenerated(file, value) {
    const name = path.basename(file, '.mjs'), bytes = Buffer.byteLength(value);
    assert(path.dirname(file) === generation && file.endsWith('.mjs') && generatedNames.has(name) && !generated.has(name), 'Unexpected generated module');
    assert([...generated.values()].reduce((sum, row) => sum + row.bytes, 0) + bytes <= 160 * 1024, 'Generated content exceeds 160 KiB before write');
    generated.set(name, { bytes, sha256: hash(Buffer.from(value)) }); fs.writeFileSync(file, value);
  }
  async function component(name, source, imports = true) {
    if (imports) source = source.replace(/<script([^>]*)>([\s\S]*?)<\/script>/g, (_, attrs, script) => '<script' + attrs + '>' + rewrite(script) + '</script>');
    const file = path.join(generation, name + '.mjs');
    writeGenerated(file, compile(source, { filename: name + '.svelte', generate: 'client', runes: true }).js.code);
    return (await import(pathToFileURL(file))).default;
  }
  const mutexFile = path.join(generation, 'Mutex.mjs');
  writeGenerated(mutexFile, nodeDiagnostics.strip(rewrite(composed('src/lib/Mutex.ts')), { mode: 'transform' }));
  env.Mutex = (await import(pathToFileURL(mutexFile))).default;
  const boundedRuntimeFile = path.join(generation, 'bounded-runtime.mjs');
  writeGenerated(boundedRuntimeFile, compileModule(nodeDiagnostics.strip(rewrite(read(path.join(root, 'platform-integrations/bounded-reader/runtime.svelte.ts'))), { mode: 'transform' }), { filename: 'bounded-runtime.svelte.js', generate: 'client' }).js.code);
  Object.assign(env, await import(pathToFileURL(boundedRuntimeFile))); 
  const contextFile = path.join(generation, 'context.mjs');
  writeGenerated(contextFile, compileModule(nodeDiagnostics.strip(rewrite(composed('src/lib/components/reader/context.svelte.ts')), { mode: 'transform' }), { filename: 'context.svelte.js', generate: 'client' }).js.code);
  const readerContext = await import(pathToFileURL(contextFile));
  env.setReader = props => { const reader = readerContext.setReader(props); env.reader = reader; return reader; };
  env.useReader = readerContext.useReader;
  const chapterFile = path.join(generation, 'Chapter.mjs');
  writeGenerated(chapterFile, nodeDiagnostics.strip(rewrite(composed('src/lib/API/Chapter.ts')), { mode: 'transform' }));
  const analyticsFile = path.join(generation, 'AnalyticsAPI.mjs');
  writeGenerated(analyticsFile, nodeDiagnostics.strip(rewrite(composed('src/lib/API/Analytics.ts')), { mode: 'transform' }));
  env.API = { Chapter: await import(pathToFileURL(chapterFile)), Analytics: await import(pathToFileURL(analyticsFile)), ChapterHistory: { UpdateBatch() { throw Error('Progress must never become HTTP'); } } };
  const EMPTY = await component('Empty', '<span></span>', false);
  env.Keyboard = env.Scroll = EMPTY; // Explicit geometry/input exclusions, not simulated browser behavior.
  class OwnedXHR extends window.EventTarget {
    controller = new AbortController(); status = 0; response = null; responseType = '';
    open(method, url) { this.method = method; this.url = url; }
    getResponseHeader(name) { return this.headers?.get(name); }
    abort() { this.controller.abort(); this.onabort?.(); }
    send() { void tracked(async () => {
      try {
        const response = await fetchOwned(this.url, { method: this.method, signal: this.controller.signal });
        this.status = response.status; this.headers = response.headers;
        this.response = await response.blob();
        if (response.ok) {
          const bytes = Buffer.from(await this.response.arrayBuffer());
          if (hash(bytes) !== '431ced6916a2a21a156e38701afe55bbd7f88969fbbfc56d7fe099d47f265460') { send({type:'fatal'}); throw Error('Synthetic image integrity'); }
          counts.image_bodies_verified++;
        }
        await this.onload?.();
      } catch { if (!this.controller.signal.aborted) { counts.media_failures++; this.onerror?.(); } }
    })(); }
  }
  globalThis.XMLHttpRequest = OwnedXHR;
  globalThis.Image = class { async decode() {} }; // Digest-verified fixture bytes only; no browser decoder claim.
  env.Queue = await component('Queue', composed('src/lib/components/reader/reader-queue.svelte'));
  env.History = await component('History', composed('src/lib/components/reader/reader-history.svelte'));
  env.Analytics = await component('Analytics', composed('src/lib/components/reader/reader-analytics.svelte'));
  env.Provider = await component('Provider', composed('src/lib/components/reader/reader-provider.svelte'));
  const Harness = await component('Harness', '<script>const { Provider } = globalThis.__mountedLoad; let {chapter} = $props();</script><Provider {chapter}><p>Mounted synthetic reader</p></Provider>', false);
  const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
  async function until(predicate) { for (let i = 0; i < 1000; i++) { runtime.flushSync(); if (predicate()) return; await sleep(5); } throw Error('Mounted condition did not settle'); }
  await library.openLibrary(); await library.verifyCatalogueScope();
  if (get(library.localLibraryState).scopeStatus !== 'verified') throw Error('Synthetic scope did not verify');
  nodeDiagnostics.finishSetup();
  counts.logical_actions_offered = config.actions.length;
  for (const specification of config.actions) {
    const mountedCase = config.bounded_controls ? { name: actions.length ? 'partial remount progress' : 'complete progress', outcome: 'started', assertions: [] } : undefined; if (mountedCase) mountedCases.push(mountedCase);
    currentAction = { outcome: 'started', started_ms: performance.now(), http_calls: 0, progress_saves_completed: 0, page_positions_selected: 0 };
    actions.push(currentAction); counts.logical_actions_started++;
    const before = counts.progress_saves_completed;
    preferences.set({ ...get(preferences), preload: specification.preload });
    env.page.params.page = '1'; env.page.state = {};
    window.history.replaceState({}, '', '/chapter/' + uuid(specification.chapter) + '/1');
    const response = await env.API.Chapter.Get(uuid(specification.chapter));
    if (response?.status !== 200 || response.body?.id !== uuid(specification.chapter)) throw Error('Chapter detail failed');
    const host = document.createElement('main'); document.body.append(host);
    const instance = runtime.mount(Harness, { target: host, props: { chapter: response.body } });
    try {
      await until(() => env.reader?.navigationState?.current?.id === env.reader.chapter.id && !env.reader.navigationState.pending && env.reader.currentPage?.isLoaded);
      if (config.selector_pages) {
        currentAction.selector_pages = [];
        for (let index = 0; index < config.selector_pages; index++) {
          if (index) assert.equal(env.reader.navigation.snapshot.groupsMore, true, 'Expected another bounded selector page');
          await env.reader.navigation.groups(index > 0);
          await until(() => !env.reader.navigation.snapshot.pending && !pending.size && !ownedWork.size);
          const view = env.reader.navigation.snapshot;
          assert.equal(view.status, 'ready', 'Actual mounted selector must validate the bounded response');
          currentAction.selector_pages.push({ ids: view.groups.map(row => row.id), more: view.groupsMore });
        }
      }
      for (const position of specification.page_turns) {
        env.reader.currentPage = env.reader.pages[position];
        counts.page_positions_selected++; currentAction.page_positions_selected++;
        runtime.flushSync();
      }
      await sleep(specification.dwell_ms);
      await until(() => !pending.size && env.reader.currentPage.isLoaded);
      const snapshot = await library.currentDeviceSnapshot();
      currentAction.saved_page_before_unmount = snapshot.records.find(row => row.family === 'chapters' && row.id === uuid(specification.chapter))?.page ?? null;
    } finally { await runtime.unmount(instance); host.remove(); }
    await until(() => !pending.size && !ownedWork.size && counts.progress_save_attempts === counts.progress_saves_completed + counts.progress_failures);
    currentAction.progress_saves_completed = counts.progress_saves_completed - before;
    const final = await library.currentDeviceSnapshot();
    currentAction.saved_page_after_unmount = final.records.find(row => row.family === 'chapters' && row.id === uuid(specification.chapter))?.page ?? null;
    const expected = (specification.page_turns.at(-1) ?? 0) + 1;
    if (currentAction.saved_page_after_unmount !== expected) throw Error('Actual local progress did not settle');
    currentAction.elapsed_ms = performance.now() - currentAction.started_ms; delete currentAction.started_ms;
    currentAction.outcome = 'completed'; counts.logical_actions_completed++;
    if (mountedCase) { const record = final.records.find(row => row.family === 'chapters' && row.id === uuid(specification.chapter)); assert.equal(record?.completed, actions.length === 1); assert.equal(pending.size, 0); assert.equal(counts.progress_save_attempts, counts.progress_saves_completed); mountedCase.assertions.push('actual saved page and completion flag', 'unmounted with requests and progress settled'); mountedCase.outcome = 'passed'; }
  }
  currentAction = null;
  if (config.bounded_controls) await runBoundedMountedCases({ env, runtime, settled: () => pending.size === 0 && ownedWork.size === 0 && counts.progress_save_attempts === counts.progress_saves_completed + counts.progress_failures, component, read: relative => read(path.join(root, relative)), library, until, counts, pending, mountedCases, navigationDispatches, uuid, window, phase: async phase => { const id = ++fixtureSequence; await new Promise((resolve, reject) => { pending.set('fixture:' + id, { resolve, reject, cleanup() {} }); send({ type: 'fixture-phase', id, phase }); }); } });
  if (config.bounded_controls) assert.equal(generated.size, 18, 'Exact generated module count');
  assert([...generated].every(([name, row]) => hash(fs.readFileSync(path.join(generation, name + '.mjs'))) === row.sha256), 'Generated module changed');
  assert.equal(ownedWork.size, 0, 'Actual transport/history/media work must settle');
  mapped.verify();
  const sourceUnchanged = [...sources].every(([name, digest]) => hash(mapped.read(name)) === digest);
  const device = await library.stores(); device.device.db.close(); device.device.channel?.close(); device.protocol.db.close();
  window.close(); window = null;
  finalResult = { passed: sourceUnchanged && counts.semantic_failures === 0 && counts.progress_failures === 0 && counts.media_failures === 0,
    ...counts, semantic_observations: semanticObservations, semantic_observation_overflow: semanticObservationOverflow, http_attempts: sequence, actions, progress, mounted_cases: mountedCases, navigation_dispatches: navigationDispatches, generated_modules: Object.fromEntries(generated), ...sourceStatus(),
    runtime_versions: pins, runtime: 'JSDOM + actual Svelte components + fake-indexeddb',
    default_progress_http_writes: 0, image_decode_measured: false, hash_adapter: 'Node SHA-256, no signing', real_browser_measured: false, public_sync_executed: false, capacity_qualified: false };
  lines.close(); process.stdin.destroy();
} catch (error) {
  for (const row of mountedCases) if (row.outcome === 'started') row.outcome = 'failed';
  process.stderr.write(String(error.stack) + '\n');
  finalResult = { passed: false, failure: 'mounted-scenario-failed', ...counts, semantic_observations: semanticObservations, semantic_observation_overflow: semanticObservationOverflow, http_attempts: sequence, actions, progress, mounted_cases: mountedCases, ...sourceStatus(), capacity_qualified: false, real_browser_measured: false };
  process.exitCode = 1; lines.close(); process.stdin.destroy();
} finally {
  active = false;
  const cleanupFailed = () => {
    finalResult = { ...finalResult, passed: false, failure: finalResult?.failure ?? 'mounted-cleanup-failed' };
    process.exitCode = 1;
  };
  try { window?.close(); } catch { cleanupFailed(); }
  if (library) {
    try { const owned = await library.stores(); owned.device.db.close(); owned.device.channel?.close(); owned.protocol.db.close(); }
    catch { cleanupFailed(); }
  }
  try { await nodeDiagnostics.settle(); nodeDiagnostics.close(); } catch { cleanupFailed(); }
  try { console.info = previousConsole.info; console.warn = previousConsole.warn; } catch { cleanupFailed(); }
}
// Final counts include cleanup-time errors. No provisional success is emitted.
send({ type: 'result', result: { ...finalResult, ...counts,
  semantic_observations: semanticObservations, semantic_observation_overflow: semanticObservationOverflow,
  node_diagnostics: nodeDiagnostics.result(),
  passed: finalResult?.passed === true && nodeDiagnostics.ready() && counts.semantic_failures === 0 && counts.progress_failures === 0 && counts.media_failures === 0 } });
