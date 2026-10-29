// Explicit clipboard/locator stand-ins execute the actual helper. No browser,
// key generation, permissions or clipboard access occurs in this finite suite.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import {exerciseGeneratedKeyBackup} from '../../scripts/browser-identity-key-controls.mjs';

const publicKey = 'npub1' + 'q'.repeat(58), secret = 'nsec1' + 'p'.repeat(58);
function fixture(failAt) {
  const calls = [], redactions = new Set(), listeners = new Map(), opened = new Set();
  let clipboard = '', revealed = false, generated = false, mutation = false;
  function action(name) { calls.push(name); if (name === failAt) throw Error('Synthetic boundary refused'); }
  const setup = {
    async count() { return 1; },
    getByText(name, {exact}) { assert.equal(exact, true); return {label: name, waitFor: async () => action(name)}; },
    locator(selector) {
      assert.equal(selector, 'details');
      return { filter({has}) {
        const label = has.label;
        assert(['Advanced: create or import a private key', 'Public identity details'].includes(label));
        return {
          count: async () => 1,
          waitFor: async () => action('disclosure-visible:' + label),
          getAttribute: async name => { assert.equal(name, 'open'); return opened.has(label) ? '' : null; },
          locator(child) { assert.equal(child, ':scope > summary'); return { click: async () => { action('Open ' + label); opened.add(label); } }; },
        };
      } };
    },
    getByRole(role, {name, exact}) {
      assert.equal(exact, true);
      return {
        async click() {
          action(name);
          if (name === 'Create a new public identity') { generated = true; if (failAt === 'create-ack-lost') throw Error('Click rejected after draft creation'); }
          if (name === 'Reveal private key') revealed = true;
          if (name === 'Hide private key') revealed = false;
          if (name === 'Copy public key') clipboard = publicKey;
          if (name === 'Copy private key') clipboard = failAt === 'empty-private-copy' ? '' : failAt === 'wrong-private-copy' ? 'different synthetic value' : secret;
          if (name === 'Discard setup and go back') { if (!generated) throw Error('Discard is not mounted'); generated = false; revealed = false; }
          if (failAt === 'unexpected-mutation' && name === 'Create a new public identity') listeners.get('request')({method: () => 'POST'});
        },
        async waitFor() { action(name); if (role === 'heading' && name === 'Set up your public identity') assert.equal(generated, false, 'The generated draft must really be gone'); },
        async isDisabled() { return failAt !== 'premature-review'; }
      };
    },
    getByLabel(name, {exact}) {
      assert.equal(exact, true);
      return {count: async () => name === 'Private key for backup' ? Number(revealed) : 1,
        inputValue: async () => name === 'Your public key' ? publicKey : failAt === 'empty-private-key' ? '' : secret};
    }
  };
  const page = {
    getByText(name, options) { return setup.getByText(name, options); },
    url() {return 'http://127.0.0.1:8123/settings/identity';},
    locator(selector) { if(selector==='[data-account-settings]')return {count:async()=>1};assert.equal(selector, '[role="dialog"] [data-identity-setup]'); return setup; },
    on(name, listener) { assert(!listeners.has(name)); listeners.set(name, listener); },
    off(name, listener) { assert.equal(listeners.get(name), listener); listeners.delete(name); },
    async evaluate(operation) {
      const read = operation.toString().includes('readText');
      action(read ? 'clipboard-read' : 'clipboard-clear');
      if (read) return clipboard;
      clipboard = '';
    }
  };
  const context = {
    async grantPermissions(values, scope) { assert.deepEqual(values, ['clipboard-read', 'clipboard-write']); assert.deepEqual(scope, {origin: 'http://127.0.0.1:8123'}); action('grant'); },
    async clearPermissions() { action('clear-permissions'); }
  };
  return {page, context, redactions, syntheticKeyExerciseApproved: true, origin: 'http://127.0.0.1:8123', calls, state: () => ({clipboard, revealed, generated, listenerCount: listeners.size})};
}

test('actual helper verifies real-control order and emits only booleans/counts', async () => {
  const f = fixture(), value = await exerciseGeneratedKeyBackup(f);
  assert.equal(value.private_clipboard_matches, true);
  assert.equal(value.generated_draft_discarded, true);
  assert.equal(value.mutation_requests, 0);
  assert.deepEqual(f.state(), {clipboard: '', revealed: false, generated: false, listenerCount: 0});
  assert(f.redactions.has(secret));
  assert.doesNotMatch(JSON.stringify(value), /npub1|nsec1/);
  assert(f.calls.indexOf('Open Advanced: create or import a private key') < f.calls.indexOf('Create a new public identity'));
  assert(f.calls.indexOf('Open Public identity details') < f.calls.indexOf('Copy public key'));
  assert(f.calls.indexOf('Create a new public identity') < f.calls.indexOf('Copy public key'));
  assert(f.calls.indexOf('Reveal private key') < f.calls.indexOf('Copy private key'));
  assert(f.calls.indexOf('Hide private key') < f.calls.indexOf('Discard setup and go back'));
});
for (const failAt of ['grant', 'Open Advanced: create or import a private key', 'Open Public identity details', 'Create a new public identity', 'create-ack-lost', 'Copy public key', 'Public key copied.', 'Reveal private key', 'Copy private key', 'Private key copied. Save it somewhere safe; clipboard history or sync may keep a copy.', 'wrong-private-copy', 'empty-private-key', 'empty-private-copy', 'premature-review', 'unexpected-mutation']) {
  test('actual helper refuses and releases owned context after ' + failAt, async () => {
    const f = fixture(failAt);
    await assert.rejects(exerciseGeneratedKeyBackup(f));
    assert.equal(f.state().listenerCount, 0);
    assert.equal(f.state().clipboard, '');
    assert.equal(f.state().generated, false);
    assert(f.calls.includes('clear-permissions'));
    assert.equal(f.redactions.has(''), false, 'An empty clipboard or input must not register an empty error-redaction token');
  });
}
for (const failAt of ['Discard setup and go back', 'clear-permissions']) test('cleanup uncertainty cannot become a passing helper: ' + failAt, async () => {
  const f = fixture(failAt); await assert.rejects(exerciseGeneratedKeyBackup(f), /cleanup must complete/);
  assert.equal(f.state().listenerCount, 0);
});
test('ordinary signup has no key exercise; explicit advanced helper uses actual product controls', () => {
  const journey = fs.readFileSync(new URL('../../scripts/browser-journeys.mjs', import.meta.url), 'utf8');
  const component = fs.readFileSync(new URL('../../platform-integrations/local-library/IdentitySetup.svelte', import.meta.url), 'utf8');
  assert.doesNotMatch(journey, /exerciseGeneratedKeyBackup|Finish later|public_identity_deferred_by_ui/);
  assert.match(journey, /name: "Continue reading", exact: true/);
  assert.match(journey, /Ordinary account signup must not require portable identity setup/);
  const onboarding = fs.readFileSync(new URL('../../platform-integrations/frontend-behavior/AccountOnboarding.svelte', import.meta.url), 'utf8');
  assert.doesNotMatch(onboarding, /IdentitySetup|generateLocalKeyDraft|connectSigner/);
  assert.match(onboarding, />Continue reading<\/Button>/);
  for (const label of ['Advanced: create or import a private key', 'Public identity details', 'Continue to site access']) assert(component.includes(label), label);
  for (const name of ['Create a new public identity', 'Copy public key', 'Copy private key', 'Reveal private key', 'Hide private key', 'Discard setup and go back']) assert(component.includes(name), name);
  assert.match(component, /navigator\.clipboard\.writeText\(draft\.exportSecret\(\)\)/);
  assert.match(component, /navigator\.clipboard\.writeText\(identity\.npub\)/);
});

test('advanced key helper refuses missing explicit synthetic approval before touching UI or permissions',async()=>{const f=fixture();await assert.rejects(exerciseGeneratedKeyBackup({...f,syntheticKeyExerciseApproved:false}),/Explicit approval/);assert.deepEqual(f.calls,[]);});
test('advanced key helper refuses ordinary signup before key generation or clipboard permission',async()=>{const f=fixture();f.page.url=()=>f.origin+'/auth/register';await assert.rejects(exerciseGeneratedKeyBackup(f),/only to Account settings/);assert.deepEqual(f.calls,[]);});
