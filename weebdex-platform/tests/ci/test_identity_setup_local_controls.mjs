#!/usr/bin/env node
// Execute the production wizard script with explicit draft, lifecycle, clipboard
// and service stand-ins. No browser, real key, account, storage or network.
// Real cryptography/registry is covered separately by local-signing-key tests.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
const source = fs.readFileSync(process.env.WEEBDEX_IDENTITY_SETUP_SOURCE ?? new URL('../../platform-integrations/local-library/IdentitySetup.svelte', import.meta.url), 'utf8');
const script = source.match(/<script\b[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, '');
function fixture({ clipboardFails = false, connected = false, resumeSelected = false } = {}) {
  const calls = { copied: [], activated: 0, discarded: 0, network: 0, finished: 0, signers: 0 };
  let selection = connected ? { owner: 'existing-public-key', generation: 1, source: 'local' } : { owner: null, generation: 0, source: null };
  const publicKey = 'synthetic-public-key', npub = 'npub1synthetic-public-key';
  const destroys = [];
  let active = true;
  const current = () => { if (!active) throw Error('Closed'); };
  const draft = () => ({ publicKey, npub, exportSecret() { throw Error('Private export was not requested'); }, async activate(assertCurrent) { current(); assertCurrent(); calls.activated++; selection = { owner: publicKey, generation: selection.generation + 1, source: 'local' }; }, discard() { calls.discarded++; } });
  const network = () => { calls.network++; throw Error('Service unavailable'); };
  const context = vm.createContext({ Error, Promise, Event,
    $props: Object.assign(() => ({ onfinish() { calls.finished++; }, assertCurrent: current, resumeSelected }), { id: () => 'local-controls' }),
    $state: Object.assign(value => value, { raw: value => value }), $derived: value => value,
    onDestroy: fn => destroys.push(fn), tick: () => Promise.resolve(), untrack: fn => fn(),
    generateLocalKeyDraft: draft, importLocalKeyDraft: draft,
    signerIdentity: {}, get: () => selection, encodePublicKey: owner => 'npub1' + owner,
    async connectSigner(assertCurrent) { assertCurrent(); calls.signers++; selection = { owner: 'external-public-key', generation: selection.generation + 1, source: 'external' }; }, capabilities: network, proveService: network, enrollmentPolicy: network, admitAccount: network,
    navigator: { clipboard: { async writeText(value) { if (clipboardFails) throw Error('Clipboard refused'); calls.copied.push(value); } } },
  });
  const api = vm.runInContext('(function(){' + stripTypeScriptTypes(script, { mode: 'transform' }) + '\nreturn {createKey,openImport,importKey,useExtension,copyPublic,reviewInstance,connectSession,join,finishLater,bindInput(value){importInput=value},acknowledge(){backupAcknowledged=true},state(){return {stage,error,notice,hasDraft:!!draft,hasIdentity:!!identity,installed,backupAcknowledged}}};})()', context);
  return { api, calls, npub, selection: () => selection, stale() { active = false; }, replaceSelection() { selection = { owner: 'newer-public-key', generation: selection.generation + 1, source: 'external' }; }, close() { active = false; destroys.forEach(fn => fn()); } };
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


test('backup step offers explicit local connection independently of instance review', () => {
  const backup = source.slice(source.indexOf("{:else if stage === 'backup'"), source.indexOf("{:else if stage === 'review'"));
  assert.match(backup, /disabled=\{pending \|\| !backupAcknowledged\} onclick=\{connectSession\}>Use this key for this session/);
  assert.match(backup, /does not request admission or publish anything/);
  const connected = source.slice(source.indexOf("{:else if stage === 'connected'"), source.indexOf("{:else if stage === 'done'"));
  assert.match(connected, /onclick=\{reviewInstance\}>Review joining this site/);
});
for (const keySource of ['generated', 'imported']) test('acknowledged ' + keySource + ' key connects locally without instance discovery', async () => {
  const f = fixture();
  if (keySource === 'generated') f.api.createKey();
  else { f.api.openImport(); f.api.bindInput({ value: 'synthetic-imported-key', dispatchEvent() {} }); f.api.importKey(); }
  await f.api.connectSession(); assert.equal(f.calls.activated, 0, 'backup acknowledgement is required');
  assert.equal(f.selection().owner, null); assert.equal(f.calls.network, 0);
  f.api.acknowledge(); await f.api.connectSession();
  assert.equal(f.api.state().stage, 'connected'); assert.equal(f.api.state().installed, true);
  assert.equal(f.selection().owner, 'synthetic-public-key'); assert.equal(f.selection().source, 'local');
  assert.equal(f.calls.activated, 1); assert.equal(f.calls.network, 0); assert.equal(f.calls.finished, 0);
  await f.api.join(); assert.equal(f.calls.network, 0, 'local connection cannot bypass reviewed recipient admission');
  f.close();
});
test('failed instance discovery still permits explicitly acknowledged local connection', async () => {
  const f = fixture(); f.api.createKey(); f.api.acknowledge(); await f.api.reviewInstance();
  assert.match(f.api.state().error, /could not be verified/); assert.equal(f.calls.network, 1);
  await f.api.connectSession(); assert.equal(f.api.state().stage, 'connected');
  assert.equal(f.calls.activated, 1); assert.equal(f.calls.network, 1); assert.equal(f.api.state().error, '');
  // Optional later discovery may fail, but cannot undo or mislabel local identity.
  await f.api.reviewInstance(); assert.equal(f.api.state().stage, 'connected');
  assert.equal(f.selection().source, 'local'); assert.equal(f.calls.activated, 1); assert.equal(f.calls.network, 2);
  f.close();
});
test('external signer local continuation requires backup acknowledgement and no instance discovery', async () => {
  const f = fixture(); await f.api.useExtension(); assert.equal(f.calls.signers, 1);
  await f.api.connectSession(); assert.equal(f.api.state().stage, 'backup');
  f.api.acknowledge(); await f.api.connectSession(); assert.equal(f.api.state().stage, 'connected');
  assert.equal(f.selection().source, 'external'); assert.equal(f.calls.activated, 0); assert.equal(f.calls.network, 0); f.close();
});
test('local setup cannot overwrite a newer signing selection or activate after closure', async () => {
  for (const change of ['replaceSelection', 'stale', 'close']) {
    const f = fixture(); f.api.createKey(); f.api.acknowledge(); f[change](); const before = f.selection();
    await f.api.connectSession(); assert.equal(f.selection(), before); assert.equal(f.calls.activated, 0);
    assert.equal(f.calls.network, 0); assert.notEqual(f.api.state().stage, 'connected'); f.close();
  }
});

test('advanced setup resumes the already selected identity without a second key or implicit request', async () => {
 const f=fixture({connected:true,resumeSelected:true});assert.equal(f.api.state().stage,'connected');assert.equal(f.api.state().hasDraft,false);assert.equal(f.calls.activated,0);assert.equal(f.calls.network,0);assert.equal(f.calls.signers,0);await f.api.reviewInstance();assert.equal(f.calls.network,1);assert.equal(f.selection().owner,'existing-public-key');f.close();
});
