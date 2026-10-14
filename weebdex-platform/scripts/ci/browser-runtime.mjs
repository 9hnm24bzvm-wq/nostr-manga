// Narrow portable dependency seam. Importing this file starts no browser.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const require = createRequire(import.meta.url);
const sha = value => createHash('sha256').update(value).digest('hex');
const installedPins = Object.freeze({
  'package.json': 'ca170ec143a88ed3043ac953eb3b2377b2b97304104f4e1e23316684ce2c35af',
  'index.js': '4e98f65f0a9d9bcab8cffc0c5cfdd87fd3f5b0be74fc494ff166a466b204cd44',
  'cli.js': '5e79302a6aad7d835d92d5ae6b4244c91cf61b28e94d7917279090e6c231f7a1'
});
const coreBrowsersPin = 'f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99';
const toolsLockPin = 'b117dbd1c52f362161dfd20bb6bae12145db37b2ebf0c63004dc738363738d7d';
const hostedRoot = path.join(root, '../.ci/browser/node_modules');
const runtimeFile = path.join(root, '.build/hosted-browser/runtime.json');

function regular(file) {
  if (fs.realpathSync(file) !== file || !fs.lstatSync(file).isFile()) throw Error('Unreviewed dependency link or nonregular file');
  return fs.readFileSync(file);
}
function verifyPlaywright(packageRoot) {
  for (const [file, pin] of Object.entries(installedPins)) {
    if (sha(regular(path.join(packageRoot, file))) !== pin) throw Error('Reviewed Playwright source changed');
  }
  const pkg = JSON.parse(regular(path.join(packageRoot, 'package.json')));
  if (pkg.name !== 'playwright' || pkg.version !== '1.62.1' || pkg.repository?.url !== 'git+https://github.com/microsoft/playwright.git') throw Error('Unrecognized official browser package');
}
function packageFiles() {
  const rows = [];
  for (const packageName of ['playwright', 'playwright-core']) {
    const visit = directory => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
        const file = path.join(directory, entry.name);
        if (entry.isDirectory()) visit(file);
        else {
          const bytes = regular(file);
          rows.push({ path: path.relative(hostedRoot, file), bytes: bytes.length, sha256: sha(bytes) });
        }
      }
    };
    visit(path.join(hostedRoot, packageName));
  }
  if (!rows.length || rows.length > 4096 || rows.reduce((sum, row) => sum + row.bytes, 0) > 64 * 1024 * 1024) throw Error('Hosted browser dependency budget');
  return rows.sort((a, b) => a.path.localeCompare(b.path));
}
export function verifyHostedPins() {
  if (sha(regular(path.join(root, '../.ci/browser/package-lock.json'))) !== toolsLockPin) throw Error('Hosted tools lock changed');
  verifyPlaywright(path.join(hostedRoot, 'playwright'));
  if (sha(regular(path.join(hostedRoot, 'playwright-core/package.json'))) !== '07c47543631fef9508760365dee9fbe958c562093ec8d122543949ed231f233f') throw Error('Reviewed Playwright core identity changed');
  if (sha(regular(path.join(hostedRoot, 'playwright-core/browsers.json'))) !== coreBrowsersPin) throw Error('Official Chromium revision changed');
}
export function recordHostedRuntime() {
  verifyHostedPins();
  const { chromium } = require(path.join(hostedRoot, 'playwright/index.js'));
  const executable = fs.realpathSync(chromium.executablePath());
  const value = { schema: 1, playwright: '1.62.1', chromium_revision: '1234', executable,
    executable_sha256: sha(regular(executable)), package_files: packageFiles(), npm_hooks_executed: false };
  fs.writeFileSync(runtimeFile, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 });
  return value;
}
export function loadBrowserRuntime() {
  if (!fs.existsSync(runtimeFile)) {
    const packageRoot = '/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright';
    verifyPlaywright(packageRoot);
    return { chromium: require(path.join(packageRoot, 'index.js')).chromium, executablePath: '/usr/bin/chromium', description: 'system Chromium via reviewed Playwright 1.62.1' };
  }
  verifyHostedPins();
  const runtime = JSON.parse(regular(runtimeFile));
  const { chromium } = require(path.join(hostedRoot, 'playwright/index.js'));
  const executable = fs.realpathSync(chromium.executablePath());
  if (runtime.schema !== 1 || runtime.playwright !== '1.62.1' || runtime.chromium_revision !== '1234' || runtime.npm_hooks_executed !== false
      || runtime.executable !== executable || sha(regular(executable)) !== runtime.executable_sha256
      || JSON.stringify(packageFiles()) !== JSON.stringify(runtime.package_files)) throw Error('Hosted browser runtime generation changed');
  return { chromium, executablePath: executable, description: 'bundled Chromium revision 1234 via reviewed Playwright 1.62.1' };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3 || !['--verify-hosted', '--record-hosted'].includes(process.argv[2])) throw Error('Use --verify-hosted before the official CLI, or --record-hosted after browser installation');
  if (process.argv[2] === '--verify-hosted') verifyHostedPins();
  else recordHostedRuntime();
}
