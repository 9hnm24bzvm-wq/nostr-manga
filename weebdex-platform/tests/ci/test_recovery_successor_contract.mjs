#!/usr/bin/env node
// Execute the actual restricted boundary, navigation/form owner, composed Session
// and Mutex against the native serializer's shared synthetic response fixture.
// Svelte runes/stores and HTTP are explicit stand-ins. No services or browser.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const behavior = path.join(root, 'platform-integrations/frontend-behavior');
const frontend = path.join(root, 'frontend');
const nativeSuccess = JSON.parse(fs.readFileSync(path.join(root, 'backend/tests/fixtures/recovery-settings-success.json')));
const submittedId = 'a'.repeat(32), submittedToken = 'synthetic-submitted-csrf';
const capability = { status: 200, body: { session_purpose: 'recovery', recovery: { can_reset_password: true } } };
const normal = { status: 200, body: { identity: { id: 'synthetic-user', username: 'synthetic-user' }, roles: [], permissions: [] } };
const clone = value => structuredClone(value);
const stripImports = code => code.replace(/^\s*import[^;]+;\s*$/gm, '');
const transform = code => stripTypeScriptTypes(code, { mode: 'transform' });
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
async function until(predicate) { for (let turn = 0; turn < 80 && !predicate(); turn++) await Promise.resolve(); assert.ok(predicate(), 'finite microtask seam did not settle'); }

function initialFlow() {
  const value = clone(nativeSuccess);
  value.id = submittedId; value.state = 'choose_method';
  value.ui.action = '/auth/self-service/settings?flow=' + submittedId;
  value.ui.nodes[0].attributes.value = submittedToken;
  value.ui.messages = [];
  return value;
}
function writable(value) {
  const callbacks = new Set();
  const store = { subscribe(callback) { callbacks.add(callback); callback(value); return () => callbacks.delete(callback); }, set(next) { value = next; for (const callback of callbacks) callback(value); }, update(callback) { store.set(callback(value)); } };
  return store;
}
function get(store) { let value; const off = store.subscribe(next => { value = next; }); off(); return value; }
class AxiosError extends Error { constructor(data, status = 400) { super('synthetic validation rejection'); this.response = { status, data }; } }

async function boundary({ response = { status: 200, data: nativeSuccess }, refresh = normal, regressConsumedId = false } = {}) {
  const writes = [], reads = [], forces = [], mounts = [], destroys = [], effects = [];
  const writeGate = deferred(), refreshGate = deferred();
  const listeners = new Map();
  const window = { location: new URL(nativeSuccess.return_to), addEventListener(name, callback) { listeners.set(name, callback); }, removeEventListener(name, callback) { assert.equal(listeners.get(name), callback); listeners.delete(name); }, requestAnimationFrame(callback) { queueMicrotask(callback); } };
  let refreshing = false;
  const Auth = { async Check() { reads.push(refreshing ? 'refresh' : 'restricted'); return refreshing ? refreshGate.promise : clone(capability); } };
  const context = vm.createContext({ URL, AbortController, Error, AxiosError, console, Promise, structuredClone,
    window, page: { url: new URL(window.location.href) }, Auth, API: { Auth },
    onMount: callback => mounts.push(callback), onDestroy: callback => destroys.push(callback),
    $state: value => value, $effect: Object.assign(() => {}, { pre: callback => effects.push(callback) }),
    writable, get, StatusCodes: { OK: 200, UNAUTHORIZED: 401, FORBIDDEN: 403 }, prefs: writable({}), bindNativeMutationAuthority() {},
    Kratos: {
      async createBrowserSettingsFlow(options, config) { assert.equal(options.returnTo, '/auth/recovery'); assert.ok(config.signal instanceof AbortSignal); return { status: 200, data: initialFlow() }; },
      async updateSettingsFlow(request) { writes.push(clone(request)); return writeGate.promise; },
    },
  });
  for (const file of ['navigation-authority.ts', 'auth-form-owner.ts']) {
    const source = stripImports(fs.readFileSync(path.join(behavior, file), 'utf8')).replace(/^export /gm, '');
    vm.runInContext(transform(source), context, { filename: file });
  }
  function compose(relative) {
    const file = path.join(frontend, relative), source = fs.readFileSync(file, 'utf8');
    return createFrontendOverlays(frontend).transform.handler(source, file)?.code ?? source;
  }
  context.Mutex = vm.runInContext(transform('(function(){' + stripImports(compose('src/lib/Mutex.ts')).replace('export default Mutex;', 'return Mutex;') + '})()'), context);
  const Session = vm.runInContext(transform('(function(){' + stripImports(compose('src/lib/stores/Session.ts')).replace('export default ', 'return ') + '})()'), context);
  await until(() => reads.length === 1);
  await Session.isValid();
  assert.equal(get(Session), undefined, 'restricted capability cannot populate normal Session');
  const initialize = Session.initialize;
  Session.initialize = force => { forces.push(force); refreshing = true; return initialize(force); };
  context.Session = Session;
  let source = fs.readFileSync(path.join(behavior, 'RecoveryBoundary.svelte'), 'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  if (regressConsumedId) {
    const anchor = '      // An acknowledged write must never reuse';
    assert.ok(source.includes(anchor));
    source = source.replace(anchor, "      if (response.data.id !== flowId) throw Error('An unrelated recovery settings flow was discarded.');\n" + anchor);
  }
  const api = vm.runInContext('(function(){' + transform(stripImports(source)) + '\nreturn { resetPassword, snapshot: () => ({ ready, resetOnly, busy, success, flow, csrf, error }), setForm: value => { form = value; }, replaceFlow: value => { flow = value; } }; })()', context);
  effects.forEach(callback => callback());
  const cleanup = mounts[0]();
  await until(() => api.snapshot().ready);
  assert.equal(api.snapshot().resetOnly, true);
  assert.equal(api.snapshot().flow.id, submittedId);
  assert.equal(get(Session), undefined);
  const field = { value: 'synthetic-replacement-password' }, replacementField = { value: 'synthetic-newer-form-password' };
  api.setForm({ elements: { namedItem(name) { assert.equal(name, 'password'); return field; } } });
  return { api, field, replacementField, Session, writes, reads, forces,
    submit() { return api.resetPassword({ preventDefault() {} }); },
    releaseWrite() { if (response instanceof Error) writeGate.reject(response); else writeGate.resolve(clone(response)); },
    releaseRefresh() { if (refresh instanceof Error) refreshGate.reject(refresh); else refreshGate.resolve(clone(refresh)); },
    navigate() { vm.runInContext('invalidateNavigationEpoch()', context); window.location = new URL('/search', window.location); effects.forEach(callback => callback()); },
    startNavigation() { vm.runInContext('invalidateNavigationEpoch()', context); },
    replaceForm() { api.replaceFlow({ ...initialFlow(), id: 'foreign-current-flow' }); api.setForm({ elements: { namedItem() { return replacementField; } } }); },
    destroy() { cleanup(); destroys.forEach(callback => callback()); assert.equal(listeners.size, 0); },
  };
}

test('native handler consumes the old flow, returns fresh serialized successor and rotates cookie', () => {
  const source = fs.readFileSync(path.join(root, 'backend/src/domains/auth/flows.rs'), 'utf8');
  const handler = source.slice(source.indexOf('async fn settings('), source.indexOf('async fn create_logout('));
  assert.match(handler, /UPDATE platform\.auth_flows SET used_at=clock_timestamp\(\) WHERE id=\$1/);
  assert.match(handler, /let next_id = crypto::id\(\);[\s\S]*?let next = flow_row\([\s\S]*?let mut result = flow_body\(&next, Some\(&updated\)\);/);
  assert.match(handler, /result\["state"\] = json!\("success"\);/);
  assert.match(handler, /set_cookie\([\s\S]*?&session_token,/);
  assert.notEqual(nativeSuccess.id, submittedId);
});

test('old consumed-id guard reproduces accepted native reset with no forced session read', async () => {
  const value = await boundary({ regressConsumedId: true });
  const pending = value.submit(); value.releaseWrite(); await pending;
  assert.equal(value.writes.length, 1); assert.deepEqual(value.forces, []);
  assert.match(value.api.snapshot().error, /unrelated recovery settings flow/);
  assert.equal(value.api.snapshot().success, false); assert.equal(get(value.Session), undefined);
  value.destroy();
});

test('native successor reaches actual forced Session read while UI remains restricted until authority confirmed', async () => {
  const value = await boundary();
  const pending = value.submit();
  assert.equal(value.writes.length, 1); assert.equal(value.api.snapshot().busy, true);
  await value.submit(); assert.equal(value.writes.length, 1, 'busy write is not replayed');
  assert.deepEqual(value.forces, []);
  value.releaseWrite(); await until(() => value.reads.includes('refresh'));
  assert.deepEqual(value.forces, [true]);
  assert.equal(value.api.snapshot().flow.id, submittedId, 'successor never replaces request owner');
  assert.equal(value.api.snapshot().csrf, '');
  assert.equal(value.api.snapshot().resetOnly, true); assert.equal(value.api.snapshot().success, false);
  assert.equal(get(value.Session), undefined);
  value.releaseRefresh(); await pending;
  assert.equal(value.api.snapshot().success, true); assert.equal(value.api.snapshot().error, '');
  assert.equal(get(value.Session).identity.id, normal.body.identity.id);
  assert.equal(value.field.value, ''); assert.equal(value.api.snapshot().busy, false);
  assert.deepEqual(value.writes[0], { flow: submittedId, updateSettingsFlowBody: { method: 'password', csrf_token: submittedToken, password: 'synthetic-replacement-password' } });
  await value.submit(); assert.equal(value.writes.length, 1, 'consumed CSRF is not replayed');
  value.destroy();
});

for (const stale of ['navigate', 'startNavigation', 'destroy', 'replaceFlow', 'replaceForm']) test('stale update response cannot start normal Session refresh: ' + stale, async () => {
  const value = await boundary(), pending = value.submit();
  if (stale === 'replaceFlow') value.api.replaceFlow({ ...initialFlow(), id: 'foreign-current-flow' }); else value[stale]();
  value.releaseWrite(); await pending;
  assert.deepEqual(value.forces, []); assert.equal(value.api.snapshot().success, false);
  assert.equal(value.api.snapshot().error, ''); assert.equal(get(value.Session), undefined);
  assert.equal(value.field.value, ''); assert.equal(value.writes.length, 1);
  assert.equal(value.replacementField.value, 'synthetic-newer-form-password');
  if (stale !== 'destroy') value.destroy();
});

for (const stale of ['navigate', 'startNavigation', 'destroy', 'replaceFlow', 'replaceForm']) test('stale forced refresh cannot complete old reset UI: ' + stale, async () => {
  const value = await boundary(), pending = value.submit();
  value.releaseWrite(); await until(() => value.reads.includes('refresh'));
  if (stale === 'replaceFlow') value.api.replaceFlow({ ...initialFlow(), id: 'foreign-current-flow' }); else value[stale]();
  value.releaseRefresh(); await pending;
  assert.equal(value.api.snapshot().success, false); assert.equal(value.api.snapshot().error, '');
  assert.equal(value.writes.length, 1); assert.equal(value.field.value, '');
  assert.equal(value.replacementField.value, 'synthetic-newer-form-password');
  assert.equal(get(value.Session).identity.id, normal.body.identity.id, 'actual accepted session read may settle independently');
  if (stale !== 'destroy') value.destroy();
});

const invalidSuccessors = {
  'missing id': value => { delete value.id; },
  'empty id': value => { value.id = ''; },
  'consumed id': value => { value.id = submittedId; value.ui.action = '/auth/self-service/settings?flow=' + submittedId; },
  'API type': value => { value.type = 'api'; },
  'different return target': value => { value.return_to = 'https://recovery.example.invalid/settings/account'; },
  'foreign origin': value => { value.ui.action = 'https://foreign.example.invalid' + value.ui.action; },
  'wrong path': value => { value.ui.action = value.ui.action.replace('/settings?', '/login?'); },
  'wrong action id': value => { value.ui.action = '/auth/self-service/settings?flow=foreign'; },
  'extra action query': value => { value.ui.action += '&extra=1'; },
  'action fragment': value => { value.ui.action += '#foreign'; },
  'action credentials': value => { value.ui.action = 'https://name:synthetic@recovery.example.invalid' + value.ui.action; },
  'missing action': value => { delete value.ui.action; },
  'wrong method': value => { value.ui.method = 'GET'; },
  'reused CSRF': value => { value.ui.nodes[0].attributes.value = submittedToken; },
  'empty CSRF': value => { value.ui.nodes[0].attributes.value = ''; },
  'non-string CSRF': value => { value.ui.nodes[0].attributes.value = 1; },
  'missing password': value => { value.ui.nodes.pop(); },
  'broader account node': value => { value.ui.nodes.push({ attributes: { name: 'traits.email', value: 'synthetic@example.invalid' } }); },
  'missing UI': value => { delete value.ui; },
};
for (const [label, mutate] of Object.entries(invalidSuccessors)) test('invalid acknowledged successor cannot grant authority or replay: ' + label, async () => {
  const data = clone(nativeSuccess); mutate(data);
  const value = await boundary({ response: { status: 200, data } }), pending = value.submit();
  value.releaseWrite(); await pending;
  assert.deepEqual(value.forces, []); assert.equal(get(value.Session), undefined);
  assert.equal(value.api.snapshot().success, false); assert.equal(value.api.snapshot().resetOnly, true);
  assert.ok(value.api.snapshot().error); assert.equal(value.api.snapshot().csrf, '');
  assert.equal(value.api.snapshot().flow.id, submittedId); assert.equal(value.field.value, '');
  await value.submit(); assert.equal(value.writes.length, 1);
  value.destroy();
});

for (const [label, response, cleared] of [
  ['same-id non-success', { status: 200, data: initialFlow() }, false],
  ['foreign non-success', { status: 200, data: { ...initialFlow(), id: 'foreign' } }, false],
  ['non-200 success', { status: 202, data: nativeSuccess }, false],
  ['empty response', { status: 200, data: null }, false],
  ['same-id validation error', new AxiosError(initialFlow()), false],
  ['foreign validation error', new AxiosError({ ...initialFlow(), id: 'foreign' }), true],
  ['broader validation error', new AxiosError({ ...initialFlow(), ui: { nodes: [{ attributes: { name: 'traits.email' } }] } }), true],
]) test('unaccepted response remains restricted with no Session refresh: ' + label, async () => {
  const value = await boundary({ response }), pending = value.submit(); value.releaseWrite(); await pending;
  assert.deepEqual(value.forces, []); assert.equal(get(value.Session), undefined);
  assert.equal(value.api.snapshot().success, false); assert.equal(value.api.snapshot().resetOnly, true);
  assert.ok(value.api.snapshot().error); assert.equal(value.field.value, '');
  assert.equal(value.api.snapshot().csrf, cleared ? '' : submittedToken);
  assert.equal(value.api.snapshot().flow?.id, cleared ? undefined : submittedId);
  value.destroy();
});

for (const [label, refresh] of [['unauthorized', { status: 401 }], ['still restricted', capability], ['read failure', Error('synthetic read failure')]]) test('acknowledgement alone cannot confirm a normal account: ' + label, async () => {
  const value = await boundary({ refresh }), pending = value.submit(); value.releaseWrite();
  await until(() => value.reads.includes('refresh')); value.releaseRefresh(); await pending;
  assert.equal(value.api.snapshot().success, false); assert.equal(value.api.snapshot().resetOnly, true);
  assert.ok(value.api.snapshot().error); assert.equal(value.api.snapshot().csrf, '');
  assert.equal(get(value.Session), undefined); assert.equal(value.field.value, '');
  assert.equal(value.writes.length, 1); value.destroy();
});
