// Launches only the bounded owned loopback fixture, never a browser.
import { createRemoteSignerBrowserFixture } from './fixture.mjs';
let fixture, timer, closing = false;
async function close() { if (closing) return; closing = true; clearTimeout(timer); try { await fixture?.close(); process.exitCode = 0; } catch { process.exitCode = 1; } }
process.once('SIGINT', () => { void close(); }); process.once('SIGTERM', () => { void close(); });
try {
  if (!process.env.WEEBDEX_FRONTEND_ROOT || !process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES) throw Error();
  fixture = await createRemoteSignerBrowserFixture({ frontend: process.env.WEEBDEX_FRONTEND_ROOT, domDependencies: process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES });
  if (closing) await fixture.close();
  else {
    // Only public loopback routes are printed. No keys, events or connection URI.
    process.stdout.write(JSON.stringify({ status: 'owned-synthetic-ready', page: fixture.origin + '/', controls: fixture.origin + '/controls', observations: fixture.origin + '/fixture/observations', maximum_lifetime_seconds: 900 }) + '\n');
    timer = setTimeout(() => { void close(); }, 900000);
  }
} catch { process.stderr.write('Owned remote-signer fixture preparation failed; no browser was launched.\n'); await close(); process.exitCode = 1; }
