#!/usr/bin/env node
// Actual composed Group/User scripts, Session, Mutex and mutation owner. Stores,
// runes and API completions are explicit finite stand-ins; no browser/services.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'), frontend = path.join(root, 'frontend');
const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)];
const compose = (relative, publicProfile = false) => { const file = path.join(frontend, relative); let code = fs.readFileSync(file, 'utf8'); for (const plugin of plugins) code = plugin.transform.handler(code, file)?.code ?? code; if (publicProfile) code = createPublicApiOverlays(frontend, { enabled: true }).transform.handler(code, file)?.code ?? code; return code; };
const stripImports = code => code.replace(/^\s*import[^;]+;\s*$/gm, '');
const transform = code => stripTypeScriptTypes(code, { mode: 'transform' });
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
async function settle() { for (let i = 0; i < 30; i++) await Promise.resolve(); }
async function until(predicate) { for (let i = 0; i < 80 && !predicate(); i++) await Promise.resolve(); assert.ok(predicate(), 'finite microtask seam did not settle'); }
function writable(value) {
  const callbacks = new Set();
  const store = { subscribe(callback) { callbacks.add(callback); callback(value); return () => callbacks.delete(callback); }, set(next) { value = next; for (const callback of [...callbacks]) callback(value); }, update(callback) { store.set(callback(value)); } };
  return store;
}
function get(store) { let value; const off = store.subscribe(next => { value = next; }); off(); return value; }
const account = id => ({ identity: { id, username: id }, roles: [], permissions: [] });

async function component(kind, { blocked = false, target = 'target', publicProfile = false } = {}) {
  const entity = kind.toLowerCase(), mounts = [], destroys = [], effects = [], reads = [], writes = [], settingsWrites = [], errors = [], auth = [];
  const prefs = Object.assign(writable({ groups: [], uploaders: [], items_per_page: '20', version: 1 }), { applyMangaPreferences() {}, applyChapterPreferences() {} });
  if (blocked) prefs.set({ ...get(prefs), [kind === 'Group' ? 'groups' : 'uploaders']: [{ id: target }] });
  const window = { location: new URL('https://local.invalid/' + entity + '/' + target), requestAnimationFrame(callback) { queueMicrotask(callback); } };
  const page = { url: new URL(window.location) };
  class HTTPError extends Error { constructor(code, message) { super(message); this.code = code; } }
  const API = { Auth: { Check() { const gate = deferred(); auth.push(gate); return gate.promise; } }, Chapter: { async Search() { return { status: 200, body: { total: 4 } }; } } };
  const context = vm.createContext({ URL, URLSearchParams, AbortController, Error, console, Promise, structuredClone, window, page, HTTPError, API,
    writable, get, prefs, Auth: API.Auth, StatusCodes: { OK: 200, NO_CONTENT: 204, NOT_FOUND: 404, UNAUTHORIZED: 401, FORBIDDEN: 403 },
    onDestroy: callback => destroys.push(callback), onMount: callback => mounts.push(callback),
    $state: Object.assign(value => value, { snapshot: structuredClone }), $derived: value => value,
    $effect: Object.assign(callback => effects.push(callback), { pre: callback => effects.push(callback) }),
    $props: () => ({ data: { id: target, name: target, version: 1, ...(publicProfile ? { public_profile_readonly: true } : {}) }, children() {} }),
    useComments: () => ({ stats: {} }), setGroup() {}, setUser() {}, setChaptersCount() {},
    Permission: { [kind]: { canUpdate: () => false } }, InfoIcon: {}, FilesIcon: {}, MessagesSquare: {}, SettingsIcon: {},
    toast: { error: (...args) => errors.push(args) },
  });
  function loadModule(relative) { const source = stripImports(fs.readFileSync(path.join(root, relative), 'utf8')).replace(/^export /gm, ''); vm.runInContext(transform(source), context, { filename: relative }); }
  loadModule('platform-integrations/frontend-behavior/mutation-authority.ts');
  context.Mutex = vm.runInContext(transform('(function(){' + stripImports(compose('src/lib/Mutex.ts')).replace('export default Mutex;', 'return Mutex;') + '})()'), context);
  context.Session = vm.runInContext(transform('(function(){' + stripImports(compose('src/lib/stores/Session.ts')).replace('export default ', 'return ') + '})()'), context);
  await until(() => auth.length === 1);
  const errorSource = stripImports(compose('src/lib/constants/Error.ts')).replace(/^export /gm, '');
  context.Errors = vm.runInContext(transform('(function(){' + errorSource + '\nreturn { Error, Unknown, TooManyRequests, UpdateConflict };})()'), context);
  context.Constants = { Error: context.Errors };
  loadModule('platform-integrations/frontend-behavior/authenticated-mutation.ts');
  loadModule('platform-integrations/frontend-behavior/actor-follow-state.ts');
  loadModule('platform-integrations/frontend-behavior/account-preferences.ts');
  API.Settings = { Update(payload, intent) { const gate = deferred(); settingsWrites.push({ payload, intent, actor: get(context.Session)?.identity.id, gate }); return gate.promise; } };
  const boundary = (collection, method) => (id, intent) => { const gate = deferred(); collection.push({ actor: get(context.Session)?.identity.id, id, method, intent, gate }); return gate.promise; };
  API[kind] = { GetFollowStatus: boundary(reads, 'GET'), Follow: boundary(writes, 'POST'), Unfollow: boundary(writes, 'DELETE') };
  let source = compose(`src/routes/(app)/${entity}/[id]/${kind}.svelte`, publicProfile).match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  source = stripImports(source);
  // Model the compiler's live auto-store reads and this derived value. The
  // controller, event handler, subscriptions and pre-effect remain actual code.
  source = source.replace(/\$(followState|group|user|Session|prefs|mutex)\b/g, 'get($1)');
  source = source.replace('let isBlocked = $derived(', 'const currentBlocked = () => (').replace(/\bisBlocked\b/g, 'currentBlocked()');
  const api = vm.runInContext(transform('(function(){' + source + '\nreturn { activate: updateFollowStatus, block: updateBlockStatus, followState, targetStore: ' + entity + ', mutex, blocked: currentBlocked, snapshot: () => get(followState) }; })()'), context);
  const runEffects = () => effects.forEach(callback => callback());
  runEffects(); mounts.forEach(callback => callback()); await settle();
  return { api, reads, writes, settingsWrites, errors, auth, Session: context.Session, page,
    async login(id = 'A') { auth[0].resolve({ status: 200, body: account(id) }); await until(() => get(context.Session)?.identity.id === id); await settle(); },
    setActor(id) { context.Session.set(id ? account(id) : undefined); },
    setTarget(id, flush = true) { api.targetStore.set({ id, name: id, version: 1 }); page.url = new URL('/' + entity + '/' + id, page.url); window.location = new URL(page.url); if (flush) runEffects(); },
    async status(index, status = 204) { reads[index].gate.resolve({ status }); await settle(); },
    async readError(index, cause = Error('synthetic read failure')) { reads[index].gate.reject(cause); await settle(); },
    async mutation(index, status = 204) { writes[index].gate.resolve({ status }); await settle(); },
    async preferences(index) { const request = settingsWrites[index], body = { ...get(prefs), version: 2, ...Object.fromEntries(Object.entries(request.payload).filter(([key]) => key !== 'version').map(([key, ids]) => [key, ids.map(id => ({ id, name: id, version: 1 }))])) }; request.gate.resolve({ status: 200, body }); await settle(); },
    async writeError(index) { writes[index].gate.reject(Error('synthetic uncertain write')); await settle(); },
    destroy() { destroys.forEach(callback => callback()); },
  };
}

for (const kind of ['Group', 'User']) {
  test(`${kind}: delayed actual Auth.Check restores existing follow without anonymous read/write`, async () => {
    const value = await component(kind);
    assert.equal(value.reads.length, 0); assert.equal(value.api.snapshot().ready, false); assert.equal(get(value.api.mutex), false);
    await value.api.activate(); assert.equal(value.writes.length, 0); assert.equal(value.reads.length, 0);
    await value.login(); assert.equal(value.reads.length, 1); assert.equal(value.api.snapshot().loading, true);
    await value.api.activate(); assert.equal(value.writes.length, 0);
    await value.status(0); assert.equal(value.api.snapshot().followed, true); assert.equal(value.api.snapshot().ready, true);
    const pending = value.api.activate(); assert.equal(value.writes[0].method, 'DELETE'); assert.equal(value.writes[0].actor, 'A');
    assert.equal(value.api.snapshot().followed, true, 'no optimistic toggle'); await value.mutation(0); await pending;
    assert.equal(value.api.snapshot().followed, false); value.destroy();
  });
  for (const [first, second] of [['A', 'B'], ['B', 'A']]) test(`${kind}: ${first} to ${second} resets, late old actor status is discarded`, async () => {
    const value = await component(kind); await value.login(first);
    value.setActor(second); assert.equal(value.api.snapshot().ready, false); assert.equal(value.api.snapshot().followed, false);
    assert.equal(value.reads.length, 2); await value.status(1, 404); await value.status(0, 204);
    assert.equal(value.api.snapshot().followed, false); assert.equal(value.api.snapshot().ready, true);
    const pending = value.api.activate(); assert.equal(value.writes[0].method, 'POST'); assert.equal(value.writes[0].actor, second);
    await value.mutation(0); await pending; value.destroy();
  });
  test(`${kind}: A to B to A fences first A and same-account logout/relogin reads`, async () => {
    const value = await component(kind); await value.login(); value.setActor('B'); value.setActor('A');
    await value.status(2, 404); await value.status(0, 204); await value.status(1, 204);
    assert.equal(value.api.snapshot().followed, false);
    value.setActor(); assert.equal(value.api.snapshot().ready, false); assert.equal(value.api.snapshot().followed, false);
    await value.api.activate(); assert.equal(value.reads.length, 3); assert.equal(value.writes.length, 0);
    value.setActor('A'); await value.status(3, 204); assert.equal(value.api.snapshot().followed, true); value.destroy();
  });
  test(`${kind}: changed target blocks stale click before pre-effect and discards old target read`, async () => {
    const value = await component(kind); await value.login(); value.setTarget('new-target', false);
    await value.api.activate(); assert.equal(value.writes.length, 0); assert.equal(value.reads[1].id, 'new-target');
    await value.status(1, 404); await value.status(0, 204); assert.equal(value.api.snapshot().followed, false);
    const pending = value.api.activate(); assert.equal(value.writes[0].id, 'new-target'); assert.equal(value.writes[0].method, 'POST');
    await value.mutation(0); await pending; value.destroy();
  });
  for (const response of [undefined, 401, 403, 429, 500, 'throw']) test(`${kind}: failed status ${response} stays unknown and explicit repeated retry reads only`, async () => {
    const value = await component(kind); await value.login();
    if (response === 'throw') await value.readError(0); else { value.reads[0].gate.resolve(response === undefined ? undefined : { status: response }); await settle(); }
    assert.equal(value.api.snapshot().ready, false); assert.equal(value.api.snapshot().error, true); assert.equal(value.errors.length, 1);
    const retry = value.api.activate(); await value.api.activate(); assert.equal(value.reads.length, 2); assert.equal(value.writes.length, 0);
    await value.status(1, 204); await retry; assert.equal(value.api.snapshot().followed, true); value.destroy();
  });
  test(`${kind}: duplicate click and refresh cannot race current mutation`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404);
    const pending = value.api.activate(); await value.api.activate(); await value.api.followState.refresh();
    assert.equal(value.writes.length, 1); assert.equal(value.reads.length, 1); assert.equal(value.api.snapshot().updating, true);
    await value.mutation(0); await pending; assert.equal(value.api.snapshot().followed, true); assert.equal(value.api.snapshot().ready, true); value.destroy();
  });
  for (const change of ['actor', 'target', 'logout', 'destroy']) test(`${kind}: late mutation completion after ${change} cannot restore old follow state`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404); const pending = value.api.activate();
    if (change === 'actor') value.setActor('B'); else if (change === 'target') value.setTarget('other'); else if (change === 'logout') value.setActor(); else value.destroy();
    assert.equal(value.writes[0].intent.alive(), false);
    if (change === 'actor' || change === 'target') await value.status(1, 404);
    const previous = structuredClone(value.api.snapshot()); await value.mutation(0); await pending;
    assert.deepEqual(structuredClone(value.api.snapshot()), previous); assert.equal(value.errors.length, 0);
    if (change !== 'destroy') value.destroy();
  });
  test(`${kind}: uncertain mutation failure requires status reconciliation and never replays`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404); const pending = value.api.activate();
    await value.writeError(0); await pending; assert.equal(value.api.snapshot().error, true); assert.equal(value.api.snapshot().ready, false);
    const retry = value.api.activate(); await value.api.activate(); assert.equal(value.writes.length, 1); assert.equal(value.reads.length, 2);
    await value.status(1, 204); await retry; assert.equal(value.api.snapshot().followed, true); value.destroy();
  });
  test(`${kind}: same-actor Session revision cannot read ahead of acknowledged pending write`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404); const pending = value.api.activate();
    value.setActor('A'); assert.equal(value.reads.length, 1, 'matching pending write defers replacement GET');
    assert.equal(value.api.snapshot().ready, false); assert.equal(value.api.snapshot().loading, true);
    await value.api.activate(); await value.api.block(); assert.equal(value.writes.length, 1); assert.equal(value.settingsWrites.length, 0);
    await value.mutation(0); await pending; assert.equal(value.reads.length, 2, 'acknowledgment starts post-settlement reconciliation');
    assert.equal(value.api.snapshot().ready, false); await value.status(1, 204); assert.equal(value.api.snapshot().followed, true); value.destroy();
  });
  test(`${kind}: same-actor canceled or uncertain write stays unknown until explicit status retry`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404); const pending = value.api.activate();
    value.setActor('A'); await value.writeError(0); await pending;
    assert.equal(value.reads.length, 1); assert.equal(value.api.snapshot().ready, false); assert.equal(value.api.snapshot().error, true);
    await value.api.block(); assert.equal(value.settingsWrites.length, 0);
    const retry = value.api.activate(); assert.equal(value.reads.length, 2); assert.equal(value.writes.length, 1);
    await value.status(1, 204); await retry; assert.equal(value.api.snapshot().followed, true); value.destroy();
  });
  for (const roundTrip of ['actor', 'target']) test(`${kind}: ${roundTrip} round trip waits for its original pending write before current reconciliation`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404); const pending = value.api.activate();
    if (roundTrip === 'actor') { value.setActor('B'); value.setActor('A'); }
    else { value.setTarget('other'); value.setTarget('target'); }
    assert.equal(value.reads.length, 2, 'different context can read, matching original context waits');
    await value.status(1, 404); assert.equal(value.api.snapshot().ready, false);
    await value.mutation(0); await pending; assert.equal(value.reads.length, 3);
    await value.status(2, 204); assert.equal(value.api.snapshot().followed, true); assert.equal(value.api.snapshot().ready, true); value.destroy();
  });
  test(`${kind}: new Block requires known current absence through refresh, error and actor switch`, async () => {
    const value = await component(kind); await value.login();
    await value.api.block(); assert.equal(value.settingsWrites.length, 0, 'unknown initial status cannot Block');
    await value.status(0, 204); await value.api.block(); assert.equal(value.settingsWrites.length, 0, 'followed target cannot Block');
    const refresh = value.api.followState.refresh(); await value.api.block(); assert.equal(value.settingsWrites.length, 0, 'loading status cannot Block');
    await value.readError(1); await refresh; await value.api.block(); assert.equal(value.settingsWrites.length, 0, 'error status cannot Block');
    value.setActor('B'); await value.api.block(); assert.equal(value.settingsWrites.length, 0, 'new actor must establish its own status');
    await value.status(2, 404); const pending = value.api.block(); assert.equal(value.settingsWrites.length, 1); assert.equal(value.settingsWrites[0].actor, 'B');
    assert.deepEqual(Array.from(value.settingsWrites[0].payload[kind === 'Group' ? 'groups' : 'uploaders']), ['target']);
    await value.preferences(0); await pending; value.destroy();
  });
  test(`${kind}: new Block is fenced by live target before the pre-effect runs`, async () => {
    const value = await component(kind); await value.login(); await value.status(0, 404); value.setTarget('other', false);
    await value.api.block(); assert.equal(value.settingsWrites.length, 0); value.destroy();
  });
  test(`${kind}: legitimate Unblock remains available during unknown/loading/error follow status`, async () => {
    for (const phase of ['loading', 'error']) {
      const value = await component(kind, { blocked: true }); await value.login(); if (phase === 'error') await value.readError(0);
      const pending = value.api.block(); assert.equal(value.settingsWrites.length, 1, phase); assert.equal(value.settingsWrites[0].actor, 'A');
      assert.deepEqual(Array.from(value.settingsWrites[0].payload[kind === 'Group' ? 'groups' : 'uploaders']), []);
      await value.preferences(0); await pending; value.destroy();
    }
  });
  test(`${kind}: existing block availability is independent of successful status readiness`, async () => {
    const value = await component(kind, { blocked: true }); await value.login(); await value.status(0, 404);
    assert.equal(value.api.blocked(), true); await value.api.activate(); assert.equal(value.writes.length, 0); value.destroy();
  });
}

test('User: existing self-follow guard still hides/disallows mutation after status readiness', async () => {
  const value = await component('User', { target: 'A' }); await value.login(); await value.status(0, 404);
  await value.api.activate(); await value.api.block(); assert.equal(value.writes.length, 0); assert.equal(value.settingsWrites.length, 0); value.destroy();
});

test('exact composed Follow control preserves permitted visibility, classes, icons and structure except scoped readiness/retry/block guard', () => {
  for (const kind of ['Group', 'User']) {
    const source = compose(`src/routes/(app)/${kind.toLowerCase()}/[id]/${kind}.svelte`);
    assert.match(source, /onclick=\{updateFollowStatus\}\s+disabled=\{\$mutex \|\| \$followState.loading \|\| \$followState.updating \|\| \(!\$followState.ready && !\$followState.error\)\}/);
    assert.match(source, /\{#if \$followState.error\}\s+Retry follow status\s+\{:else if isFollowed\}/);
    assert.ok(source.includes('{#if !isBlocked}'));
    assert.ok(source.includes('{#if isBlocked || ($followState.ready && !isFollowed)}'));
    assert.ok(source.includes('(!isBlocked && !followState.knownAbsent())'));
    if (kind === 'User') assert.ok(source.includes('{#if $Session && $user.id !== $Session.identity.id}'));
    else assert.ok(source.includes('{#if $Session}'));
  }
});

test('public UUID profile never borrows a native Session for follow status or writes', async () => {
  const value = await component('User', { publicProfile: true }); await value.login();
  assert.equal(value.reads.length, 0); assert.equal(value.writes.length, 0); assert.equal(value.errors.length, 0);
  await value.api.activate(); assert.equal(value.reads.length, 0); assert.equal(value.writes.length, 0);
  assert.equal(value.api.snapshot().ready, false); assert.equal(value.api.snapshot().followed, false); value.destroy();
});
