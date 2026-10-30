// Source-only until runtime release. This starts no browser and enters no secret.
import readline from 'node:readline';
import { createIndependentNakFixture, MAX_SECONDS } from './fixture.mjs';
let fixture, input, deadline, closing = false;
async function close() {
  if (closing) return; closing = true; clearTimeout(deadline); input?.close();
  try { const result = await fixture?.close(); if (result) process.stdout.write(JSON.stringify({ status: 'closed', ...result }) + '\n'); }
  catch { process.stderr.write('Owned independent signer cleanup requires inspection.\n'); process.exitCode = 1; }
}
process.once('SIGINT', () => { void close(); }); process.once('SIGTERM', () => { void close(); });
try {
  if (process.env.WEEBDEX_INDEPENDENT_NAK_APPROVED !== 'yes' || !process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES) throw Error();
  const portText = process.env.WEEBDEX_INDEPENDENT_NAK_PORT;
  if (portText !== undefined && !/^[1-9][0-9]{3,4}$/.test(portText)) throw Error();
  fixture = await createIndependentNakFixture({ approved: true, domDependencies: process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES, relayPort: portText === undefined ? undefined : Number(portText) });
  if (closing) await fixture.close();
  else {
    process.stdout.write(JSON.stringify({ status: 'independent-signer-ready', relayOrigin: fixture.relayOrigin, privateConnectionFile: fixture.privateConnectionFile, maximumLifetimeSeconds: MAX_SECONDS, expiresAt: new Date(fixture.expiresAt).toISOString(), browserOriginBound: false }) + '\n');
    input = readline.createInterface({ input: process.stdin });
    deadline = setTimeout(() => { void close(); }, Math.max(0, fixture.expiresAt - Date.now()));
    input.on('line', line => {
      try {
        if (line.length > 256) throw Error();
        const value = JSON.parse(line);
        if (value && Object.keys(value).join(',') === 'browserOrigin') { fixture.setBrowserOrigin(value.browserOrigin); process.stdout.write(JSON.stringify({ status: 'browser-origin-bound', ...fixture.snapshot() }) + '\n'); }
        else if (value && Object.keys(value).join(',') === 'stop' && value.stop === true) { input.close(); void close(); }
        else throw Error();
      } catch { process.stderr.write('Owned independent signer command refused.\n'); }
    });
    input.on('close', () => { void close(); });
  }
} catch (error) { process.stderr.write('Independent signer setup refused or failed; no browser was launched.\n'); if (error?.receipt) process.stdout.write(JSON.stringify({ status: 'failed-with-cleanup', ...error.receipt }) + '\n'); await close(); process.exitCode = 1; }
