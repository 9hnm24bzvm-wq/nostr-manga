// Fixed, credential-free import progress and actual UI controls. No HTTP adapter,
// signer override, enrollment request, storage inspection or secret serialization.
import assert from 'node:assert/strict';

export const keyImportSubsteps = Object.freeze([
  'open-settings', 'settings-route', 'public-identity', 'identity-panel',
  'key-dialog', 'import-form', 'import-key', 'backup-ack', 'recipient-review',
  'session-connect', 'session-ready', 'done', 'upload-return', 'upload-ready',
]);
export const keyImportCategories = Object.freeze([
  'none', 'readiness-timeout', 'duplicate-control', 'browser-timeout', 'assertion', 'unexpected',
]);
export function keyImportFailureCategory(error) {
  let code, name;
  try { code = error?.code; name = error?.name; } catch { return 'unexpected'; }
  if (code === 'UPLOAD_UI_READINESS_TIMEOUT') return 'readiness-timeout';
  if (code === 'UPLOAD_UI_DUPLICATE_CONTROL') return 'duplicate-control';
  if (name === 'TimeoutError') return 'browser-timeout';
  if (code === 'ERR_ASSERTION' || name === 'AssertionError') return 'assertion';
  return 'unexpected';
}
export function createKeyImportStep(attempts, save) {
  assert.ok(Array.isArray(attempts) && attempts.length < 2, 'At most two explicit key imports');
  assert.ok(attempts.every(row => row.status === 'passed'), 'A failed import cannot be followed by another import');
  const row = { attempt: attempts.length + 1, substep: keyImportSubsteps[0], completed_substeps: 0, status: 'running', caught_category: 'none' };
  attempts.push(row);
  return async (substep, action) => {
    const index = row.completed_substeps;
    assert.equal(substep, keyImportSubsteps[index], 'Import substeps must follow their fixed sequence');
    assert.equal(row.status, 'running', 'A terminal import cannot continue');
    row.substep = substep;
    try {
      save(); const result = await action();
      row.completed_substeps = index + 1;
      if (row.completed_substeps === keyImportSubsteps.length) row.status = 'passed';
      save(); return result;
    } catch (error) {
      row.completed_substeps = index; row.status = 'failed'; row.caught_category = keyImportFailureCategory(error);
      try { save(); } catch { /* Preserve the original failure. */ }
      throw error;
    }
  };
}
export async function uniqueUploadControl(locator, until) {
  await until(async () => {
    const count = await locator.count();
    assert.ok(Number.isSafeInteger(count) && count >= 0, 'Invalid control count');
    if (count > 1) throw Object.assign(Error('Duplicate upload control'), { code: 'UPLOAD_UI_DUPLICATE_CONTROL' });
    return count;
  }, count => count === 1, 'Unique upload control attached');
  assert.equal(await locator.count(), 1, 'Upload control changed after readiness');
  return locator;
}
export async function connectPublicUploadKey({ page, unique, button, until, assertUploadRoute, secretHex, importStep }) {
  let before;
  await importStep('open-settings', async () => {
    before = await page.evaluate(() => performance.timeOrigin);
    await (await unique(page.getByRole('link', { name: 'Open Settings', exact: true }))).click();
  });
  await importStep('settings-route', () => until(() => new URL(page.url()).pathname, value => value === '/settings/preferences', 'Settings replacement navigation settled'));
  await importStep('public-identity', async () => { await (await unique(page.getByRole('link', { name: 'Public identity', exact: true }))).click(); });
  const identity = page.locator('[data-protocol-identity-panel]');
  await importStep('identity-panel', async () => { await (await button(identity, 'Create or paste a key')).click(); });
  const setup = page.locator('[data-identity-setup]');
  await importStep('key-dialog', async () => { await (await button(setup, 'Paste an existing key')).click(); });
  await importStep('import-form', async () => {
    const input = await unique(setup.getByLabel('Private key (nsec or hex)', { exact: true }));
    assert.equal(await input.getAttribute('type'), 'password'); await input.fill(secretHex);
  });
  await importStep('import-key', async () => { await (await button(setup, 'Use this key')).click(); });
  await importStep('backup-ack', async () => { await setup.getByRole('checkbox', { name: 'I have saved my private key somewhere safe.', exact: true }).check(); });
  await importStep('recipient-review', async () => {
    await (await button(setup, 'Review public identity')).click();
    await until(async () => {
      assert.equal(await setup.getByRole('alert').count(), 0, 'Verified recipient review failed');
      const count = await setup.getByRole('heading', { name: 'Instance approval needed', exact: true }).count();
      assert.ok(count <= 1, 'Duplicate verified recipient review heading');
      return count;
    }, count => count === 1, 'Actual closed-enrollment recipient review ready');
  });
  // This fixture is operator-admitted and deliberately has closed self-enrollment.
  // Use the real reviewed local-session control; it grants no server authority.
  await importStep('session-connect', async () => { await (await button(setup, 'Use this key for this session')).click(); });
  await importStep('session-ready', () => until(() => setup.getByRole('heading', { name: 'Key connected for this session', exact: true }).count(), count => count === 1, 'Local session connection confirmed'));
  await importStep('done', async () => { await (await button(setup, 'Done')).click(); });
  await importStep('upload-return', async () => {
    await until(async () => await identity.getByText(/^Session private key connected\./).count() === 1 && await identity.getByRole('button', { name: 'Lock private key', exact: true }).isEnabled(), Boolean, 'Actual imported identity ready');
    assert.equal(new URL(page.url()).pathname, '/settings/identity');
    await page.goBack({ waitUntil: 'domcontentloaded' }); await page.goBack({ waitUntil: 'domcontentloaded' });
    assertUploadRoute(); assert.equal(await page.evaluate(() => performance.timeOrigin), before, 'Expected same-document Settings navigation');
  });
  await importStep('upload-ready', () => until(() => page.getByRole('heading', { name: 'Upload Chapter', exact: true }).count(), count => count === 1, 'Bound upload editor ready'));
}
