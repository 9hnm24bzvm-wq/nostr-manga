// Standalone browser capability probe. Importing this module never launches a browser.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const DATA_DOCUMENT = 'data:text/html;charset=utf-8,' + encodeURIComponent('<!doctype html><meta charset="utf-8"><title>Sandbox preflight</title><p id="preflight">sandbox-preflight-ok</p>');
const SANDBOX_BANNERS = Object.freeze([
  'No usable sandbox!',
  'Failed to move to new namespace:',
  'SUID sandbox helper binary',
  'Running as root without --no-sandbox is not supported',
]);

export function initialRecord() {
  return { schema: 1, stage: 'initializing', caught_stage: 'none', caught_category: 'none',
    browser_created: false, context_created: false, page_created: false,
    script_verified: false, context_closed: false, browser_closed: false, completed: false };
}
export function classify(stage, error) {
  if (stage === 'launching-browser') {
    let message;
    try { message = error?.message; } catch {}
    return typeof message === 'string' && SANDBOX_BANNERS.some(banner => message.includes(banner)) ? 'sandbox-refused' : 'launch-failed';
  }
  if (stage === 'browser-launched') return 'context-failed';
  if (stage === 'context-created') return 'page-failed';
  if (stage === 'page-created') return 'script-failed';
  if (stage === 'closing-context' || stage === 'closing-browser') return 'cleanup-failed';
  return 'worker-failed';
}
export function supportedHost(release, platform, arch, uid) {
  return typeof release === 'string' && release.length <= 16384
    && /^ID=(?:ubuntu|"ubuntu")$/m.test(release) && /^VERSION_ID=(?:22\.04|"22\.04")$/m.test(release)
    && platform === 'linux' && arch === 'x64' && Number.isSafeInteger(uid) && uid > 0;
}
export async function probe(runtime, persist = () => {}) {
  const record = initialRecord();
  let browser, context;
  const save = () => persist({ ...record });
  const transition = stage => { record.stage = stage; save(); };
  const caught = error => {
    if (record.caught_stage !== 'none') return;
    record.caught_stage = record.stage;
    record.caught_category = classify(record.stage, error);
    save();
  };
  save();
  try {
    transition('launching-browser');
    browser = await runtime.chromium.launch({ executablePath: runtime.executablePath,
      headless: true, chromiumSandbox: true, timeout: 30000 });
    record.browser_created = true;
    transition('browser-launched');
    context = await browser.newContext({ serviceWorkers: 'block', acceptDownloads: false });
    record.context_created = true;
    transition('context-created');
    // The sole document is a literal data URL. Block context network requests.
    await context.route('**/*', route => route.abort('blockedbyclient'));
    const page = await context.newPage();
    record.page_created = true;
    transition('page-created');
    await page.goto(DATA_DOCUMENT, { waitUntil: 'load', timeout: 15000 });
    if (await page.evaluate(() => document.getElementById('preflight')?.textContent) !== 'sandbox-preflight-ok') throw Error('Fixed data document script check failed');
    record.script_verified = true;
    transition('script-verified');
  } catch (error) { caught(error); }
  finally {
    if (context) {
      transition('closing-context');
      try { await context.close(); record.context_closed = true; save(); }
      catch (error) { caught(error); }
    }
    if (browser) {
      transition('closing-browser');
      try { await browser.close(); record.browser_closed = true; save(); }
      catch (error) { caught(error); }
    }
    record.completed = record.caught_stage === 'none' && record.script_verified && record.context_closed && record.browser_closed;
    transition('complete');
  }
  return record;
}

async function main() {
  if (process.argv.length !== 2) throw Error('No probe arguments accepted');
  const directory = path.join(ROOT, '.build/sandbox-preflight');
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  const output = path.join(directory, 'worker.json');
  const fd = fs.openSync(output, fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_WRONLY | fs.constants.O_NOFOLLOW, 0o600);
  const persist = value => {
    const bytes = Buffer.from(JSON.stringify(value) + '\n');
    if (bytes.length > 65536) throw Error('Closed worker record exceeded budget');
    fs.ftruncateSync(fd, 0); fs.writeSync(fd, bytes, 0, bytes.length, 0); fs.fsyncSync(fd);
  };
  try {
    if (!supportedHost(fs.readFileSync('/etc/os-release', 'utf8'), process.platform, process.arch, process.getuid?.())) throw Error('Exact nonroot Ubuntu22.04 host required');
    const runtimePath = path.join(ROOT, '.build/hosted-browser/runtime.json');
    if (fs.realpathSync(runtimePath) !== runtimePath || !fs.lstatSync(runtimePath).isFile() || fs.statSync(runtimePath).size > 2 * 1024 * 1024) throw Error('Explicit hosted runtime generation required');
    const { loadBrowserRuntime } = await import('./browser-runtime.mjs');
    const runtime = loadBrowserRuntime();
    const record = await probe(runtime, persist);
    process.exitCode = record.completed ? 0 : 1;
  } catch {
    const value = initialRecord(); value.caught_stage = 'initializing'; value.caught_category = 'worker-failed'; value.stage = 'complete'; persist(value); process.exitCode = 1;
  } finally { fs.closeSync(fd); }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(() => { process.exitCode = 1; });
}
