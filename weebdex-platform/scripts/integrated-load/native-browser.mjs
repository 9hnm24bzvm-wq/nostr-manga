// Real Chromium/application/IndexedDB probe. Importing this module is inert.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { loadBrowserRuntime } from '../ci/browser-runtime.mjs';
import { selectListboxTrigger, clickListboxOption, selectTitleLibraryStatus } from '../browser-select-controls.mjs';
import { Credits, Refused, createNativeProxy, exactOrigin, laneFor } from './native-proxy.mjs';

export const phases = Object.freeze([
  { name: 'smoke', seconds: 10, devices: 1, rate: 2, interval: 4000 },
  { name: 'baseline', seconds: 20, devices: 2, rate: 12, interval: 2500 },
  { name: 'mixed', seconds: 30, devices: 4, rate: 30, interval: 2000 },
  { name: 'pressure', seconds: 20, devices: 4, rate: 60, interval: 1000 }
]);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
function requireValue(value, code) { if (!value) throw new Refused(code); }
export function validateInput(input) {
  requireValue(input && Object.keys(input).sort().join(',') === 'chapter,manga,origin,report', 'input');
  exactOrigin(input.origin);
  for (const field of ['chapter', 'manga']) requireValue(typeof input[field] === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(input[field]), 'uuid');
  requireValue(typeof input.report === 'string' && path.isAbsolute(input.report) && !fs.existsSync(input.report), 'report');
  return input;
}

export function reconcileRequests(browser, proxy, cache) {
  for (const key of new Set([...browser.keys(), ...proxy.keys(), ...cache.keys()])) {
    requireValue((browser.get(key) ?? 0) === (proxy.get(key) ?? 0) + (cache.get(key) ?? 0), 'native-proxy-reconciliation');
  }
  return true;
}

export function latencySummary(values) {
  const sorted = values.filter(value => Number.isFinite(value) && value >= 0).sort((a, b) => a - b);
  const percentile = p => sorted.length ? sorted[Math.ceil(sorted.length * p) - 1] : null;
  return { observations: sorted.length, p50: percentile(.5), p95: percentile(.95), p99: percentile(.99) };
}

// Observation only: native put and transaction behavior is unchanged. A successful
// chapter put is not a higher-level save call, disk fsync proof, or backend write.
export function matchingProgressCommit(rows, device, intent) {
  return rows.some(row => row.device === device && row.outcome === 'committed' && row.target_intent === intent && row.target_puts > 0);
}

export async function settleActions(actions, cancel, milliseconds = 10000) {
  requireValue(Array.isArray(actions) && actions.length <= 256, 'action-owner');
  let timer;
  try {
    const results = await Promise.race([Promise.allSettled([Promise.resolve().then(cancel), ...actions]),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Refused('action-settlement-timeout')), milliseconds); })]);
    requireValue(results[0].status === 'fulfilled', 'action-cancellation');
    return { all_settled: true, actions: actions.length, fulfilled: results.slice(1).filter(row => row.status === 'fulfilled').length,
      rejected: results.slice(1).filter(row => row.status === 'rejected').length };
  } finally { clearTimeout(timer); }
}

export function observeProgress({ database, chapter, manga }) {
  const original = IDBObjectStore.prototype.put, transactions = new WeakMap();
  let intent = 0;
  Object.defineProperty(globalThis, '__capacityBeginProgressIntent', { configurable: true, value: value => {
    if (!Number.isSafeInteger(value) || value <= 0 || value > 256 || value <= intent) throw Error('Invalid observation intent');
    intent = value;
  } });
  IDBObjectStore.prototype.put = function (...args) {
    const result = Reflect.apply(original, this, args);
    if (this.transaction.db.name === database && this.name === 'records' && this.transaction.mode === 'readwrite' && args[0]?.family === 'chapters') {
      let row = transactions.get(this.transaction);
      if (!row) {
        row = { puts: 0, target_puts: 0, target_intent: 0, began: performance.now() }; transactions.set(this.transaction, row);
        const end = outcome => { void globalThis.__capacityCommit({ outcome, puts: row.puts, target_puts: row.target_puts, target_intent: row.target_intent, elapsed_ms: performance.now() - row.began }); };
        this.transaction.addEventListener('complete', () => end('committed'), { once: true });
        this.transaction.addEventListener('abort', () => end('aborted'), { once: true });
      }
      row.puts++;
      if (intent && args[0]?.id === chapter && args[0]?.mangaId === manga && args[0]?.page === 2) {
        row.target_puts++;
        row.target_intent = row.target_intent === 0 || row.target_intent === intent ? intent : -1;
      }
    }
    return result;
  };
}

async function storedProgress(page, input, expected, series = false) {
  return page.evaluate(async ({ origin, chapter, manga, expected, series }) => {
    const name = 'weebdex-device-library-v1:' + encodeURIComponent(origin);
    // Never create/upgrade a missing database as a verification side effect.
    if (!(await indexedDB.databases()).some(row => row.name === name)) return false;
    const db = await new Promise((resolve, reject) => {
      const request = indexedDB.open(name);
      request.onupgradeneeded = () => { request.transaction.abort(); reject(Error('Missing database')); };
      request.onerror = () => reject(Error('Read-only observation failed'));
      request.onsuccess = () => resolve(request.result);
    });
    try {
      return await new Promise((resolve, reject) => {
        const tx = db.transaction('records', 'readonly'), read = tx.objectStore('records').get(series ? ['series', manga] : ['chapters', chapter]);
        tx.oncomplete = () => resolve(series ? read.result?.id === manga && read.result?.status === 'reading' : read.result?.id === chapter && read.result?.mangaId === manga && read.result?.page === expected);
        tx.onabort = () => reject(Error('Read-only observation failed'));
      });
    } finally { db.close(); }
  }, { origin: input.origin, chapter: input.chapter, manga: input.manga, expected, series });
}

export async function runNative(input) {
  validateInput(input);
  const here = path.dirname(fileURLToPath(import.meta.url));
  const sourceFiles = ['native-browser.mjs', 'native-proxy.mjs', '../ci/browser-runtime.mjs', '../browser-select-controls.mjs'];
  const sha = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const sourceHashes = Object.fromEntries(sourceFiles.map(name => [name, sha(path.join(here, name))]));
  const proxyKeys = new Map(), browserKeys = new Map(), cacheKeys = new Map();
  const add = (map, key) => map.set(key, (map.get(key) ?? 0) + 1);
  const credits = new Credits(), proxy = createNativeProxy({ origins: [input.origin], credits, onAttempt: key => add(proxyKeys, key) });
  const report = { schema: 'real-application-bounded-probe-v1', status: 'failed', capacity_qualified: false,
    real_browser_executed: false, application_responses_mocked: false, context_interception_used: false,
    public_sync_executed: false, library_30k_500k_qualified: false, production_scale_qualified: false,
    browser_requests: 0, browser_terminals: 0, browser_failed: 0, decoded_image_checks: 0,
    progress_transactions: [], operations: [], native_requests: [], phases: [], cleanup: {}, failure: null };
  let browser, closeResult;
  const contexts = [], requests = new Set(), terminal = new Set(), nativeRows = new Map(), actionPromises = [];
  let phaseName = 'warmup', offered = 0, nextIntent = 0;
  const fail = code => { report.failure ??= code; proxy.fail(new Refused(code)); };
  const check = () => { credits.check(); requireValue(!report.failure, report.failure); };
  async function until(read, predicate, milliseconds = 8000) {
    const deadline = performance.now() + milliseconds;
    while (performance.now() < deadline) { check(); const value = await read(); if (predicate(value)) return value; await sleep(50); }
    throw new Refused('semantic-timeout');
  }
  async function idle() { await until(async () => credits.active + credits.waiting, count => count === 0); }
  async function open(page, route) {
    check(); const response = await page.goto(input.origin + route, { waitUntil: 'domcontentloaded', timeout: 12000 });
    requireValue(response?.status() === 200, 'navigation-status');
  }
  async function reader(page) {
    await open(page, '/chapter/' + input.chapter + '/1');
    const images = () => page.locator('img[src^="blob:"]').evaluateAll(rows => rows.filter(image => image.complete && image.naturalWidth > 0 && image.getClientRects().length).length);
    await until(images, count => count > 0); report.decoded_image_checks++;
    const intent = ++nextIntent;
    await page.evaluate(value => globalThis.__capacityBeginProgressIntent(value), intent);
    const selector = selectListboxTrigger(page).filter({ hasText: /^\s*\d+(?:-\d+)?\s*\/\s*\d+\s*$/ }).first();
    await selector.click(); await clickListboxOption(page, '2');
    await page.waitForURL(url => url.origin === input.origin && url.pathname === '/chapter/' + input.chapter + '/2');
    await until(images, count => count > 0); report.decoded_image_checks++;
    await until(() => storedProgress(page, input, 2), Boolean);
    await until(async () => matchingProgressCommit(report.progress_transactions, page.capacityDevice, intent), Boolean);
    // A former page-two row is not proof that this new reader session committed.
    requireValue(await storedProgress(page, input, 2), 'new-progress-commit');
  }
  async function action(device, kind, planned) {
    const row = { phase: phaseName, kind, device: device.index, offered_ms: planned, started_ms: performance.now(), outcome: 'started' };
    report.operations.push(row);
    try {
      if (kind === 'reader') await reader(device.page);
      else if (kind === 'search') {
        await open(device.page, '/search');
        const form = device.page.locator('form').filter({ has: device.page.getByRole('button', { name: 'Search', exact: true }) });
        await form.getByRole('button', { name: 'Search', exact: true }).click();
        await idle();
        requireValue(await device.page.locator('a[href^="/title/"]').count() > 0, 'search-empty');
      } else {
        await open(device.page, '/library'); await idle();
        requireValue(await storedProgress(device.page, input, 2), 'library-progress-lost');
        requireValue(await storedProgress(device.page, input, 2, true), 'library-membership-lost');
        await device.page.locator('a[href^="/title/' + input.manga + '"]').first().waitFor({ state: 'visible' });
      }
      row.outcome = 'completed';
    } catch (error) { row.outcome = 'failed'; fail(error instanceof Refused ? error.code : 'native-action'); throw error; }
    finally { row.elapsed_ms = performance.now() - row.started_ms; row.started_ms -= credits.start; device.busy = false; }
  }
  try {
    await new Promise((resolve, reject) => { proxy.server.once('error', reject); proxy.server.listen(0, '127.0.0.1', resolve); });
    const { chromium, executablePath, description } = loadBrowserRuntime();
    report.runtime = { description, launcher_sha256: sha(executablePath), node_sha256: sha(process.execPath), browser_executable_and_package_manifest_pinned: description.startsWith('bundled Chromium'), entire_browser_installation_pinned: false };
    browser = await chromium.launch({ executablePath, headless: true, chromiumSandbox: true,
      proxy: { server: 'http://127.0.0.1:' + proxy.server.address().port, bypass: '<-loopback>' },
      args: ['--proxy-bypass-list=<-loopback>', '--disable-quic', '--disable-background-networking', '--disable-component-update', '--disable-sync', '--no-first-run', '--disable-default-apps', '--disable-features=MediaRouter'] });
    report.real_browser_executed = true;
    report.runtime.browser_version = browser.version();
    for (let index = 0; index < 4; index++) {
      const context = await browser.newContext({ serviceWorkers: 'block', viewport: { width: 1280, height: 900 } });
      contexts.push(context);
      await context.exposeBinding('__capacityCommit', (_, value) => {
        if (!value || Object.keys(value).sort().join(',') !== 'elapsed_ms,outcome,puts,target_intent,target_puts' || !['committed', 'aborted'].includes(value.outcome) || !Number.isSafeInteger(value.puts) || value.puts < 1 || !Number.isSafeInteger(value.target_intent) || value.target_intent < 0 || value.target_intent > nextIntent || !Number.isSafeInteger(value.target_puts) || value.target_puts < 0 || value.target_puts > value.puts || !Number.isFinite(value.elapsed_ms) || value.elapsed_ms < 0 || report.progress_transactions.length >= 1024) { fail('progress-observer'); return; }
        report.progress_transactions.push({ phase: phaseName, device: index, outcome: value.outcome, puts: value.puts, target_puts: value.target_puts, target_intent: value.target_intent, elapsed_ms: value.elapsed_ms });
        if (value.outcome !== 'committed') fail('progress-aborted');
      });
      await context.addInitScript(observeProgress, { database: 'weebdex-device-library-v1:' + encodeURIComponent(input.origin), chapter: input.chapter, manga: input.manga });
      context.on('request', request => {
        if (!/^https?:/.test(request.url())) return;
        if (requests.size >= 2500 || new URL(request.url()).origin !== input.origin || requests.has(request)) { fail('browser-request-scope'); return; }
        requests.add(request); report.browser_requests++; add(browserKeys, request.method() + ' ' + request.url());
        const row = { phase: phaseName, method: request.method(), lane: laneFor(request.url()), offered_ms: performance.now() - credits.start, terminal_ms: null, elapsed_ms: null, outcome: 'pending' };
        nativeRows.set(request, row); report.native_requests.push(row);
      });
      const finish = (request, failed) => {
        if (!/^https?:/.test(request.url())) return;
        if (!requests.has(request) || terminal.has(request)) { fail('browser-terminal'); return; }
        terminal.add(request); report.browser_terminals++; if (failed) { report.browser_failed++; fail('native-request-failed'); }
        const row = nativeRows.get(request); row.terminal_ms = performance.now() - credits.start;
        row.elapsed_ms = row.terminal_ms - row.offered_ms; row.outcome = failed ? 'failed' : 'complete';
      };
      context.on('requestfinished', request => finish(request, false)); context.on('requestfailed', request => finish(request, true));
      context.on('page', page => page.on('pageerror', () => fail('page-error')));
      const page = await context.newPage(); page.setDefaultTimeout(8000); page.capacityDevice = index;
      // Passive CDP evidence distinguishes cache-only browser requests from proxy
      // bypass. It never pauses, rewrites, fulfills or reads a response body.
      const cdp = await context.newCDPSession(page), cdpRequests = new Map();
      await cdp.send('Network.enable');
      cdp.on('Network.requestWillBeSent', event => {
        if (!/^https?:/.test(event.request.url)) return;
        if (cdpRequests.size >= 2500) { fail('cdp-observation-limit'); return; }
        cdpRequests.set(event.requestId, { key: event.request.method + ' ' + event.request.url, cache: false, finished: false });
      });
      cdp.on('Network.requestServedFromCache', event => {
        const row = cdpRequests.get(event.requestId); if (!row) fail('cdp-cache-identity'); else row.cache = true;
      });
      cdp.on('Network.responseReceived', event => {
        const row = cdpRequests.get(event.requestId);
        if (row && (event.response.fromDiskCache || event.response.fromPrefetchCache)) row.cache = true;
        if (event.response.fromServiceWorker) fail('unaccounted-service-worker');
      });
      cdp.on('Network.loadingFinished', event => {
        const row = cdpRequests.get(event.requestId); if (!row) return;
        if (row.finished) { fail('duplicate-cdp-terminal'); return; } row.finished = true;
        if (row.cache && event.encodedDataLength === 0) add(cacheKeys, row.key);
      });
      context.capacityDevice = { index, page, busy: false };
      await open(page, '/title/' + input.manga);
      await selectTitleLibraryStatus(page, 'Reading');
      await until(() => storedProgress(page, input, 2, true), Boolean);
      await reader(page); await idle();
    }
    // Deliberately specified sensitivity mix, not inferred from monthly volumes.
    const kinds = ['reader', 'search', 'library'];
    for (const phase of phases) {
      await idle(); check(); phaseName = phase.name; proxy.setPhase(phaseName, phase.rate);
      const start = performance.now(), end = start + phase.seconds * 1000, active = new Set();
      const counts = { ...phase, offered: 0, started: 0, dropped_busy: 0, completed: 0 };
      report.phases.push(counts);
      for (let next = start; next < end; next += phase.interval) {
        await sleep(Math.max(0, next - performance.now())); check();
        for (let index = 0; index < phase.devices; index++) {
          const device = contexts[index].capacityDevice; counts.offered++; offered++;
          if (device.busy) { counts.dropped_busy++; continue; }
          device.busy = true; counts.started++;
          requireValue(actionPromises.length < 256, 'action-owner-limit');
          const operation = action(device, kinds[(offered - 1) % kinds.length], next - credits.start);
          actionPromises.push(operation);
          active.add(operation); operation.then(() => { counts.completed++; }, () => {}).finally(() => active.delete(operation));
        }
      }
      // No next phase until current work settles; this tail is measured separately.
      await sleep(Math.max(0, end - performance.now()));
      await Promise.allSettled(active); await idle(); check();
      counts.elapsed_ms = performance.now() - start;
      counts.tail_ms = Math.max(0, counts.elapsed_ms - phase.seconds * 1000);
    }
    phaseName = 'drain'; await idle(); proxy.setPhase('drain', 60);
    for (const context of contexts) {
      await open(context.capacityDevice.page, '/library');
      requireValue(await storedProgress(context.capacityDevice.page, input, 2), 'final-progress');
    }
    await idle();
    await until(async () => requests.size - terminal.size, count => count === 0);
    // Exact method/URL multiplicities stay private. Every non-proxy request must
    // have an independently terminal cache-only CDP observation with zero wire
    // length. Ambiguous cache/retry/service-worker behavior is never a pass.
    reconcileRequests(browserKeys, proxyKeys, cacheKeys);
    report.cache_only_requests = [...cacheKeys.values()].reduce((a, b) => a + b, 0);
    requireValue(report.progress_transactions.some(row => row.phase !== 'warmup' && row.outcome === 'committed'), 'no-measured-progress');
    report.status = 'observed-bounded-probe-pending-cleanup';
  } catch (error) { report.failure ??= error instanceof Refused ? error.code : 'native-browser'; }
  finally {
    try { report.cleanup.actions = await settleActions(actionPromises, async () => {
      for (const context of contexts) await context.close(); report.cleanup.contexts_closed = true;
    }); } catch { report.failure ??= 'action-or-context-cleanup'; }
    try { if (browser) await browser.close(); report.cleanup.browser_closed = true; } catch { report.failure ??= 'browser-cleanup'; }
    try { closeResult = await proxy.close(); report.cleanup.proxy = closeResult; } catch { report.failure ??= 'proxy-cleanup'; }
    report.transport = credits.snapshot(); report.elapsed_ms = performance.now() - credits.start;
    report.source_hashes = sourceHashes;
    report.source_unchanged = sourceFiles.every(name => sha(path.join(here, name)) === sourceHashes[name]);
    if (!report.source_unchanged) report.failure ??= 'native-source-changed';
    report.proxy_latency_ms = {
      proxy_queue: latencySummary(credits.records.filter(row => row.forwarded_ms !== null).map(row => row.forwarded_ms - row.offered_ms)),
      upstream_headers: latencySummary(credits.records.filter(row => row.headers_ms !== null).map(row => row.headers_ms - row.forwarded_ms)),
      first_payload: latencySummary(credits.records.filter(row => row.first_body_ms !== null).map(row => row.first_body_ms - row.forwarded_ms)),
      complete_payload: latencySummary(credits.records.filter(row => row.state === 'complete').map(row => row.complete_ms - row.forwarded_ms)) };
    report.native_request_latency_ms = {
      request_to_finished_event: latencySummary(report.native_requests.filter(row => row.outcome === 'complete').map(row => row.elapsed_ms)),
      request_to_failed_event: latencySummary(report.native_requests.filter(row => row.outcome === 'failed').map(row => row.elapsed_ms)) };
    report.idb_transaction_latency_ms = latencySummary(report.progress_transactions.map(row => row.elapsed_ms));
    report.phase_metrics = report.phases.map(phase => {
      const rows = credits.records.filter(row => row.phase === phase.name), seconds = phase.elapsed_ms / 1000;
      return { phase: phase.name, observation_seconds: seconds, offered_actions_per_second: phase.offered / seconds,
        completed_actions_per_second: phase.completed / seconds, proxy_attempts_per_second: rows.length / seconds,
        complete_http_per_second: rows.filter(row => row.state === 'complete').length / seconds,
        http_statuses: rows.reduce((result, row) => { const status = String(row.status); result[status] = (result[status] ?? 0) + 1; return result; }, {}),
        lanes: Object.fromEntries(['api', 'media', 'analytics', 'document-static'].map(lane => {
          const selected = rows.filter(row => row.lane === lane);
          return [lane, { attempts: selected.length, response_bytes_forwarded: selected.reduce((total, row) => total + row.response_bytes, 0),
            complete_payload_ms: latencySummary(selected.filter(row => row.state === 'complete').map(row => row.complete_ms - row.forwarded_ms)) }]; })) };
    });
    report.status = !report.failure && !credits.failure && closeResult?.unforced && report.cleanup.actions?.all_settled && report.cleanup.browser_closed && report.cleanup.contexts_closed ? 'passed-bounded-native-probe-only' : 'failed-bounded-native-probe';
    fs.writeFileSync(input.report, JSON.stringify(report, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  }
  return report;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 3 || fs.statSync(process.argv[2]).size > 65536) throw new Refused('input');
    const result = await runNative(JSON.parse(fs.readFileSync(process.argv[2], 'utf8')));
    process.exitCode = result.status === 'passed-bounded-native-probe-only' ? 0 : 1;
  } catch { process.stderr.write('Native capacity adapter refused or failed; private evidence retained.\n'); process.exitCode = 1; }
}
