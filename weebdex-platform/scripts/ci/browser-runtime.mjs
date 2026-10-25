// Narrow portable dependency seam. Importing this file starts no browser.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

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
const qualifiedRoot = path.join(root, '../.ci/native-upload-counterfactual/node_modules');
const qualifiedExecutable = path.join(root, '.build/hosted-browser/cft-acquisition/157.0.8103.6/chrome-linux64/chrome');
const qualifiedHash = 'ec4e78d65a5382c2b50aaf28c6a655a624973b3d3c711a2a4ae34369fd9a3444';
export function runtimeProfile(value = 'bundled') {
  if (!['bundled', 'cft-157'].includes(value)) throw Error('Unreviewed browser runtime profile');
  return value;
}
function verifyQualified(mode) {
  execFileSync('python3', ['-B', path.join(root, 'scripts/ci/verify-chrome157-runtime.py'), mode], { timeout: 30000, stdio: 'pipe', maxBuffer: 65536 });
}

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
function packageFiles(directoryRoot = hostedRoot) {
  const rows = [];
  for (const packageName of ['playwright', 'playwright-core']) {
    const visit = directory => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
        const file = path.join(directory, entry.name);
        if (entry.isDirectory()) visit(file);
        else {
          const bytes = regular(file);
          rows.push({ path: path.relative(directoryRoot, file), bytes: bytes.length, sha256: sha(bytes) });
        }
      }
    };
    visit(path.join(directoryRoot, packageName));
  }
  if (!rows.length || rows.length > 4096 || rows.reduce((sum, row) => sum + row.bytes, 0) > 64 * 1024 * 1024) throw Error('Hosted browser dependency budget');
  return rows.sort((a, b) => a.path.localeCompare(b.path));
}
export function verifyHostedPins(profile = runtimeProfile(process.env.BROWSER_RUNTIME_PROFILE)) {
  runtimeProfile(profile);
  if (profile === 'cft-157') { verifyQualified('--driver-only'); return; }
  if (sha(regular(path.join(root, '../.ci/browser/package-lock.json'))) !== toolsLockPin) throw Error('Hosted tools lock changed');
  verifyPlaywright(path.join(hostedRoot, 'playwright'));
  if (sha(regular(path.join(hostedRoot, 'playwright-core/package.json'))) !== '07c47543631fef9508760365dee9fbe958c562093ec8d122543949ed231f233f') throw Error('Reviewed Playwright core identity changed');
  if (sha(regular(path.join(hostedRoot, 'playwright-core/browsers.json'))) !== coreBrowsersPin) throw Error('Official Chromium revision changed');
}
export function recordHostedRuntime() {
  const profile = runtimeProfile(process.env.BROWSER_RUNTIME_PROFILE);
  verifyHostedPins(profile);
  const qualified = profile === 'cft-157', directory = qualified ? qualifiedRoot : hostedRoot;
  if (qualified) verifyQualified('--full');
  const { chromium } = require(path.join(directory, 'playwright/index.js'));
  const executable = fs.realpathSync(qualified ? qualifiedExecutable : chromium.executablePath());
  const value = { schema: 1, ...(qualified ? { profile } : {}), playwright: qualified ? '1.64.1' : '1.62.1', chromium_revision: qualified ? 'cft-157.0.8103.6' : '1234', executable,
    executable_sha256: sha(regular(executable)), package_files: packageFiles(directory), npm_hooks_executed: false };
  fs.writeFileSync(runtimeFile, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 });
  return value;
}
export function loadBrowserRuntime() {
  if (!fs.existsSync(runtimeFile)) {
    if (runtimeProfile(process.env.BROWSER_RUNTIME_PROFILE) !== 'bundled') throw Error('Explicit qualified runtime is unavailable');
    const packageRoot = '/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright';
    verifyPlaywright(packageRoot);
    return { chromium: require(path.join(packageRoot, 'index.js')).chromium, executablePath: '/usr/bin/chromium', description: 'system Chromium via reviewed Playwright 1.62.1' };
  }
  const runtime = JSON.parse(regular(runtimeFile));
  const profile = runtimeProfile(runtime.profile), qualified = profile === 'cft-157';
  if (process.env.BROWSER_RUNTIME_PROFILE !== undefined && runtimeProfile(process.env.BROWSER_RUNTIME_PROFILE) !== profile) throw Error('Selected browser runtime changed');
  verifyHostedPins(profile);
  if (qualified) verifyQualified('--full');
  const directory = qualified ? qualifiedRoot : hostedRoot;
  const { chromium } = require(path.join(directory, 'playwright/index.js'));
  const executable = fs.realpathSync(qualified ? qualifiedExecutable : chromium.executablePath());
  if (runtime.schema !== 1 || runtime.playwright !== (qualified ? '1.64.1' : '1.62.1') || runtime.chromium_revision !== (qualified ? 'cft-157.0.8103.6' : '1234') || runtime.npm_hooks_executed !== false
      || runtime.executable !== executable || sha(regular(executable)) !== runtime.executable_sha256
      || (qualified && runtime.executable_sha256 !== qualifiedHash)
      || JSON.stringify(packageFiles(directory)) !== JSON.stringify(runtime.package_files)) throw Error('Hosted browser runtime generation changed');
  return { chromium, executablePath: executable, description: qualified ? 'qualified Chrome for Testing 157.0.8103.6 via reviewed Playwright 1.64.1' : 'bundled Chromium revision 1234 via reviewed Playwright 1.62.1' };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3 || !['--verify-hosted', '--record-hosted'].includes(process.argv[2])) throw Error('Use --verify-hosted before the official CLI, or --record-hosted after browser installation');
  if (process.argv[2] === '--verify-hosted') verifyHostedPins();
  else recordHostedRuntime();
}
