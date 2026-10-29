#!/usr/bin/env node
// Execute the actual wizard script and real local key/registry/signExact modules.
// Finite UI lifecycle, clipboard and verified-service adapters are synthetic.
// No browser, real account, real network, private-key screenshot or persisted key.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import { fixture } from './library-management-fixture.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const wizardFile = process.env.WEEBDEX_IDENTITY_SETUP_SOURCE ?? path.join(root, 'platform-integrations/local-library/IdentitySetup.svelte');
const source = fs.readFileSync(wizardFile, 'utf8');
const encoding = await import(new URL('../../.build/local-signing-base-2.0.0/node_modules/@scure/base/index.js', import.meta.url));
const syntheticPrivate = '0'.repeat(63) + '1';
const get = store => { let value; store.subscribe(next => { value = next; })(); return value; };
const deferred = () => { let resolve; const promise = new Promise(yes => { resolve = yes; }); return { promise, resolve }; };

async function wizard({ downloadFails = false, changeOnDownloadUrl = false, clipboardFails = false, capabilityFails = false, proofFails = false, admissionFails = false, enrollmentOpen = true, policySequence, gateAt } = {}) {
  const f = await fixture({ moduleAdapters: { '@scure/base': encoding } });
  f.publication.disconnectSigner();
  const keys = await f.load('local-signing-key.ts');
  const gate = deferred(), entered = deferred(), destroyers = [];
  const calls = { enrollmentChecks: 0, capabilities: 0, proofs: 0, admissions: 0, signedProofs: 0, clipboard: 0, privateClipboard: 0, finished: 0, focuses: 0, profileOpened: 0, remembered: 0 };
  let active = true;
  const current = () => { if (!active) throw Error('Parent setup closed'); };
  const input = { value: '', dispatchEvent() {} };
  const props = Object.assign(() => ({ onfinish: () => { calls.finished++; }, onprofile: () => { calls.profileOpened++; }, assertCurrent: current }), { id: () => 'synthetic-setup' });
  const localState = Object.assign(value => value, { raw: value => value });
  const waitAt = async boundary => { if (gateAt === boundary) { entered.resolve(); await gate.promise; } };
  const downloads = { blobs: [], clicked: 0, removed: 0, revoked: [], filename: '', href: '' };
  const context = vm.createContext({ Error, Promise, Event, Blob,
    URL: { createObjectURL(file) { downloads.blobs.push(file); if (changeOnDownloadUrl) f.publication.disconnectSigner(); return 'blob:synthetic-download'; }, revokeObjectURL(url) { downloads.revoked.push(url); } },
    document: { body: { appendChild() {} }, createElement(tag) { assert.equal(tag, 'a'); return { href: '', download: '', hidden: false, click() { if (downloadFails) throw Error('Synthetic download refusal'); downloads.clicked++; downloads.filename = this.download; downloads.href = this.href; }, remove() { downloads.removed++; } }; } },
    console: { error() { throw Error('Unexpected wizard logging'); } },
    $props: props, $state: localState, $derived: value => value, get,
    onDestroy: fn => destroyers.push(fn), tick: () => Promise.resolve(), untrack: fn => fn(),
    ...keys, signerIdentity: f.publication.signerIdentity,
    connectSigner: f.publication.connectSigner, assertAllowedRelay: f.publication.assertAllowedRelay,
    relayPreferences: async () => { await waitAt('preferences'); return f.publication.relayPreferences(); },
    rememberRelay: async (...args) => { calls.remembered++; await waitAt('remember'); return f.publication.rememberRelay(...args); },
    async capabilities() { calls.capabilities++; await waitAt('capabilities'); if (capabilityFails) throw Error('Synthetic capability failure'); return f.publication.capabilities(); },
    async proveService(origin, key) { calls.proofs++; await waitAt('proof'); if (proofFails) throw Error('Synthetic service proof failure'); assert.equal(origin, f.review.recipients[0].origin); assert.equal(key, f.review.recipients[0].serviceKey); return { origin, serviceKey: key }; },
    async enrollmentPolicy(relay, assertCurrent) {
      calls.enrollmentChecks++; assertCurrent(); await waitAt('enrollment-policy'); assertCurrent();
      assert.equal(relay.origin, f.review.recipients[0].origin);
      return { enabled: policySequence ? policySequence[Math.min(calls.enrollmentChecks - 1, policySequence.length - 1)] : enrollmentOpen };
    },
    async admitAccount(relay, assertCurrent) {
      calls.admissions++; assertCurrent(); await waitAt('admission'); assertCurrent();
      if (admissionFails) throw Error('Synthetic admission failure');
      const selected = f.publication.selectedSigner();
      const proof = await f.nostr.signExact({ signEvent(template) { assertCurrent(); return selected.signer.signEvent(template); } }, selected.owner,
        { kind: 27235, created_at: Math.floor(Date.now() / 1000), tags: [['u', relay.origin + '/enrollment'], ['method', 'POST']], content: '' });
      assertCurrent(); assert.equal(f.nostr.verifyEvent(proof), true); calls.signedProofs++;
      return { status: 201, data: { admitted: true } };
    },
    navigator: { clipboard: { async writeText(value) { calls.clipboard++; if (value.startsWith('nsec1')) calls.privateClipboard++; await waitAt('clipboard'); if (clipboardFails) throw Error('Synthetic clipboard refusal'); } } },
  });
  const script = source.match(/<script\b[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, '');
  const api = vm.runInContext('(function(){' + stripTypeScriptTypes(script, { mode: 'transform' }) + '\nreturn { createKey, openImport, importKey, downloadRecoveryKey, useExtension, reveal, copyPrivate, copyPublic, reviewInstance, join, openProfile, connectSession, finishLater, startAgain, bindInput(value){importInput=value}, bindHeading(value){heading=value}, acknowledge(){backupAcknowledged=true}, state(){return {stage,hasDraft:!!draft,hasSecret:!!revealedSecret,hasIdentity:!!identity,hasRelay:!!relay,pending,backupAcknowledged,error,notice,installed,enrollmentOpen}}, draftHandle(){return draft}, publicIdentity(){return identity} }; })()', context);
  api.bindInput(input); api.bindHeading({ focus() { calls.focuses++; } });
  return { f, api, input, calls, downloads, entered: entered.promise, release: gate.resolve,
    identity: () => get(f.publication.signerIdentity),
    stale() { active = false; }, destroy() { destroyers.forEach(fn => fn()); },
    async dispose() { destroyers.forEach(fn => fn()); f.publication.disconnectSigner(); await f.dispose(); },
  };
}
async function imported(options, input = syntheticPrivate) { const w = await wizard(options); w.api.openImport(); w.input.value = input; w.api.importKey(); return w; }

test('private-key controls have no native form/name/URL fallback or implicit clipboard action', () => {
  assert.doesNotMatch(source, /<form\b|name=["'](?:private-key|nsec|secret)["']/);
  assert.match(source, /type="password" autocomplete="off"/);
  assert.match(source, /data-1p-ignore/); assert.match(source, /data-lpignore/); assert.match(source, /data-bwignore/);
  assert.match(source, /type="button"[^>]*onclick=\{\(\) => importKey\(\)\}/);
  assert.match(source, /event\.key === 'Enter'[\s\S]*?event\.preventDefault\(\); importKey\(\)/);
  assert.match(source, /type="checkbox" bind:checked=\{backupAcknowledged\}/);
  assert.match(source, /onclick=\{join\}/); assert.match(source, /onclick=\{copyPrivate\}/);
  assert.doesNotMatch(source, /localStorage|sessionStorage|indexedDB|console\.|fetch\(|URLSearchParams|JSON\.stringify\(draft/);
});

test('opening, generating and acknowledging a key never selects, signs, sends or copies it', async () => {
  const w = await wizard();
  try {
    assert.equal(w.api.state().stage, 'choose'); w.api.createKey();
    assert.equal(w.api.state().stage, 'backup'); assert.equal(w.api.state().hasSecret, false);
    assert.equal(w.identity().owner, null); assert.equal(w.calls.clipboard, 0);
    await w.api.reviewInstance(); await w.api.join();
    assert.equal(w.calls.capabilities, 0); assert.equal(w.calls.proofs, 0); assert.equal(w.calls.admissions, 0);
    w.api.acknowledge(); assert.equal(w.calls.signedProofs, 0); assert.equal(w.f.calls.posts, 0);
  } finally { await w.dispose(); }
});

test('paste imports only locally, immediately clears input, and rejects public-only/corrupt keys', async () => {
  const w = await imported();
  try {
    assert.equal(w.input.value, ''); assert.equal(w.api.state().stage, 'backup'); assert.equal(w.identity().owner, null);
    const npub = w.api.publicIdentity().npub;
    for (const invalid of [npub, 'nsec1invalid', '0'.repeat(64)]) {
      w.api.startAgain(); w.api.openImport(); w.input.value = invalid; w.api.importKey();
      assert.equal(w.input.value, ''); assert.equal(w.api.state().stage, 'import'); assert.equal(w.api.state().hasDraft, false);
      assert.equal(w.api.state().error.includes(invalid), false);
    }
    assert.equal(w.calls.clipboard, 0); assert.equal(w.calls.proofs, 0); assert.equal(w.calls.admissions, 0);
  } finally { await w.dispose(); }
});

test('pasting nsec or hex selects the same public candidate without private-key authority', async () => {
  const nsec = encoding.bech32.encode('nsec', encoding.bech32.toWords(Uint8Array.from(Buffer.from(syntheticPrivate, 'hex'))));
  const hexWizard = await imported(), nsecWizard = await imported({}, nsec);
  try {
    assert.equal(hexWizard.api.publicIdentity().npub, nsecWizard.api.publicIdentity().npub);
    assert.equal(nsecWizard.input.value, ''); assert.equal(nsecWizard.identity().owner, null);
    assert.equal(nsecWizard.calls.admissions, 0); assert.equal(nsecWizard.calls.clipboard, 0);
  } finally { await hexWizard.dispose(); await nsecWizard.dispose(); }
});

test('explicit reveal/hide and copy are separate; clipboard failure retains a local selectable fallback', async () => {
  const w = await imported({ clipboardFails: true });
  try {
    assert.equal(w.api.state().hasSecret, false); await w.api.copyPrivate();
    assert.equal(w.calls.clipboard, 1); assert.equal(w.calls.privateClipboard, 1); assert.match(w.api.state().error, /Reveal the private key/);
    assert.equal(w.api.state().hasSecret, false); w.api.reveal(); assert.equal(w.api.state().hasSecret, true);
    w.api.reveal(); assert.equal(w.api.state().hasSecret, false); assert.equal(w.calls.admissions, 0);
  } finally { await w.dispose(); }
});

test('backup review verifies only the allowed recipient; explicit Join alone activates and signs admission', async () => {
  const w = await imported();
  try {
    w.api.reveal(); w.api.acknowledge(); await w.api.reviewInstance();
    assert.equal(w.api.state().stage, 'review'); assert.equal(w.api.state().hasSecret, false);
    assert.equal(w.calls.capabilities, 1); assert.equal(w.calls.proofs, 1); assert.equal(w.calls.admissions, 0); assert.equal(w.identity().owner, null);
    const joining = w.api.join(); await w.api.join(); await joining; assert.equal(w.api.state().stage, 'done');
    assert.equal(w.calls.admissions, 1); assert.equal(w.calls.signedProofs, 1); assert.equal(w.identity().source, 'local');
    assert.equal(w.identity().owner, w.api.publicIdentity().publicKey); assert.equal(w.f.calls.posts, 0);
    assert.equal(w.calls.clipboard, 0); assert.deepEqual(w.f.calls.kinds, [], 'no external signer invocation');
  } finally { await w.dispose(); }
});

for (const options of [{ capabilityFails: true }, { proofFails: true }]) test('failed recipient verification keeps pending key local and cannot admit: ' + Object.keys(options)[0], async () => {
  const w = await imported(options);
  try { w.api.acknowledge(); await w.api.reviewInstance(); assert.equal(w.api.state().stage, 'backup'); assert.equal(w.identity().owner, null); assert.equal(w.calls.admissions, 0); await w.api.join(); assert.equal(w.calls.admissions, 0); }
  finally { await w.dispose(); }
});

test('failed admission keeps the same selected session identity for explicit retry without regenerating or copying', async () => {
  const w = await imported({ admissionFails: true });
  try { w.api.acknowledge(); await w.api.reviewInstance(); await w.api.join(); const selected = w.identity();
    assert.equal(w.api.state().stage, 'review'); assert.equal(w.api.state().installed, true); assert.equal(w.calls.signedProofs, 0);
    await w.api.join(); assert.equal(w.identity(), selected); assert.equal(w.calls.admissions, 2); assert.equal(w.calls.clipboard, 0);
  } finally { await w.dispose(); }
});

for (const mode of ['finish-later', 'destroy', 'parent-navigation']) test('closing during pending service verification suppresses continuation: ' + mode, async () => {
  const w = await imported({ gateAt: 'proof' });
  try { w.api.acknowledge(); const pending = w.api.reviewInstance(); await w.entered;
    const held = w.api.draftHandle(); if (mode === 'finish-later') w.api.finishLater(); else if (mode === 'destroy') w.destroy(); else w.stale();
    w.release(); await pending; assert.equal(w.identity().owner, null); assert.equal(w.calls.admissions, 0); assert.notEqual(w.api.state().stage, 'review');
    if (mode !== 'parent-navigation') assert.throws(() => held.exportSecret(), /no longer available|no longer current/);
  } finally { w.release(); await w.dispose(); }
});

test('closing pending admission never accepts a late result or permits a deferred signature', async () => {
  const w = await imported({ gateAt: 'admission' });
  try { w.api.acknowledge(); await w.api.reviewInstance(); const pending = w.api.join(); await w.entered;
    w.api.finishLater(); w.release(); await pending;
    assert.equal(w.calls.signedProofs, 0); assert.notEqual(w.api.state().stage, 'done'); assert.equal(w.calls.finished, 1);
  } finally { w.release(); await w.dispose(); }
});

test('superseding an identity after key review prevents stale activation and admission', async () => {
  const w = await imported();
  try { w.api.acknowledge(); await w.api.reviewInstance(); await w.f.publication.connectSigner(); const selected = w.identity();
    await w.api.join(); assert.equal(w.identity(), selected); assert.equal(w.calls.admissions, 0); assert.equal(w.calls.signedProofs, 0);
  } finally { await w.dispose(); }
});

test('locking the key during pending admission prevents a deferred local proof and late success', async () => {
  const w = await imported({ gateAt: 'admission' });
  try {
    w.api.acknowledge(); await w.api.reviewInstance(); const pending = w.api.join(); await w.entered;
    w.f.publication.disconnectSigner(); w.release(); await pending;
    assert.equal(w.identity().owner, null); assert.equal(w.calls.signedProofs, 0); assert.notEqual(w.api.state().stage, 'done');
  } finally { w.release(); await w.dispose(); }
});

test('a replaced wizard cannot steal focus after an awaited UI tick', async () => {
  const w = await wizard();
  try { w.api.createKey(); w.stale(); await Promise.resolve(); await Promise.resolve(); assert.equal(w.calls.focuses, 0); }
  finally { await w.dispose(); }
});

test('Lock makes an installed wizard identity unusable; a fresh session starts without authority', async () => {
  const w = await imported();
  try { w.api.acknowledge(); await w.api.reviewInstance(); await w.api.join(); const old = w.f.publication.selectedSigner();
    w.f.publication.disconnectSigner(); assert.throws(old.check, /identity changed/); assert.equal(w.identity().owner, null);
  } finally { await w.dispose(); }
  const fresh = await wizard(); try { assert.equal(fresh.identity().owner, null); assert.equal(fresh.api.state().hasDraft, false); assert.equal(fresh.api.state().hasSecret, false); } finally { await fresh.dispose(); }
});

test('closing during an explicit clipboard request suppresses stale notices and clears private UI state', async () => {
  const w = await imported({ gateAt: 'clipboard' });
  try { w.api.reveal(); const pending = w.api.copyPrivate(); await w.entered; w.destroy(); w.release(); await pending;
    assert.equal(w.calls.privateClipboard, 1); assert.equal(w.api.state().hasSecret, false); assert.equal(w.api.state().notice, ''); assert.equal(w.calls.admissions, 0);
  } finally { w.release(); await w.dispose(); }
});

// Reconstructed after environment reset. Fresh results, not lost historical proof.
test('closed enrollment review remains local and never offers implicit admission', async () => {
  const w = await imported({ enrollmentOpen: false });
  try {
    w.api.acknowledge(); await w.api.reviewInstance();
    assert.equal(w.api.state().stage, 'review'); assert.equal(w.api.state().enrollmentOpen, false);
    await w.api.join(); assert.equal(w.calls.admissions, 0); assert.equal(w.identity().owner, null);
    assert.match(source, /This site needs approval from its team/);
    assert.match(source, /already-approved key can still connect/);
  } finally { await w.dispose(); }
});
test('fresh policy closing after review suppresses key activation and enrollment', async () => {
  const w = await imported({ policySequence: [true, false] });
  try {
    w.api.acknowledge(); await w.api.reviewInstance(); await w.api.join();
    assert.equal(w.calls.enrollmentChecks, 2); assert.equal(w.api.state().enrollmentOpen, false);
    assert.equal(w.calls.admissions, 0); assert.equal(w.identity().owner, null); assert.equal(w.api.state().installed, false);
  } finally { await w.dispose(); }
});
test('closed instance explicitly connects a reviewed key for this session without asserting admission', async () => {
  const w = await imported({ enrollmentOpen: false });
  try {
    await w.api.connectSession(); assert.equal(w.identity().owner, null);
    w.api.acknowledge(); await w.api.reviewInstance(); await w.api.connectSession();
    assert.equal(w.api.state().stage, 'connected'); assert.equal(w.identity().source, 'local');
    assert.equal(w.identity().owner, w.api.publicIdentity().publicKey);
    assert.equal(w.calls.admissions, 0); assert.equal(w.calls.signedProofs, 0); assert.equal(w.f.calls.posts, 0);
    assert.match(source, /site access has not been checked/);
    const old = w.f.publication.selectedSigner(); w.f.publication.disconnectSigner(); assert.throws(old.check, /identity changed/);
    w.api.startAgain(); w.api.openImport(); w.input.value = syntheticPrivate; w.api.importKey();
    w.api.acknowledge(); await w.api.reviewInstance(); await w.api.connectSession(); assert.equal(w.api.state().stage, 'connected');
    assert.equal(w.calls.admissions, 0);
  } finally { await w.dispose(); }
});
test('local reconnect cannot overwrite an identity selected after review', async () => {
  const w = await imported({ enrollmentOpen: false });
  try {
    w.api.acknowledge(); await w.api.reviewInstance(); await w.f.publication.connectSigner(); const selected = w.identity();
    await w.api.connectSession(); assert.equal(w.identity(), selected); assert.notEqual(w.api.state().stage, 'connected');
    assert.equal(w.calls.admissions, 0);
  } finally { await w.dispose(); }
});


test('recovery download is explicit, local, reimportable and never acknowledges or activates identity', async () => {
  const w = await imported(); let recovered;
  try {
    const publicKey = w.api.publicIdentity().npub;
    assert.equal(w.downloads.blobs.length, 0); w.api.downloadRecoveryKey();
    assert.equal(w.downloads.clicked, 1); assert.equal(w.downloads.removed, 1);
    assert.equal(w.downloads.filename, 'weebdex-recovery-key.txt'); assert.equal(w.downloads.href, 'blob:synthetic-download');
    assert.deepEqual(w.downloads.revoked, ['blob:synthetic-download']);
    assert.equal(w.api.state().backupAcknowledged, false); assert.equal(w.api.state().installed, false);
    assert.equal(w.identity().owner, null); assert.equal(w.calls.admissions, 0); assert.equal(w.calls.clipboard, 0); assert.equal(w.f.calls.posts, 0);
    assert.match(w.api.state().notice, /does not verify your backup/);
    const contents = await w.downloads.blobs[0].text();
    recovered = await imported({}, contents); assert.equal(recovered.api.publicIdentity().npub, publicKey);
    assert.equal(recovered.identity().owner, null); assert.equal(recovered.input.value, '');
  } finally { if (recovered) await recovered.dispose(); await w.dispose(); }
});
test('failed recovery download cleans up without acknowledgement or private error values', async () => {
  const w = await imported({ downloadFails: true });
  try { w.api.downloadRecoveryKey(); assert.equal(w.downloads.clicked, 0); assert.equal(w.downloads.removed, 1); assert.equal(w.downloads.revoked.length, 1); assert.equal(w.api.state().backupAcknowledged, false); assert.equal(w.identity().owner, null); assert.match(w.api.state().error, /could not be downloaded safely/); assert.doesNotMatch(w.api.state().error, /nsec|Synthetic/); } finally { await w.dispose(); }
});
test('identity changes during recovery download revoke the URL before any click', async () => {
  const w = await imported({ changeOnDownloadUrl: true });
  try { w.api.downloadRecoveryKey(); assert.equal(w.downloads.clicked, 0); assert.equal(w.downloads.revoked.length, 1); assert.equal(w.api.state().backupAcknowledged, false); } finally { await w.dispose(); }
});
test('closed or replaced identity setup cannot export a recovery download', async () => {
  for (const stale of [w => w.stale(), w => w.f.publication.disconnectSigner()]) {
    const w = await imported();
    try { stale(w); w.api.downloadRecoveryKey(); assert.equal(w.downloads.blobs.length, 0); assert.equal(w.downloads.clicked, 0); } finally { await w.dispose(); }
  }
});


test('joining does not silently select a destination; explicit profile handoff remembers verified site without another admission', async () => {
 const w=await wizard();try{w.api.openImport();w.input.value=syntheticPrivate;w.api.importKey();w.api.acknowledge();await w.api.reviewInstance();await w.api.join();assert.equal(w.api.state().stage,'done');assert.equal(w.calls.remembered,0);assert.equal(w.calls.profileOpened,0);const signatures=w.calls.signedProofs;await w.api.openProfile();assert.equal(w.calls.remembered,1);assert.equal(w.calls.profileOpened,1);assert.equal(w.calls.signedProofs,signatures);const preferences=await w.f.publication.relayPreferences();assert.equal(preferences.relays.length,1);assert.equal(preferences.relays[0].selected,false,'profile destination handoff does not opt library into publication');}finally{await w.f.dispose();}
});
test('closing join-to-profile handoff before deferred preferences prevents remembering or opening stale profile',async()=>{
 const w=await wizard({gateAt:'preferences'});try{w.api.openImport();w.input.value=syntheticPrivate;w.api.importKey();w.api.acknowledge();await w.api.reviewInstance();await w.api.join();const opening=w.api.openProfile();await w.entered;w.destroy();w.release();await opening;assert.equal(w.calls.remembered,0);assert.equal(w.calls.profileOpened,0);}finally{w.release();await w.f.dispose();}
});

test('closing during destination verification cannot persist a late remembered site',async()=>{const w=await wizard({gateAt:'remember'});try{w.api.openImport();w.input.value=syntheticPrivate;w.api.importKey();w.api.acknowledge();await w.api.reviewInstance();await w.api.join();const opening=w.api.openProfile();await w.entered;w.destroy();w.release();await opening;assert.equal(w.calls.profileOpened,0);assert.equal((await w.f.publication.relayPreferences()).relays.length,0);}finally{w.release();await w.f.dispose();}});
