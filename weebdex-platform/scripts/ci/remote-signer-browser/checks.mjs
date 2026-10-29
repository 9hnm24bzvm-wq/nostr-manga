// Parent supplies its already-qualified sandboxed browser. This module does not
// launch a browser, disable sandboxing, install dependencies or write receipts.
import assert from 'node:assert/strict';
import { createRemoteSignerBrowserFixture } from './fixture.mjs';
export async function runOwnedRemoteSignerBrowserChecks(browser, options) {
  const report = { scope: 'isolated-current-components-real-dialog-and-owned-NIP44-transport', status: 'pending', cases: [], limitations: ['Finite native-account/publication-selection seams; not a full deployed Account journey.', 'Fixture CSS geometry; no production pixel-parity claim.', 'No real user, external signer, application-event signing or native authority change.'] };
  for (const scenario of ['cancel-late-handshake', 'escape-late-handshake', 'account-switch-late-handshake', 'link-change-late-handshake', 'successful-connect-account-disconnect']) {
    const row = { scenario, status: 'pending', phase: 'fixture', cleanup: false }; report.cases.push(row);
    let fixture, context;
    try {
      fixture = await createRemoteSignerBrowserFixture(options);
      report.source_files ??= fixture.sources;
      context = await browser.newContext({ serviceWorkers: 'block' });
      let unexpectedNetwork = 0, unexpectedPopup = 0, pageErrors = 0;
      await context.route('**/*', route => { if (new URL(route.request().url()).origin === fixture.origin) return route.continue(); unexpectedNetwork++; return route.abort(); });
      const page = await context.newPage(); page.setDefaultTimeout(4000); page.setDefaultNavigationTimeout(8000);
      page.on('pageerror', () => pageErrors++); page.on('popup', popup => { unexpectedPopup++; void popup.close(); });
      page.on('websocket', socket => { if (socket.url() !== fixture.origin.replace('http:', 'ws:') + '/relay') unexpectedNetwork++; });
      row.phase = 'open-parent'; await page.goto(fixture.origin);
      await page.getByRole('button', { name: 'Open synthetic Account connection', exact: true }).click();
      const parent = page.locator('[data-remote-parent-dialog]'); await parent.waitFor({ state: 'visible' });
      await page.getByLabel('Connection link from your trusted signer').fill(fixture.bunkerLink);
      row.phase = 'review-no-socket'; await page.getByRole('button', { name: 'Review remote signer', exact: true }).click();
      const consent = page.getByLabel('I trust this signer and approve contact with this exact relay.'); await consent.waitFor({ state: 'visible' });
      assert.equal(fixture.snapshot().connections, 0, 'Review must not open a socket.');
      assert.equal(await page.getByLabel('Connection link from your trusted signer').inputValue(), '', 'Connection token must leave the input after review.');
      await consent.check(); assert.equal(fixture.snapshot().connections, 0, 'Consent checkbox alone must not connect.');
      row.phase = 'held-handshake'; await page.getByRole('button', { name: 'Connect reviewed remote signer', exact: true }).click();
      const status = page.locator('[data-remote-signer-status]'); await status.waitFor({ state: 'visible' });
      await page.waitForFunction(() => !!document.activeElement?.closest('[data-remote-signer-status]'));
      for (let index = 0; index < 4; index++) { await page.keyboard.press('Tab'); assert.equal(await page.evaluate(() => !!document.activeElement?.closest('[data-remote-signer-status]')), true, 'Pending approval focus must remain in the top dialog.'); }
      // Wait on the owned server observation, not an arbitrary UI sleep.
      const deadline = Date.now() + 4000; while (fixture.snapshot().pending !== 1 && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10));
      assert.equal(fixture.snapshot().pending, 1, 'Expected one held authenticated public-key reply.');
      row.phase = scenario;
      if (scenario === 'cancel-late-handshake' || scenario === 'escape-late-handshake') {
        if (scenario === 'escape-late-handshake') await page.keyboard.press('Escape');
        else await status.getByRole('button', { name: 'Cancel signer request and disconnect', exact: true }).click();
        await status.waitFor({ state: 'hidden' }); fixture.release();
      } else if (scenario === 'account-switch-late-handshake') {
        fixture.switchAccount(); await page.getByText('Account: SyntheticAccountB', { exact: true }).waitFor();
        await status.waitFor({ state: 'hidden' }); fixture.release();
      } else if (scenario === 'link-change-late-handshake') {
        fixture.changeLink(); fixture.release(); await status.waitFor({ state: 'hidden' });
        await parent.getByRole('alert').waitFor();
      } else {
        fixture.release(); await status.waitFor({ state: 'hidden' });
        await page.getByText('Selected: yes', { exact: true }).waitFor();
        // Selected transport survives closing its setup view, then actual Session
        // subscription disconnects it when the finite account boundary changes.
        await parent.getByRole('button', { name: 'Close', exact: true }).click();
        await parent.waitFor({ state: 'hidden' });
        assert.equal(await page.getByText('Selected: yes', { exact: true }).count(), 1);
        fixture.switchAccount(); await page.getByText('Selected: no', { exact: true }).waitFor();
      }
      if (scenario !== 'successful-connect-account-disconnect') {
        await parent.getByRole('alert').waitFor();
        await page.getByText('Selected: no', { exact: true }).waitFor();
        // Account replacement must not run old-account focus restoration.
        if (scenario !== 'account-switch-late-handshake') await page.waitForFunction(() => !!document.activeElement?.closest('[data-remote-parent-dialog]'));
        assert.equal(fixture.snapshot().requests, scenario === 'link-change-late-handshake' ? 3 : 2, 'Handshake must not retry or sign an application event.');
      }
      const closeDeadline = Date.now() + 4000; while (fixture.snapshot().activeConnections !== 0 && Date.now() < closeDeadline) await new Promise(resolve => setTimeout(resolve, 10));
      assert.equal(fixture.snapshot().activeConnections, 0, 'Cancelled or changed account must close its actual owned transport.');
      assert.equal(await page.getByText('Selected: no', { exact: true }).count(), 1, 'No late selected identity after transport termination.');
      assert.equal(fixture.snapshot().connections, 1); assert.equal(fixture.snapshot().failures, 0);
      assert.equal(unexpectedNetwork, 0); assert.equal(unexpectedPopup, 0); assert.equal(pageErrors, 0);
      row.observation = fixture.snapshot(); row.status = 'passed'; row.phase = 'complete';
    } catch { row.status = 'failed'; /* Retain only finite phase, never raw URI/key/error bytes. */ }
    finally { try { await context?.close(); await fixture?.close(); row.cleanup = true; } catch { row.cleanup = false; } }
  }
  report.status = report.cases.every(row => row.status === 'passed' && row.cleanup) ? 'passed' : 'failed';
  return report;
}
