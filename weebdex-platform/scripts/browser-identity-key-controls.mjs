// Genuine UI controls for a fresh synthetic browser context. Never return a key.
import assert from 'node:assert/strict';
import { expandDetailsPanel } from './browser-details-controls.mjs';

export async function exerciseGeneratedKeyBackup({page, context, origin, redactions, syntheticKeyExerciseApproved = false}) {
  assert.equal(syntheticKeyExerciseApproved, true, 'Explicit approval for this synthetic advanced-key exercise is required');
  const target = new URL(origin);
  assert.ok(target.protocol === 'http:' && ['127.0.0.1', 'localhost'].includes(target.hostname) && target.origin === origin, 'Exact owned loopback origin required');
  assert.ok(redactions instanceof Set, 'The final browser error serializer must retain synthetic-key redactions');
  assert.equal(new URL(page.url()).origin, origin, 'Advanced-key exercise must remain on the owned site');
  assert.equal(new URL(page.url()).pathname, '/settings/identity', 'Advanced-key exercise belongs only to Account settings');
  assert.equal(await page.locator('[data-account-settings]').count(), 1, 'Expected exactly one Account settings surface');
  const setup = page.locator('[role="dialog"] [data-identity-setup]');
  assert.equal(await setup.count(), 1, 'Expected exactly one explicitly opened advanced identity dialog');
  const openDetails = async label => {
    const details = setup.locator('details').filter({has: page.getByText(label, {exact: true})});
    assert.equal(await details.count(), 1, 'Expected exactly one named advanced disclosure');
    await expandDetailsPanel(details);
  };
  let permissionAttempted = false, permission = false, creationAttempted = false, primary, secret = '', publicKey = '', result;
  const cleanupErrors = [];
  let mutations = 0;
  const request = value => { if (!['GET', 'HEAD', 'OPTIONS'].includes(value.method())) mutations++; };
  page.on('request', request);
  try {
    permissionAttempted = true;
    await context.grantPermissions(['clipboard-read', 'clipboard-write'], {origin});
    permission = true;
    await page.evaluate(() => navigator.clipboard.writeText(''));
    await openDetails('Advanced: create or import a private key');
    creationAttempted = true;
    await setup.getByRole('button', {name: 'Create a new public identity', exact: true}).click();
    await setup.getByRole('heading', {name: 'Keep your key safe', exact: true}).waitFor({state: 'visible'});
    assert.equal(await setup.getByLabel('Private key for backup', {exact: true}).count(), 0, 'Generated private key must start concealed');
    assert.ok(await setup.getByRole('button', {name: 'Continue to site access', exact: true}).isDisabled(), 'Backup acknowledgement must precede admission review');
    await openDetails('Public identity details');
    publicKey = await setup.getByLabel('Your public key', {exact: true}).inputValue();
    assert.ok(/^npub1[023456789acdefghjklmnpqrstuvwxyz]{58}$/.test(publicKey), 'Generated public key must use npub encoding');
    await setup.getByRole('button', {name: 'Copy public key', exact: true}).click();
    await setup.getByText('Public key copied.', {exact: true}).waitFor({state: 'visible'});
    assert.ok(await page.evaluate(() => navigator.clipboard.readText()) === publicKey, 'Public-key clipboard must equal the actual displayed key');
    await setup.getByRole('button', {name: 'Reveal private key', exact: true}).click();
    secret = await setup.getByLabel('Private key for backup', {exact: true}).inputValue();
    if (secret.length) redactions.add(secret); // Register before validation; an empty token would corrupt the final serializer.
    assert.ok(/^nsec1[023456789acdefghjklmnpqrstuvwxyz]{58}$/.test(secret), 'Generated backup must use private nsec encoding');
    await setup.getByRole('button', {name: 'Copy private key', exact: true}).click();
    await setup.getByText('Private key copied. Save it somewhere safe; clipboard history or sync may keep a copy.', {exact: true}).waitFor({state: 'visible'});
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    if (copied.length) redactions.add(copied);
    assert.ok(copied === secret, 'Private-key clipboard must equal the actual revealed key');
    await setup.getByRole('button', {name: 'Hide private key', exact: true}).click();
    assert.equal(await setup.getByLabel('Private key for backup', {exact: true}).count(), 0, 'Hide must remove the revealed private key');
    assert.equal(mutations, 0, 'Generate/copy must not mutate a server or publish an identity');
    result = {generated_via_ui: true, public_clipboard_matches: true, private_clipboard_matches: true, backup_gate_preserved: true, private_reconcealed: true, mutation_requests: mutations};
  } catch (error) { primary = error; }
  finally {
    if (creationAttempted) try {
      // A click can run its handler before its acknowledgement rejects. Always
      // attempt bounded discard; an already-choose state is also confirmed.
      try { await setup.getByRole('button', {name: 'Discard setup and go back', exact: true}).click({timeout: 3000}); }
      catch { /* Confirm the empty choose stage below, never infer discard. */ }
      await setup.getByRole('heading', {name: 'Set up your public identity', exact: true}).waitFor({state: 'visible', timeout: 1000});
    } catch { cleanupErrors.push('Generated draft discard was not confirmed'); }
    if (permission) {
      try { await page.evaluate(() => navigator.clipboard.writeText('')); }
      catch { cleanupErrors.push('Synthetic-key clipboard cleanup was not confirmed'); }
    }
    if (permissionAttempted) try { await context.clearPermissions(); }
    catch { cleanupErrors.push('Fresh-context permission cleanup was not confirmed'); }
    page.off('request', request);
    secret = ''; publicKey = '';
  }
  if (primary && cleanupErrors.length) throw new AggregateError([primary, ...cleanupErrors.map(message => Error(message))], 'Key backup failed and cleanup was not confirmed');
  if (primary) throw primary;
  assert.deepEqual(cleanupErrors, [], 'Key-backup cleanup must complete before accepting the journey');
  return {...result, generated_draft_discarded: true, clipboard_cleared: true, permissions_cleared: true};
}
