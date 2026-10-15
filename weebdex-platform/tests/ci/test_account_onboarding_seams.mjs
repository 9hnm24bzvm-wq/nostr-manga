#!/usr/bin/env node
// Finite production registration/public-control seams. Svelte lifecycle/store,
// auth transport and browser APIs are explicit stand-ins. No browser, services,
// signer, real storage or network starts; this is not browser accessibility QA.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const behavior = path.join(root, 'platform-integrations/frontend-behavior');
const registerRoute = 'src/routes/(app)/auth/register/+page.svelte';
const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)];
const compose = relative => {
  const file = path.join(frontend, relative); let source = fs.readFileSync(file, 'utf8');
  for (const plugin of plugins) source = plugin.transform.handler(source, file)?.code ?? source;
  return source;
};
const withoutImports = source => source.replace(/^\s*import[^;]+;\s*$/gm, '');
const transform = source => stripTypeScriptTypes(source, { mode: 'transform' });
const clone = value => structuredClone(value);
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
async function until(predicate) { for (let i = 0; i < 100 && !predicate(); i++) await Promise.resolve(); assert.ok(predicate(), 'finite microtask seam did not settle'); }
function writable(value) {
  const listeners = new Set();
  const store = { subscribe(fn) { listeners.add(fn); fn(value); return () => listeners.delete(fn); }, set(next) { value = next; for (const fn of [...listeners]) fn(value); }, update(fn) { store.set(fn(value)); } };
  return store;
}
function derived(stores, fn) {
  if (!Array.isArray(stores)) stores = [stores];
  return { subscribe(callback) { const values = [], ready = new Set(); const offs = stores.map((store, index) => store.subscribe(value => { values[index] = value; ready.add(index); if (ready.size === stores.length) callback(fn(values)); })); return () => offs.forEach(off => off()); } };
}
function get(store) { let value; const unsubscribe = store.subscribe(next => { value = next; }); unsubscribe(); return value; }
class AxiosError extends Error {
  constructor(data, status = 400) { super('Synthetic registration rejection'); this.status = status; this.response = { status, data }; }
}
class PublicError extends Error { constructor(title, body) { super(title); this.title = title; this.body = body; } }
const unknown = new PublicError('Something went wrong', 'Try again.');
const identity = { id: 'synthetic-new-account', username: 'ReaderExample', is_verified: false };
const success = () => ({ status: 200, data: { session: { active: true, identity: clone(identity) }, identity: clone(identity) } });
function registrationFlow(returnTo = '/library?view=grid') {
  const input = (name, value, type = 'text') => ({ attributes: { name, value, type, required: true }, messages: [] });
  return { id: 'synthetic-registration-flow', type: 'browser', return_to: returnTo, ui: { action: '/auth/self-service/registration?flow=synthetic-registration-flow', method: 'POST', messages: [], nodes: [input('csrf_token', 'synthetic-csrf', 'hidden'), input('traits.email', ''), input('traits.username', ''), input('password', '', 'password')] } };
}

async function registration({ route = registerRoute, signedIn = false, url = 'https://weebdex.example.invalid/auth/register?redirect=%2Flibrary%3Fview%3Dgrid', flow = registrationFlow(), mountResponse, response = success(), refreshError, gatedMount = false, invalidateOnGoto = true, gotoFailures = 0 } = {}) {
  const source = compose(route);
  const form = source.match(/<form\b[^>]*onsubmit=\{([^}]+)\}[^>]*bind:this=\{([^}]+)\}/);
  assert.ok(form, 'exercise the actual rendered registration form binding');
  const guide = source.match(/\{(?:#if|:else if)\s+([^}]+)\}\s*<AccountOnboarding\b([\s\S]*?)\/>/);
  if (route === registerRoute) assert.ok(guide, 'welcome must have an explicit registration-only render branch');
  const mounts = [], destroys = [], effects = [], preEffects = [], navigation = [], writes = [], reads = [], notices = [], listeners = new Map();
  const writeGate = deferred(), refreshGate = deferred(), mountGate = deferred();
  let location = new URL(url);
  const page = { url: new URL(location), state: {} };
  const history = [];
  const locationWindow = { get location() { return location; }, set location(value) { navigation.push(String(value)); location = new URL(value, location); page.url = new URL(location); },
    addEventListener(name, fn) { listeners.set(name, fn); }, removeEventListener(name, fn) { assert.equal(listeners.get(name), fn); listeners.delete(name); },
    requestAnimationFrame(fn) { queueMicrotask(fn); }, setTimeout, clearTimeout };
  const tripwire = name => () => assert.fail('Signup onboarding must not invoke ' + name);
  Object.defineProperty(locationWindow, 'nostr', { get: tripwire('a signer') });
  const storage = { getItem: tripwire('storage reads'), setItem: tripwire('storage writes'), removeItem: tripwire('storage removal') };
  const Session = Object.assign(writable(signedIn ? { identity: clone(identity) } : undefined), {
    async isValid() { reads.push('session'); return signedIn; },
    async initialize(force) { reads.push(['refresh', force]); await refreshGate.promise; if (refreshError) throw refreshError; Session.set({ identity: clone(identity), roles: [] }); },
  });
  const Kratos = {
    async createBrowserRegistrationFlow(options) { reads.push(['create', clone(options)]); return gatedMount ? mountGate.promise : clone(mountResponse ?? { status: 200, data: flow }); },
    async getRegistrationFlow(options) { reads.push(['get', clone(options)]); return gatedMount ? mountGate.promise : clone(mountResponse ?? { status: 200, data: flow }); },
    async createBrowserLoginFlow(options) { reads.push(['create-login', clone(options)]); return clone({ status: 200, data: flow }); },
    async createBrowserVerificationFlow(options) { reads.push(['create-verification', clone(options)]); return clone({ status: 200, data: flow }); },
    async updateRegistrationFlow(request) { writes.push(clone(request)); return writeGate.promise; },
  };
  class Query extends URLSearchParams { toString() { const value = super.toString(); return value ? '?' + value : ''; } }
  const context = vm.createContext({ console: { error() {} }, URL, URLSearchParams: Query, Error, Promise, structuredClone, setTimeout, clearTimeout, AbortController,
    window: locationWindow, location, page, Session, Kratos, AxiosError, writable, get, derived,
    ErrorSessionAlreadyAvailable: 'session_already_available', Constants: { Error: { Error: PublicError, Unknown: unknown } },
    StatusCodes: { OK: 200, BAD_REQUEST: 400, GONE: 410, UNAUTHORIZED: 401, FORBIDDEN: 403 },
    PUBLIC_SITE_NAME: 'WeebDex',
    $props: Object.assign(() => ({}), { id: () => 'synthetic-form-id' }),
    $state: Object.assign(value => value, { snapshot: clone }), $derived: Object.assign(value => value, { by: fn => fn() }),
    $effect: Object.assign(fn => effects.push(fn), { pre: fn => preEffects.push(fn) }),
    onMount: fn => mounts.push(fn), onDestroy: fn => destroys.push(fn),
    goto: async value => { navigation.push(String(value)); if (gotoFailures-- > 0) throw Error('Synthetic navigation failure'); if (invalidateOnGoto) vm.runInContext('invalidateNavigationEpoch()', context); },
    replaceState(value) { location = new URL(value, location); page.url = new URL(location); history.push(location.href); },
    toast: Object.fromEntries(['success', 'info', 'error'].map(level => [level, (...args) => notices.push([level, ...args])])),
    localStorage: storage, sessionStorage: storage, indexedDB: { open: tripwire('IndexedDB open'), deleteDatabase: tripwire('database removal') },
    fetch: tripwire('network publication'), navigator: { clipboard: { writeText: tripwire('key clipboard') } },
  });
  for (const file of ['navigation-authority.ts', 'auth-form-owner.ts']) vm.runInContext(transform(withoutImports(fs.readFileSync(path.join(behavior, file), 'utf8')).replace(/^export /gm, '')), context, { filename: file });
  context.Mutex = vm.runInContext(transform('(function(){' + withoutImports(compose('src/lib/Mutex.ts')).replace('export default Mutex;', 'return Mutex;') + '})()'), context);
  Object.defineProperty(context, '$Session', { get: () => get(Session) });
  // The actual public library store is observed across the auth transition.
  // Database/network methods remain tripwires and are never opened here.
  const librarySource = fs.readFileSync(path.join(root, 'platform-integrations/local-library/library.ts'), 'utf8');
  const library = vm.runInContext('(function(){' + transform(withoutImports(librarySource).replace(/^export /gm, '')) + '\nreturn { localLibraryState, libraryIdentity }; })()', context);
  // Import local, pure helpers used by this registration route. Unexpected
  // integrations are rejected rather than silently turned into no-ops.
  for (const match of source.matchAll(/import\s+\{([^}]+)\}\s+from\s+["']\$platform\/([^"']+)["'];/g)) {
    if (match[2] === 'auth-form-owner') continue;
    const file = path.join(behavior, match[2] + '.ts');
    assert.ok(fs.existsSync(file), 'review registration helper: ' + match[2]);
    vm.runInContext(transform(withoutImports(fs.readFileSync(file, 'utf8')).replace(/^export /gm, '')), context, { filename: file });
  }
  let script = source.match(/<script\b[^>]*>([\s\S]*?)<\/script>/)[1];
  const api = vm.runInContext('(function(){' + transform(withoutImports(script)) + '\nreturn { submit: ' + form[1] + ', bindForm: value => { ' + form[2] + ' = value; }, guide: () => (' + (guide?.[1] ?? 'false') + '), guideProps: () => ({' + [...(guide?.[2] ?? '').matchAll(/\b(\w+)=\{([^}]+)\}/g)].map(match => JSON.stringify(match[1]) + ': (' + match[2] + ')').join(',') + '}) }; })()', context);
  const fields = Object.fromEntries(Object.entries({ email: 'reader@example.invalid', username: 'ReaderExample', password: 'Synthetic!234', csrf_token: 'synthetic-csrf' }).map(([name, value]) => [name, { value }]));
  api.bindForm(Object.assign({ elements: { namedItem: name => fields[name] } }, fields));
  const runEffects = () => { preEffects.forEach(fn => fn()); effects.forEach(fn => fn()); };
  runEffects();
  const mounting = mounts[0]();
  if (!gatedMount) { await mounting; runEffects(); }
  const stale = kind => {
    if (kind === 'destroy') { destroys.forEach(fn => fn()); return; }
    vm.runInContext('invalidateNavigationEpoch()', context);
    if (kind === 'back') { location = new URL('/search', location); page.url = new URL(location); listeners.get('popstate')?.(); }
    preEffects.forEach(fn => fn());
  };
  return { source, api, library, fields, reads, writes, notices, navigation, history, Session, mounting, runEffects,
    submit: () => api.submit({ preventDefault() {} }),
    guideVisible: () => Boolean(api.guide()),
    releaseWrite() { if (response instanceof Error) writeGate.reject(response); else writeGate.resolve(clone(response)); },
    releaseRefresh() { refreshGate.resolve(); },
    releaseMount() { mountGate.resolve(clone(mountResponse ?? { status: 200, data: flow })); },
    stale,
    destroy() { destroys.forEach(fn => fn()); assert.equal(listeners.size, 0); },
  };
}

test('browsing, login and shared layouts never mount the account-creation guide', () => {
  const routes = [];
  function visit(directory) { for (const entry of fs.readdirSync(directory, { withFileTypes: true })) { const file = path.join(directory, entry.name); if (entry.isDirectory()) visit(file); else if (entry.name.endsWith('.svelte')) routes.push(path.relative(frontend, file)); } }
  visit(path.join(frontend, 'src/routes'));
  const consumers = routes.filter(route => /AccountOnboarding/.test(compose(route)));
  assert.deepEqual(consumers, [registerRoute]);
  assert.doesNotMatch(compose('src/routes/(app)/auth/login/+page.svelte'), /registrationWelcome|AccountOnboarding/);
  assert.doesNotMatch(compose(registerRoute), /\$protocol|\$local-library|connectSigner|signEvent|publishEvent|localStorage|sessionStorage/);
});

test('opening signup leaves the familiar form and does not show a welcome or write anything', async () => {
  const value = await registration();
  assert.equal(value.guideVisible(), false);
  assert.equal(value.writes.length, 0);
  assert.deepEqual(value.reads, ['session', ['create', { returnTo: '/library?view=grid' }]]);
  assert.deepEqual(value.navigation, []);
  value.destroy();
});

test('existing account visiting signup bypasses creation and the welcome', async () => {
  const value = await registration({ signedIn: true });
  assert.equal(value.guideVisible(), false);
  assert.deepEqual(value.reads, ['session']);
  assert.equal(value.writes.length, 0);
  assert.deepEqual(value.navigation, ['/library?view=grid']);
  value.destroy();
});

test('resuming an unsubmitted registration flow never counts as new-account success', async () => {
  const value = await registration({ url: 'https://weebdex.example.invalid/auth/register?flow=synthetic-registration-flow&redirect=%2Flibrary' });
  assert.equal(value.guideVisible(), false);
  assert.deepEqual(value.reads, ['session', ['get', { id: 'synthetic-registration-flow' }]]);
  assert.equal(value.writes.length, 0);
  value.destroy();
});

test('accepted new-account submission shows welcome only after authoritative refresh and Continue navigates once', async () => {
  const value = await registration({ invalidateOnGoto: false });
  const first = value.submit();
  assert.equal(value.writes.length, 1);
  await value.submit();
  assert.equal(value.writes.length, 1, 'double-submit cannot create a second account');
  assert.equal(value.guideVisible(), false);
  value.releaseWrite(); await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  assert.equal(value.guideVisible(), false, 'response alone must not open the guide');
  assert.deepEqual(value.navigation, []);
  value.releaseRefresh(); await first; value.runEffects();
  assert.equal(value.guideVisible(), true);
  assert.deepEqual(value.writes, [{ flow: 'synthetic-registration-flow', updateRegistrationFlowBody: { method: 'password', csrf_token: 'synthetic-csrf', password: 'Synthetic!234', traits: { email: 'reader@example.invalid', username: 'ReaderExample' } } }]);
  await value.submit(); assert.equal(value.writes.length, 1, 'welcome cannot replay the consumed signup form');
  const props = value.api.guideProps();
  const continueAction = Object.values(props).find(item => typeof item === 'function');
  assert.equal(typeof continueAction, 'function', 'guide receives its real primary control callback');
  await continueAction(); await continueAction();
  assert.deepEqual(value.navigation, ['/library?view=grid']);
  value.destroy();
});

for (const response of [new AxiosError({ error: { reason: 'That email or username is unavailable' } }), { status: 200, data: { error: { reason: 'Try another username' } } }, { status: 200, data: null }, { status: 202, data: success().data }, new AxiosError(success().data)]) test('unaccepted registration response never opens the guide: ' + (response instanceof Error ? response.message + ' ' + JSON.stringify(response.response.data) : JSON.stringify(response)), async () => {
  const value = await registration({ response });
  const pending = value.submit(); value.releaseWrite(); await pending; value.runEffects();
  assert.equal(value.guideVisible(), false);
  assert.equal(value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'), false);
  assert.deepEqual(value.navigation, []);
  assert.equal(value.writes.length, 1);
  value.destroy();
});

test('a same-flow validation error stays on the editable registration form', async () => {
  const flow = registrationFlow(); flow.ui.messages.push({ type: 'error', text: 'That email or username is unavailable' });
  const value = await registration({ response: new AxiosError(flow) });
  const pending = value.submit(); value.releaseWrite(); await pending; value.runEffects();
  assert.equal(value.guideVisible(), false);
  assert.ok(value.notices.some(row => row.includes('That email or username is unavailable')));
  assert.deepEqual(value.navigation, []);
  assert.equal(value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'), false);
  value.destroy();
});

for (const kind of ['back', 'navigation-start', 'destroy']) {
  test('leaving signup while its request is pending suppresses late welcome: ' + kind, async () => {
    const value = await registration(), pending = value.submit();
    value.stale(kind); value.releaseWrite(); await pending; value.runEffects();
    assert.equal(value.guideVisible(), false);
    assert.equal(value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'), false);
    assert.deepEqual(value.navigation, []);
    if (kind !== 'destroy') value.destroy();
  });
  test('leaving signup during session refresh suppresses late welcome: ' + kind, async () => {
    const value = await registration(), pending = value.submit();
    value.releaseWrite(); await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
    value.stale(kind); value.releaseRefresh(); await pending; value.runEffects();
    assert.equal(value.guideVisible(), false); assert.deepEqual(value.navigation, []);
    if (kind !== 'destroy') value.destroy();
  });
  test('a detached welcome Continue cannot override newer navigation: ' + kind, async () => {
    const value = await registration(), pending = value.submit(); value.releaseWrite();
    await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
    value.releaseRefresh(); await pending; value.runEffects();
    const continueAction = Object.values(value.api.guideProps()).find(item => typeof item === 'function');
    value.stale(kind); await continueAction();
    assert.deepEqual(value.navigation, []);
    if (kind !== 'destroy') value.destroy();
  });
}

test('failed session refresh never announces a newly usable account', async () => {
  const value = await registration({ refreshError: Error('Synthetic refresh failure') }), pending = value.submit();
  value.releaseWrite(); await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending; value.runEffects();
  assert.equal(value.guideVisible(), true, 'accepted creation remains consumed even if session read fails');
  assert.equal(value.api.guideProps().accountCurrent, false, 'show session-changed fallback, never account-ready controls');
  await value.api.guideProps().oncontinue(); await value.submit();
  assert.equal(value.writes.length, 1); assert.deepEqual(value.navigation, []);
  value.destroy();
});

for (const [label, requested, expected] of [
  ['local path', '/library?view=grid#reading', '/library?view=grid#reading'],
  ['same-origin absolute path', 'https://weebdex.example.invalid/search?title=hello', '/search?title=hello'],
  ['foreign origin', 'https://foreign.example.invalid/steal', '/'],
  ['protocol-relative foreign origin', '//foreign.example.invalid/steal', '/'],
  ['auth loop', '/auth/register?flow=consumed', '/'],
  ['login loop', '/auth/login', '/'],
  ['backslash authority', '/\\foreign.example.invalid', '/'],
]) test('signup and Continue preserve only safe browsing targets: ' + label, async () => {
  const value = await registration({ url: 'https://weebdex.example.invalid/auth/register?redirect=' + encodeURIComponent(requested), flow: registrationFlow(requested) });
  assert.deepEqual(value.reads[1], ['create', { returnTo: expected }]);
  const pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending;
  assert.equal(value.api.guideProps().returnTo, expected);
  await value.api.guideProps().oncontinue();
  assert.deepEqual(value.navigation, [expected]);
  value.destroy();
});

test('refreshing successful signup bypasses the ephemeral guide without replaying the consumed flow', async () => {
  const value = await registration(), pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending;
  const refreshUrl = value.history.at(-1); assert.ok(refreshUrl);
  const reloaded = await registration({ signedIn: true, url: refreshUrl });
  assert.equal(reloaded.guideVisible(), false);
  assert.deepEqual(reloaded.reads, ['session']);
  assert.deepEqual(reloaded.navigation, ['/library?view=grid']);
  assert.equal(reloaded.writes.length, 0);
  value.destroy(); reloaded.destroy();
});

test('logout or account switching changes welcome authority and disables the captured Continue', async () => {
  const value = await registration(), pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending;
  const continueAction = value.api.guideProps().oncontinue;
  for (const session of [undefined, { identity: { id: 'another-account' } }]) {
    value.Session.set(session);
    assert.equal(value.api.guideProps().accountCurrent, false);
    await continueAction();
    assert.deepEqual(value.navigation, []);
  }
  await value.submit(); assert.equal(value.writes.length, 1);
  value.destroy();
});

for (const verified of [false, true]) test('email verification is optional and driven by accepted account data: ' + verified, async () => {
  const response = success(); response.data.session.identity.verifiable_addresses = [{ via: 'email', verified }];
  const value = await registration({ response }), pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending;
  assert.equal(value.api.guideProps().needsVerification, !verified);
  assert.equal(value.writes.length, 1); assert.deepEqual(value.navigation, []);
  await value.api.guideProps().oncontinue();
  assert.deepEqual(value.navigation, ['/library?view=grid'], 'verification does not block browsing');
  value.destroy();
});

function guideControls({ accountCurrent = true, needsVerification = true, returnTo = '/library?view=grid#reading' } = {}) {
  const source = fs.readFileSync(path.join(behavior, 'AccountOnboarding.svelte'), 'utf8');
  const props = { returnTo, accountCurrent, needsVerification, oncontinue() {} };
  const mounts = [];
  class Query extends URLSearchParams { toString() { const value = super.toString(); return value ? '?' + value : ''; } }
  const context = vm.createContext({ URL, URLSearchParams: Query, window: { location: new URL('https://weebdex.example.invalid/auth/register') }, $props: () => props, $state: value => value, $derived: value => value, onMount: fn => mounts.push(fn) });
  const helper = fs.readFileSync(path.join(behavior, 'account-onboarding.ts'), 'utf8');
  vm.runInContext(transform(withoutImports(helper).replace(/^export /gm, '')), context);
  const script = source.match(/<script\b[^>]*>([\s\S]*?)<\/script>/)[1];
  const heading = source.match(/<h1\b[^>]*bind:this=\{([^}]+)\}[^>]*>/);
  const verification = source.match(/<a\b[^>]*href=\{([^}]+)\}[^>]*>Verify email<\/a>/);
  const signin = source.match(/<Button\b[^>]*href=\{([^}]+)\}[^>]*>Sign in<\/Button>/);
  const primary = source.match(/<Button\b[^>]*onclick=\{([^}]+)\}[^>]*>Continue<\/Button>/);
  assert.ok(heading && verification && signin && primary, 'exercise the actual guide controls');
  const api = vm.runInContext('(function(){' + transform(withoutImports(script)) + '\nreturn { heading: node => { ' + heading[1] + ' = node; }, verificationHref: () => (' + verification[1] + '), loginHref: () => (' + signin[1] + '), primary: () => (' + primary[1] + ') }; })()', context);
  return { source, api, mounts, props, helper, context };
}

test('welcome uses native, reachable keyboard controls, focus transfer and mobile touch targets', () => {
  const value = guideControls();
  assert.match(value.source, /aria-labelledby="account-welcome-title"/);
  assert.match(value.source, /<h1\b[^>]*id="account-welcome-title"[^>]*tabindex="-1"/);
  const focused = []; value.api.heading({ focus: options => focused.push(clone(options)) }); value.mounts.forEach(fn => fn());
  assert.deepEqual(focused, [{ preventScroll: true }]);
  assert.equal(value.api.primary(), value.props.oncontinue);
  assert.match(value.source, /<Button\b[^>]*class="[^"]*min-h-11[^\"]*w-full[^\"]*"[^>]*onclick=\{oncontinue\}>Continue<\/Button>/);
  assert.match(value.source, /\{#if needsVerification\}[\s\S]*?<a\b[^>]*min-h-11[^>]*>Verify email<\/a>/);
  assert.match(value.source, /\{#if accountCurrent\}[\s\S]*?>Continue<\/Button>[\s\S]*?\{:else\}[\s\S]*?>Sign in<\/Button>/);
  assert.doesNotMatch(value.source, /onkeydown|onkeyup|role="dialog"|position:\s*fixed|\$protocol|signEvent|connectSigner|publishEvent|localStorage|sessionStorage/);
  const button = fs.readFileSync(path.join(frontend, 'src/lib/components/ui/button/button.svelte'), 'utf8');
  assert.match(button, /type = "button"/); assert.match(button, /<button\s/);
  assert.match(button, /focus-visible:ring/);
});

test('verification and session-recovery links retain the browsing target under the real app query-prefix behavior', () => {
  const value = guideControls();
  for (const [route, href] of [['/auth/verification', value.api.verificationHref()], ['/auth/login', value.api.loginHref()]]) {
    const target = new URL(href, 'https://weebdex.example.invalid');
    assert.equal(target.pathname, route);
    assert.equal(target.searchParams.get('redirect'), '/library?view=grid#reading');
    assert.deepEqual([...target.searchParams.keys()], ['redirect']);
    assert.doesNotMatch(href, /\?\?/);
  }
  // Exercise the same native URLSearchParams prototype extension installed by
  // the retained +layout.ts, then restore it before another test can run.
  const native = URLSearchParams.prototype.toString;
  try {
    const layout = fs.readFileSync(path.join(frontend, 'src/routes/+layout.ts'), 'utf8');
    const start = layout.indexOf('const _toString ='), end = layout.indexOf('Number.prototype.short =');
    value.context.URLSearchParams = URLSearchParams;
    vm.runInContext(transform(layout.slice(start, end)), value.context);
    for (const route of ['/auth/verification', '/auth/login']) {
      const href = vm.runInContext('registrationContinuationHref(' + JSON.stringify(route) + ', "/library?view=grid#reading")', value.context);
      const target = new URL(href, 'https://weebdex.example.invalid');
      assert.equal(target.searchParams.get('redirect'), '/library?view=grid#reading');
      assert.doesNotMatch(href, /\?\?/);
    }
  } finally { URLSearchParams.prototype.toString = native; }
});

for (const [route, call] of [['login', 'create-login'], ['verification', 'create-verification']]) {
  for (const returnTo of ['/library?title=100%25&literal=%252F#reading', '/search?title=100%&literal=%2F#reading']) test('guide ' + route + ' link is consumed once and retains literal percent/query/hash: ' + returnTo, async () => {
    const controls = guideControls({ returnTo });
    const href = route === 'login' ? controls.api.loginHref() : controls.api.verificationHref();
    const expected = new URL(returnTo, 'https://weebdex.example.invalid');
    const value = await registration({ route: 'src/routes/(app)/auth/' + route + '/+page.svelte', url: new URL(href, 'https://weebdex.example.invalid').href });
    assert.equal(value.guideVisible(), false);
    const request = value.reads.find(row => Array.isArray(row) && row[0] === call);
    assert.ok(request); assert.equal(request[1].returnTo, expected.pathname + expected.search + expected.hash);
    value.destroy();
  });
  test(route + ' redirect consumer rejects a foreign target and never mounts a signup welcome', async () => {
    const value = await registration({ route: 'src/routes/(app)/auth/' + route + '/+page.svelte', url: 'https://weebdex.example.invalid/auth/' + route + '?redirect=' + encodeURIComponent('https://foreign.example.invalid/') });
    assert.equal(value.guideVisible(), false);
    const request = value.reads.find(row => Array.isArray(row) && row[0] === call);
    assert.ok(request); assert.equal(request[1].returnTo, '/');
    assert.deepEqual(value.navigation, []);
    value.destroy();
  });
}

test('Continue recovers from a navigation failure without resubmitting account creation', async () => {
  const value = await registration({ gotoFailures: 1 }), pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending;
  await value.api.guideProps().oncontinue();
  assert.equal(value.guideVisible(), true); assert.ok(value.notices.some(row => row[0] === 'error'));
  await value.api.guideProps().oncontinue();
  assert.deepEqual(value.navigation, ['/library?view=grid', '/library?view=grid']);
  assert.equal(value.writes.length, 1);
  value.destroy();
});

for (const mode of ['device', 'account']) test('creating an account preserves existing library mode and saved manga: ' + mode, async () => {
  const value = await registration();
  assert.equal(get(value.library.localLibraryState).mode, 'device', 'unselected library starts device-first');
  const records = [{ family: 'series', key: 'SeedManga1', id: 'SeedManga1', status: 'reading', notification: true }];
  value.library.localLibraryState.update(current => ({ ...current, ready: true, scopeStatus: 'verified', mode, revision: 7, records: clone(records) }));
  const before = clone(get(value.library.localLibraryState));
  const pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'refresh'));
  value.releaseRefresh(); await pending;
  assert.deepEqual(clone(get(value.library.localLibraryState)), before);
  assert.equal(get(value.library.libraryIdentity).mode, mode);
  assert.equal(get(value.library.libraryIdentity).accountId, identity.id);
  await value.api.guideProps().oncontinue();
  assert.deepEqual(clone(get(value.library.localLibraryState)), before);
  assert.equal(value.writes.length, 1);
  value.destroy();
});

for (const kind of ['back', 'navigation-start', 'destroy']) test('abandoning signup during initial form loading cannot reveal a late welcome: ' + kind, async () => {
  const value = await registration({ gatedMount: true });
  await until(() => value.reads.some(row => Array.isArray(row) && row[0] === 'create'));
  value.stale(kind); value.releaseMount(); await value.mounting; value.runEffects();
  assert.equal(value.guideVisible(), false);
  assert.equal(value.writes.length, 0); assert.deepEqual(value.navigation, []);
  if (kind !== 'destroy') value.destroy();
});
