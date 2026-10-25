// Fixed diagnostic identities only. The values inspected by these checks,
// including mail links and passwords, must never enter their public receipt.
export const accountRecoveryCheckIds = Object.freeze([
  'device-library-seeded', 'logout-before-recovery', 'recovery-mail-delivered',
  'mail-link-local', 'reset-ui-mounted', 'reset-route', 'restricted-capability',
  'password-only-controls', 'private-library-denied', 'device-library-retained',
  'password-reset-accepted', 'reset-completion-visible', 'normal-session-confirmed',
  'account-settings-reached', 'old-password-rejected', 'new-password-authenticated',
  'device-library-retained-after-reset', 'recovery-link-replay-rejected',
]);

export function createAccountRecoveryChecks(report, save = () => {}) {
  const rows = accountRecoveryCheckIds.map(check_id => ({ check_id, status: 'pending' }));
  report.account_recovery_checks = rows;
  let next = 0;
  save();
  return async (checkId, action) => {
    const row = rows[next];
    if (!row || row.check_id !== checkId || row.status !== 'pending') throw Error('Account recovery check order changed');
    row.status = 'running'; save();
    try {
      const result = await action();
      row.status = 'passed'; next++; save();
      return result;
    } catch (cause) {
      row.status = 'failed'; save();
      throw cause;
    }
  };
}

export async function submitRecoveryPassword(page, password) {
  // A missing prerequisite input must not create an abandoned response waiter.
  await page.locator('input[name="password"]').fill(password);
  // Both branches have a rejection handler immediately. If clicking fails,
  // a later response timeout cannot terminate the runner as an unhandled rejection.
  const [response] = await Promise.all([
    page.waitForResponse(response => new URL(response.url()).pathname === '/auth/self-service/settings' && response.request().method() === 'POST'),
    page.getByRole('button', { name: 'Reset Password', exact: true }).click(),
  ]);
  return response;
}
