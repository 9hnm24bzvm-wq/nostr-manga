#!/usr/bin/env node
// Execute the production wizard script with explicit draft, lifecycle, clipboard
// and service stand-ins. No browser, real key, account, storage or network.
// Real cryptography/registry is covered separately by local-signing-key tests.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
const source = fs.readFileSync(new URL('../../platform-integrations/local-library/IdentitySetup.svelte', import.meta.url), 'utf8');
const script = source.match(/<script\b[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, '');
function fixture({ clipboardFails = false } = {}) {
  const calls = { copied: [], activated: 0, discarded: 0, network: 0, finished: 0 };
  const publicKey = 'synthetic-public-key', npub = 'npub1synthetic-public-key';
  const destroys = [];
  let active = true;
  const current = () => { if (!active) throw Error('Closed'); };
  const draft = () => ({ publicKey, npub, exportSecret() { throw Error('Private export was not requested'); }, async activate() { calls.activated++; }, discard() { calls.discarded++; } });
  const network = () => { calls.network++; throw Error('Service unavailable'); };
  const context = vm.createContext({ Error, Promise, Event,
    $props: Object.assign(() => ({ onfinish() { calls.finished++; }, assertCurrent: current }), { id: () => 'local-controls' }),
    $state: Object.assign(value => value, { raw: value => value }), $derived: value => value,
    onDestroy: fn => destroys.push(fn), tick: () => Promise.resolve(),
    generateLocalKeyDraft: draft, importLocalKeyDraft: draft,
    signerIdentity: {}, get: () => ({ owner: null, generation: 0 }),
    connectSigner: network, capabilities: network, proveService: network, enrollmentPolicy: network, admitAccount: network,
    navigator: { clipboard: { async writeText(value) { if (clipboardFails) throw Error('Clipboard refused'); calls.copied.push(value); } } },
  });
  const api = vm.runInContext('(function(){' + stripTypeScriptTypes(script, { mode: 'transform' }) + '\nreturn {createKey,copyPublic,reviewInstance,finishLater,acknowledge(){backupAcknowledged=true},state(){return {stage,error,notice,hasDraft:!!draft,hasIdentity:!!identity}}};})()', context);
  return { api, calls, npub, close() { active = false; destroys.forEach(fn => fn()); } };
}
test('both key backups are available before instance discovery or admission', async () => {
  const f = fixture(); f.api.createKey();
  assert.equal(f.api.state().stage, 'backup');
  const backup = source.slice(source.indexOf("{:else if stage === 'backup'"), source.indexOf("{:else if stage === 'review'"));
  assert.match(backup, /onclick=\{copyPrivate\}/);
  assert.match(backup, /aria-label="Your public key" value=\{identity.npub\} readonly/);
  assert.match(backup, /onclick=\{copyPublic\}/);
  await f.api.copyPublic(); assert.deepEqual(f.calls.copied, [f.npub]);
  assert.equal(f.calls.network, 0); assert.equal(f.calls.activated, 0);
  f.api.acknowledge(); await f.api.reviewInstance();
  assert.equal(f.api.state().stage, 'backup'); assert.match(f.api.state().error, /could not be verified/);
  await f.api.copyPublic(); assert.deepEqual(f.calls.copied, [f.npub, f.npub]);
  assert.equal(f.calls.activated, 0); f.close();
});
test('public copy refusal retains a readonly selectable fallback without activating the key', async () => {
  const f = fixture({ clipboardFails: true }); f.api.createKey(); await f.api.copyPublic();
  assert.match(f.api.state().error, /Select the public key/); assert.equal(f.calls.activated, 0); assert.equal(f.calls.network, 0); f.close();
});
test('deferring setup resets the wizard and discards unfinished key ownership', async () => {
  const f = fixture(); f.api.createKey(); f.api.finishLater();
  assert.equal(f.api.state().stage, 'choose'); assert.equal(f.api.state().hasDraft, false); assert.equal(f.api.state().hasIdentity, false);
  assert.equal(f.calls.discarded, 1); assert.equal(f.calls.finished, 1); assert.equal(f.calls.activated, 0); assert.equal(f.calls.network, 0); f.close();
});
