// Independent UI11–16 browser regressions. Importing starts no application,
// browser, listener or fixture. Run only after the parent's final runtime gate.
// Real success data comes from the disposable native backend. Native browser
// gates delay acknowledged work, and never manufacture a successful response.
import { randomBytes } from "node:crypto";
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { selectListboxTrigger, clickListboxOption, setListboxOptions } from "./browser-select-controls.mjs";
import { observeCompactFeedResponse } from "./compact-feed-http-outcome.mjs";
import { actorFollowPhase, actorFollowDiagnosticChecks, runActorFollowJourneys } from "./browser-actor-follow-journeys.mjs";
import { runHeldAccountDenial } from "./browser-account-capacity.mjs";
import { homeFailureDiagnosticChecks, retryDeniedHomeRead } from "./browser-home-controls.mjs";
const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export const ui11To16JourneyPhases = Object.freeze([
  "ui11-device-navigation", "ui11-device-progress", "ui12-compact-feed",
  "ui13-terminal-feed", "ui13-feed-authority", "ui14-entity-pagination",
  "ui14-account-failures", "ui14-home-failures", "ui15-chapter-filter",
  "ui15-reader-feedback", "ui15-reader-geometry", "ui15-16-empty-tabs",
  "ui16-home-preferences", "ui16-search-cap", actorFollowPhase
]);

// Closed diagnostic vocabulary. IDs encode reviewed controls only, never fixture
// identities, paths, URLs, credentials, request bodies or exception text.
const progressCases = Object.freeze(["timer-page1", "timer-page3", "lock-page1", "lock-page3"]);
const progressChecks = Object.freeze([
  "setup", "select-page", "no-prior-progress", "gate-captured",
  "old-intent-cancelled", "page-retained", "durable-progress",
  "reload-progress", "stable-revision"
]);
export const ui11To16DiagnosticChecks = Object.freeze({
  ...homeFailureDiagnosticChecks,
  ...actorFollowDiagnosticChecks,
  "ui14-account-failures": Object.freeze(["prerequisite", "open", "account-mode", "durable-baseline", "trigger", "option", "pressure-captured", "capacity-attempt", "capacity-fixture", "capacity-connections", "capacity-saturated", "capacity-live", "capacity-ready", "pressure-released", "denial", "feedback", "no-replay", "capacity-cleanup", "durable-unchanged"].map(check => `account429-title-${check}`)),
  "ui14-entity-pagination": Object.freeze([
    "entity-seed-pagination", "entity-fixture",
    ...["group", "author", "user"].flatMap(kind => ["desc", "asc"].flatMap(order => ["prerequisite", "open", "native-last-page", "page-one-control", "page-one-route", "native-first-page"].map(check => `entity-${kind}-${order}-${check}`))),
    ...["withdrawal", "open", "empty-visible", "native-empty", "page-one-control", "native-first-page"].map(check => `entity-empty-${check}`)
  ]),
  "ui11-device-progress": Object.freeze([
    "device-progress-signup-context",
    ...progressCases.flatMap(control => progressChecks.map(check => `device-progress-${control}-${check}`))
  ]),
  "ui12-compact-feed": Object.freeze([
    "compact-feed-seed-pagination", "compact-feed-spanish-preferences", "compact-feed-fixture-total",
    "compact-feed-fixture-response-received", "compact-feed-fixture-http-200", "compact-feed-fixture-total-integer",
    "compact-feed-fixture-total-at-least-seeded", "compact-feed-fixture-total-within-known-scenarios",
    "compact-feed-open-updates", "compact-feed-third-page", "compact-feed-resume-visible",
    "compact-feed-bounded-burst", "compact-feed-compact-geometry", "compact-feed-trimmed-cards",
    "compact-feed-manual-terminal-page", "compact-feed-unique-sequential-pages",
    "compact-feed-scenario-membership", "compact-feed-all-compact-chapters",
    "compact-feed-terminal-idle", "compact-feed-resize-retains-entries", "compact-feed-back-restores-entries"
  ])
});

export async function runUi11To16Journeys(h) {
  const { phase, page, context, origin, fixture, step, insist, open, report, redactions } = h;
  const check = (id, operation) => {
    insist(typeof h.check === "function", "The reviewed fixed-check diagnostic recorder is required for this journey");
    return h.check(id, operation);
  };
  insist(ui11To16JourneyPhases.includes(phase), "Unknown UI11–16 journey phase");
  const started = Date.now(), marker = randomBytes(6).toString("hex"), ids = fixture.ids ?? {};
  let extras = fixture.ui_regressions;
  const privateCredentials = new Set();
  let account;
  const remember = value => { if (typeof value === "string" && value) { redactions.add(value); privateCredentials.add(value); } return value; };
  const finite = () => insist(Date.now() - started < 270000, "The finite UI regression phase exhausted its 270-second work budget");
  const until = async (read, valid, message, timeout = 20000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) { finite(); const value = await read(); if (await valid(value)) return value; await page.waitForTimeout(80); }
    throw Error(message);
  };
  const dependency = name => {
    insist(extras?.fixture_only === true && extras[name], `UNIMPLEMENTED FIXTURE DEPENDENCY: genuine ui_regressions.${name} is required; no successful feed is substituted`);
    return extras[name];
  };
  const privateControl = async (action, extra = {}) => {
    const control = fixture.fixture_control, directory = path.resolve(fixture.run_directory ?? "");
    insist(control?.fixture_only === true && control.allowed_actions?.includes(action), `UNIMPLEMENTED FIXTURE DEPENDENCY: reviewed private ${action} control is required`);
    insist(path.dirname(directory) === path.join(projectRoot, "test-runtime") && /^browser-final-[A-Za-z0-9_-]+$/.test(path.basename(directory)) && fs.realpathSync(directory) === directory && (fs.statSync(directory).mode & 0o077) === 0, "Private UI fixture root is not the owned disposable runtime");
    const source = path.join(directory, "supervisor.json"), command = path.join(directory, "fixture-control-command.json"), results = path.join(directory, "control-results");
    insist(control.command_file === command && control.results_dir === results && path.resolve(process.env.BROWSER_FIXTURE_FILE ?? "") === source, "UI private control paths differ from the exact owned supervisor");
    const readPrivate = (file, maximum = 1024 * 1024) => { insist(fs.realpathSync(file) === file && fs.lstatSync(file).isFile() && fs.statSync(file).size <= maximum, "Unbounded or linked private UI fixture receipt"); return JSON.parse(fs.readFileSync(file, "utf8")); };
    const ownership = readPrivate(path.join(directory, "fixture-owned.json"), 4096); insist(ownership.fixture_only === true && ownership.database === "weebdex_browser_test", "UI disposable fixture ownership marker differs");
    const before = readPrivate(source), owned = Object.entries(before.children ?? {}).filter(([name, child]) => name.startsWith("browser-") && child.pid === process.pid);
    insist(owned.length === 1 && before.platform_origin === origin, "This UI runner is not the exact recorded owned fixture browser process");
    const browserPhase = owned[0][0].slice("browser-".length), timeout = ["ui-regressions-seed", "ui-regressions-readers"].includes(action) ? 120000 : action.startsWith("pg-home-fault-") ? 60000 : 30000;
    insist(before.browser_phase_deadline_unix * 1000 - Date.now() >= timeout + 20000 && before.deadline_unix * 1000 - Date.now() >= timeout + 20000, "Insufficient finite phase/fixture lifetime for the named UI control");
    const requestId = "ui-" + marker + "-" + randomBytes(6).toString("hex"), bytes = Buffer.from(JSON.stringify({ request_id: requestId, action, browser_phase: browserPhase, ...extra }));
    insist(bytes.length <= 4096 && !fs.existsSync(command), "Private UI command queue is occupied or oversized");
    const temporary = command + "." + requestId + ".new"; fs.writeFileSync(temporary, bytes, { flag: "wx", mode: 0o600 }); fs.renameSync(temporary, command);
    const file = path.join(results, requestId + ".json"); await until(() => fs.existsSync(file), Boolean, "Named UI fixture control did not return its finite terminal receipt", timeout + 5000);
    const result = readPrivate(file); insist(result.fixture_only === true && result.request_id === requestId && result.action === action && result.success === true && !result.cleanup_required, "Named UI fixture control failed; acceptance is not inferred");
    const current = readPrivate(source); insist(current.platform_origin === origin && current.service_key === before.service_key && current.frontend?.index_sha256 === before.frontend?.index_sha256, "Named UI control changed approved service/frontend identity");
    extras = current.ui_regressions; Object.assign(fixture, current); return result;
  };
  const ensurePagination = async () => { if (!extras?.compact) await privateControl("ui-regressions-seed", { scenario: "pagination" }); };
  const ensureReaders = async () => { insist(extras?.compact, "The pagination seed/compact phase must finish before reader activation; two 120-second controls do not fit one ordinary phase"); if (!Array.isArray(extras?.readers) || !extras.readers.length || !extras.bilingual?.manga_id) await privateControl("ui-regressions-readers"); };
  const api = async (route, options = {}, observeOutcome) => {
    finite(); insist(route.startsWith("/api/") && !route.includes("://") && !route.startsWith("//"), "UI regression observation escaped the fixture API");
    const fetchResponse = () => context.request.fetch(origin + route, { ...options, maxRedirects: 0, timeout: 12000, headers: { Origin: origin, ...(options.headers ?? {}) } });
    if (observeOutcome) return observeCompactFeedResponse(fetchResponse, observeOutcome);
    const response = await fetchResponse();
    return { status: response.status(), body: /json/.test(response.headers()["content-type"] ?? "") ? await response.json() : undefined };
  };
  const sink = () => {
    const url = new URL(fixture.mail_origin ?? fixture.control_origin ?? "http://127.0.0.1:0");
    insist(url.protocol === "http:" && ["127.0.0.1", "localhost"].includes(url.hostname) && url.port && url.port !== "0" && !url.username && !url.password && url.pathname === "/" && !url.search && !url.hash, "The verified local mail/provisioning sink is required");
    return url.origin;
  };
  const provision = async profile => {
    const response = await context.request.post(sink() + "/fixture/provision", { data: { email: account.email, profile, join_seed_group: true }, maxRedirects: 0 });
    const body = await response.json(); insist(response.status() === 200 && body.fixture_only === true, "Actual synthetic account provisioning failed");
    return body;
  };
  const signup = async (profile = "user") => {
    const suffix = randomBytes(5).toString("hex");
    account = { email: remember(`ui1116_${marker}_${suffix}@example.invalid`), username: `UI${suffix}`, password: remember(`Fixture!${randomBytes(24).toString("base64url")}9a`) };
    await open("/auth/register");
    await page.locator('input[name="email"]').fill(account.email);
    await page.locator('input[name="username"]').fill(account.username);
    await page.locator('input[name="password"]').fill(account.password);
    await page.getByRole("button", { name: "Create Account", exact: true }).click();
    await until(() => api("/api/auth/check"), value => value.status === 200 && value.body?.identity?.username === account.username, "Visible registration did not establish the genuine synthetic account");
    await provision(profile);
    await open("/auth/verification"); await page.locator('input[name="email"]').fill(account.email);
    await page.getByRole("button", { name: "Send Verification Link", exact: true }).click();
    const mail = await until(async () => { const response = await context.request.get(sink() + `/mail/latest?to=${encodeURIComponent(account.email)}&kind=verification`, { maxRedirects: 0 }); return response.ok() ? response.json() : undefined; }, value => value?.fixture_only === true && typeof value.link === "string", "Genuine local verification mail did not arrive");
    const link = new URL(mail.link); insist(link.origin === origin && link.pathname.startsWith("/auth/"), "One-time verification link escaped the exact fixture");
    remember(link.href); for (const [, value] of link.searchParams) remember(value);
    await page.goto(link.href, { waitUntil: "domcontentloaded" });
    await until(() => api("/api/auth/check"), value => value.status === 200 && value.body?.identity?.is_verified, "Genuine verification did not establish verified authority");
    await open("/");
  };
  const login = async () => {
    await open("/auth/login"); await page.locator('input[name="identifier"]').fill(account.email); await page.locator('input[name="password"]').fill(account.password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await until(() => api("/api/auth/check"), value => value.status === 200 && value.body?.identity?.username === account.username, "Actual login did not restore the synthetic browser session");
    await open("/");
  };
  const panel = () => page.locator("details[data-local-library-panel]");
  const expand = async locator => { if (!await locator.evaluate(node => node.open)) await locator.locator(":scope > summary").click(); };
  const ready = async () => { await expand(panel()); await panel().getByText(/Scope verification: verified\./).waitFor({ state: "visible" }); };
  const mode = async selected => { await expand(panel()); const button = panel().getByRole("button", { name: `Use ${selected} library`, exact: true }); if (await button.isEnabled()) await button.click(); await panel().getByText(selected === "device" ? /Library source: this browser device\./ : /Library source: signed-in WeebDex account\./).waitFor({ state: "visible" }); };
  const realm = async () => page.evaluate(value => { window.__ui1116RealmProof = value; }, marker);
  const sameRealm = async () => insist(await page.evaluate(() => window.__ui1116RealmProof) === marker, "The regression navigated by document reload, hiding the SPA lifetime defect");
  const spaLink = async href => { await page.locator(`a[href="${href}"]`).first().click(); await page.waitForURL(url => url.pathname === new URL(href, origin).pathname); };
  const search = async () => { await page.getByRole("link", { name: "Advanced Search", exact: true }).first().click(); await page.waitForURL(url => url.pathname === "/search"); };
  const stable = async (predicate, message, milliseconds = 800) => { const end = Date.now() + milliseconds; while (Date.now() < end) { finite(); insist(await predicate(), message); await page.waitForTimeout(60); } };
  const bodyHas = async text => (await page.locator("body").innerText()).includes(text);
  const checked = async (selector, desired) => { const target = page.locator(selector); await target.waitFor({ state: "visible" }); const actual = await target.evaluate(node => node instanceof HTMLInputElement ? node.checked : node.getAttribute("aria-checked") === "true"); if (actual !== desired) await target.click(); };
  const savePreferences = async () => {
    const save = page.getByRole("button", { name: "Save", exact: true });
    if (await save.isEnabled()) { const response = account ? page.waitForResponse(value => new URL(value.url()).pathname === "/api/settings" && value.request().method() === "PUT") : undefined; await save.click(); if (response) insist((await response).status() === 200, "Actual account preference save was not acknowledged"); await page.getByText("Your changes have been saved.", { exact: true }).last().waitFor({ state: "visible" }); }
  };
  const selectOptions = (trigger, labels) => setListboxOptions(page, trigger, labels);
  const selectOption = label => clickListboxOption(page, label);
  const chapterLanguagePreference = async label => { await open("/settings/preferences"); const section = page.locator("div.flex.flex-col").filter({ has: page.locator("strong", { hasText: "Chapter Language Filter" }) }).last(); await selectOptions(selectListboxTrigger(section), label ? [label] : []); await savePreferences(); };
  const deviceRows = () => page.evaluate(async currentOrigin => {
    const db = await new Promise((resolve, reject) => { const request = indexedDB.open("weebdex-device-library-v1:" + encodeURIComponent(currentOrigin), 1); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(Error("Read-only device observation failed")); });
    try { return await new Promise((resolve, reject) => { const tx = db.transaction("records", "readonly"), request = tx.objectStore("records").getAll(); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(Error("Read-only record observation failed")); }); } finally { db.close(); }
  }, origin);
  const deviceRevision = () => page.evaluate(async currentOrigin => {
    const db = await new Promise((resolve, reject) => { const request = indexedDB.open("weebdex-device-library-v1:" + encodeURIComponent(currentOrigin), 1); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(Error("Read-only revision observation failed")); });
    try { return await new Promise((resolve, reject) => { const request = db.transaction("meta", "readonly").objectStore("meta").get("revision"); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(Error("Read-only revision observation failed")); }); } finally { db.close(); }
  }, origin);
  const removeDeviceChapter = async id => {
    // Test setup uses the supplied, reviewed backup workflow. No direct IDB
    // mutation can make a missing progress repair look successful.
    await expand(panel()); const received = page.waitForEvent("download"); await panel().getByRole("button", { name: "Export device backup", exact: true }).click(); const download = await received;
    const stream = await download.createReadStream(), chunks = []; for await (const chunk of stream) chunks.push(chunk);
    const backup = JSON.parse(Buffer.concat(chunks).toString("utf8")); insist(backup.format === "weebdex-device-library" && backup.scope.origin === origin, "Actual UI device backup belongs to another scope");
    backup.records = backup.records.filter(row => !(row.family === "chapters" && row.id === id));
    await panel().getByLabel("Preview device backup", { exact: false }).setInputFiles({ name: "synthetic-reviewed-device.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(backup)) });
    await panel().getByRole("button", { name: "Replace with reviewed import", exact: true }).waitFor({ state: "visible" });
    page.once("dialog", dialog => dialog.accept()); await panel().getByRole("button", { name: "Replace with reviewed import", exact: true }).click();
    await until(deviceRows, rows => !rows.some(row => row.family === "chapters" && row.id === id), "Reviewed actual UI replacement did not remove the previous device progress prerequisite");
  };

  // Withhold exactly one real request before forwarding or after its real
  // response. No body/status is replaced. A browser cancellation is distinct
  // from absent server effects and is reported truthfully by the gate.
  const hold = async (pattern, match, before = false) => {
    let resolve, admitted = false, captured = false, done = false, disposition = "pending", failure, abort = false, status;
    const gate = new Promise(value => { resolve = value; });
    const intercept = async route => {
      if (!match(route.request()) || admitted) return route.continue();
      admitted = true;
      try {
        let response;
        if (before) { captured = true; await gate; response = await route.fetch({ maxRedirects: 0 }); }
        else { response = await route.fetch({ maxRedirects: 0 }); captured = true; await gate; }
        status = response.status();
        if (abort) { await route.abort("failed"); disposition = "failed-delivery-after-real-response"; }
        else { await route.fulfill({ response }); disposition = "delivered"; }
      } catch { disposition = "browser-cancelled"; if (!captured) failure = "The intended genuine fixture request could not be captured"; try { await route.abort("failed"); } catch {} }
      finally { done = true; }
    };
    await context.route(pattern, intercept);
    return { wait: () => until(() => captured || failure, Boolean, "Actual UI request was not captured").then(() => insist(!failure, failure)), release: async () => { resolve(); await until(() => done || !admitted, Boolean, "Held native request did not drain"); }, fail: async () => { abort = true; resolve(); await until(() => done || !admitted, Boolean, "Native failed delivery did not drain"); }, close: async () => { resolve(); if (admitted) await until(() => done, Boolean, "Held request cleanup did not finish"); await context.unroute(pattern, intercept); }, get disposition() { return disposition; }, get nativeStatus() { return status; } };
  };
  const observe = matcher => {
    const rows = [], active = new Set(), pending = new Set(); let maximum = 0, requests = 0;
    const request = value => { if (matcher(value)) { requests++; active.add(value); maximum = Math.max(maximum, active.size); } };
    const finished = value => active.delete(value);
    const response = value => {
      if (!matcher(value.request())) return;
      const promise = (async () => { const body = /json/.test(value.headers()["content-type"] ?? "") ? await value.json() : undefined; rows.push({ status: value.status(), method: value.request().method(), pathname: new URL(value.url()).pathname, query: new URL(value.url()).search, body }); })().catch(() => rows.push({ status: value.status(), body_unavailable: true })).finally(() => pending.delete(promise)); pending.add(promise);
    };
    page.on("request", request); page.on("requestfinished", finished); page.on("requestfailed", finished); page.on("response", response);
    return { rows, get maximum() { return maximum; }, get requests() { return requests; }, settle: async () => { await Promise.all([...pending]); }, close: async () => { page.off("request", request); page.off("requestfinished", finished); page.off("requestfailed", finished); page.off("response", response); await Promise.all([...pending]); } };
  };
  const requestPath = (request, pathname, method = "GET") => new URL(request.url()).origin === origin && new URL(request.url()).pathname === pathname && request.method() === method;
  const countPosts = pathname => observe(request => requestPath(request, pathname, "POST"));
  const withCapacity = async (operation, probe = (_suffix, action) => action()) => {
    await probe("capacity-fixture", async () => { const url = new URL(origin); insist(fixture.fixture_only === true && url.protocol === "http:" && ["127.0.0.1", "localhost"].includes(url.hostname) && !!url.port && url.port !== "0", "Capacity fault requires the owned exact loopback disposable fixture"); });
    const owned = [], sockets = new Set(); let released = false;
    const close = () => { if (released) return; released = true; for (const request of owned) request.destroy(); for (const socket of sockets) socket.destroy(); };
    // Start the watchdog before connecting, so even a partially connected set
    // cannot keep the actual middleware saturated while waiting for a browser.
    const watchdog = setTimeout(close, 9000);
    try {
      await probe("capacity-connections", () => Promise.all(Array.from({ length: 32 }, () => new Promise((resolve, reject) => {
        const request = http.request(origin + "/api/client", { method: "POST", headers: { Origin: origin, "Content-Type": "application/json", "Content-Length": "65536" }, agent: false }); owned.push(request);
        request.on("socket", socket => { sockets.add(socket); socket.once("close", () => sockets.delete(socket)); socket.once("connect", () => { request.write('{"name":"'); resolve(); }); }); request.on("error", () => { if (!released) reject(Error("Owned local capacity socket failed before saturation")); }); request.on("response", response => { response.resume(); if (!released) reject(Error("Incomplete native JSON did not retain its genuine admission permit")); }); request.setTimeout(10000, () => request.destroy());
      }))));
      await probe("capacity-saturated", () => until(() => api("/api/manga?limit=1"), value => value.status === 429 && value.body?.code === "capacity_busy", "Actual native request permits were not saturated", 3000));
      await probe("capacity-live", async () => { insist(!released, "The bounded capacity watchdog expired before the denied gesture was released"); });
      return await operation();
    } finally { clearTimeout(watchdog); close(); await probe("capacity-cleanup", () => until(() => sockets.size, count => count === 0, "Owned local capacity sockets did not reach their close events", 3000)); }
  };

  // Native platform fault gates, installed in this one real document only.
  // Snapshot gates delay only the final request callback, after the real readonly
  // transaction. Lock gates own the genuine browser lock and defer its callback.
  // Timer gates hold the actual admitted350ms progress callback, not the store.
  const browserGate = async kind => {
    await page.evaluate(kind => {
      if (window.__ui1116Gate) throw Error("A native gate is already installed");
      let release, admitted = false, pending, captured = false;
      const wait = new Promise(resolve => { release = resolve; });
      const gate = { kind, get captured() { return captured; }, release() { release(); }, restore() {} }; window.__ui1116Gate = gate;
      if (kind === "snapshot") {
        const original = IDBObjectStore.prototype.get, descriptor = Object.getOwnPropertyDescriptor(IDBRequest.prototype, "onsuccess");
        IDBObjectStore.prototype.get = function(key) {
          const result = original.call(this, key);
          if (!admitted && key === "scopeMigrationReason" && this.name === "meta" && this.transaction.mode === "readonly" && this.transaction.db.name.startsWith("weebdex-device-library-v1:")) {
            admitted = true;
            Object.defineProperty(result, "onsuccess", { configurable: true, get() { return pending; }, set(callback) { pending = callback; descriptor.set.call(result, function(event) { captured = true; void wait.then(() => callback?.call(result, event)); }); } });
          }
          return result;
        }; gate.restore = () => { IDBObjectStore.prototype.get = original; release(); delete window.__ui1116Gate; };
      } else if (kind === "mode-completion") {
        const original = IDBDatabase.prototype.transaction, descriptor = Object.getOwnPropertyDescriptor(IDBTransaction.prototype, "oncomplete");
        IDBDatabase.prototype.transaction = function(stores, mode, options) {
          const transaction = original.call(this, stores, mode, options);
          if (!admitted && this.name === "weebdex-protocol-library-v1" && mode === "readwrite" && (stores === "meta" || Array.isArray(stores) && stores.length === 1 && stores[0] === "meta")) {
            admitted = true;
            Object.defineProperty(transaction, "oncomplete", { configurable: true, get() { return pending; }, set(callback) { pending = callback; descriptor.set.call(transaction, function(event) { captured = true; void wait.then(() => callback?.call(transaction, event)); }); } });
          }
          return transaction;
        }; gate.restore = () => { IDBDatabase.prototype.transaction = original; release(); delete window.__ui1116Gate; };
      } else if (kind === "lock") {
        const original = navigator.locks.request;
        navigator.locks.request = function(name, options, callback) {
          const operation = typeof options === "function" ? options : callback;
          if (name !== "weebdex-device-library-write" || admitted) return original.apply(this, arguments);
          admitted = true;
          const delayed = async lock => { captured = true; await wait; return operation(lock); };
          return typeof options === "function" ? original.call(this, name, delayed) : original.call(this, name, options, delayed);
        }; gate.restore = () => { navigator.locks.request = original; release(); delete window.__ui1116Gate; };
      } else if (kind === "timer") {
        const original = window.setTimeout;
        window.setTimeout = function(callback, delay, ...args) {
          if (delay !== 350 || typeof callback !== "function" || admitted) return original.call(this, callback, delay, ...args);
          admitted = true;
          return original.call(this, () => { captured = true; void wait.then(() => callback(...args)); }, delay);
        }; gate.restore = () => { window.setTimeout = original; release(); delete window.__ui1116Gate; };
      } else throw Error("Unsupported native gate");
    }, kind);
    return { wait: () => until(() => page.evaluate(() => window.__ui1116Gate?.captured), Boolean, "The actual browser storage/timer seam was not reached"), release: () => page.evaluate(() => window.__ui1116Gate?.release()), close: () => page.evaluate(() => window.__ui1116Gate?.restore()) };
  };
  const readPage = async () => Number((await page.title()).match(/^Page (\d+):/)?.[1]);
  const readerSelector = () => selectListboxTrigger(page).filter({ hasText: /^\s*\d+(?:-\d+)?\s*\/\s*\d+\s*$/ });
  const selectReaderPage = async (label, expected) => { const selector = readerSelector().first(); await selector.click(); await selectOption(label); await until(readPage, value => value === expected, "Actual page selector chose a different raw page than its advertised option"); insist(new URL(page.url()).pathname.endsWith("/" + expected), "Reader URL does not agree with selected raw page"); };
  const readerSettings = async () => { if (!await page.getByRole("button", { name: "Paged", exact: true }).isVisible()) await page.locator('nav button:has(svg.lucide-settings)').first().click(); await page.getByRole("button", { name: "Paged", exact: true }).waitFor({ state: "visible" }); };
  const settings = async (direction = "Left-to-right", double = false) => { await readerSettings(); await page.getByRole("button", { name: "Paged", exact: true }).click(); await page.getByRole("button", { name: direction, exact: true }).click(); await page.getByRole("button", { name: double ? "Double" : "Single", exact: true }).click(); await page.getByRole("button", { name: "All", exact: true }).first().click(); };
  const closeReaderPanel = async () => { await page.locator('[data-sidebar="header"] button:has(svg.lucide-x)').click(); await page.getByRole("button", { name: "Paged", exact: true }).waitFor({ state: "hidden" }); };
  const nativeClient = async () => {
    const created = await api("/api/client", { method: "POST", data: { name: "UI regression readonly " + marker } });
    insist(created.status === 201 && typeof created.body?.key === "string", "Real readonly synthetic API client creation failed");
    const client = { id: created.body.id, key: remember(created.body.key) };
    const allowed = await api("/api/user/follows/manga?limit=1", { headers: { Authorization: "Bearer " + client.key } }); insist(allowed.status === 200, "The genuine readonly client did not have its real owner read scope");
    return client;
  };
  const withBearer = async (pathname, key, operation) => {
    const matcher = origin + pathname + "*", intercept = route => route.continue({ headers: { ...route.request().headers(), authorization: "Bearer " + key } });
    await context.route(matcher, intercept);
    try { return await operation(); } finally { await context.unroute(matcher, intercept); }
  };

  if (phase === "ui11-device-navigation") {
    let listName = "Delayed list " + marker, listHref;
    await step("UI11 list: actual creation and current saved-list link select the exact device list", async () => {
      await signup(); await ready(); await mode("device");
      const lists = panel().locator("details").filter({ has: page.locator("summary", { hasText: "Custom device lists" }) }); await expand(lists);
      await lists.getByLabel("List name", { exact: true }).fill(listName); await lists.getByRole("button", { name: "Create list", exact: true }).click();
      const link = lists.getByRole("link", { name: listName, exact: true }); await link.waitFor({ state: "visible" }); listHref = await link.getAttribute("href");
      await mode("account"); await link.click(); await page.waitForURL(url => url.pathname === "/library" && url.searchParams.get("list") === new URL(listHref, origin).searchParams.get("list"));
      await panel().getByText(/Library source: this browser device\./).waitFor({ state: "visible" });
      return { real_ui_created_list: true, exact_list_route: true, selected_device: true };
    });
    for (const waitAt of ["snapshot", "mode-completion"]) for (const lifetime of ["navigate", "back-forward", "reader-disposal"]) await step(`UI11 list: delayed native ${waitAt} cannot override ${lifetime}`, async () => {
      insist(listHref, "Actual saved-list prerequisite failed");
      await open(lifetime === "reader-disposal" ? `/chapter/${ids.chapter}/1` : "/library"); await ready(); await realm();
      const lists = panel().locator("details").filter({ has: page.locator("summary", { hasText: "Custom device lists" }) }); await expand(lists);
      const gate = await browserGate(waitAt);
      try {
        await lists.getByRole("link", { name: listName, exact: true }).click(); await gate.wait();
        if (lifetime === "reader-disposal") { await page.locator('[data-sidebar="header"] a[href="/"]').click(); await page.waitForURL(url => url.pathname === "/"); }
        else await search();
        if (lifetime === "back-forward") { await page.goBack({ waitUntil: "domcontentloaded" }); await page.goForward({ waitUntil: "domcontentloaded" }); }
        const newer = page.url(); await gate.release();
        await stable(() => page.url() === newer, "The obsolete saved-list callback overwrote newer route/history"); await sameRealm();
        return { gate: waitAt === "snapshot" ? "real IndexedDB readonly final acknowledgement" : "real committed mode transaction completion", exact_newer_route_retained: true, same_document: true, disposed_reader: lifetime === "reader-disposal", committed_preference_not_claimed_rolled_back: true };
      } finally { await gate.close(); }
    });
  }

  if (phase === "ui11-device-progress") {
    await step("UI11 progress: synthetic verified account and durable device context", async () => {
      await check("device-progress-signup-context", async () => { await signup(); await ready(); await mode("device"); });
      return { real_account: true, device_scope_verified: true };
    });
    for (const gateKind of ["timer", "lock"]) for (const targetPage of [1, 3]) await step(`UI11 progress: ${gateKind} device→account→device retains same page ${targetPage}`, async () => {
      const prefix = `device-progress-${gateKind}-page${targetPage}`;
      await check(prefix + "-setup", async () => {
        await open(`/title/${ids.manga}`); await ready(); await mode("account"); await removeDeviceChapter(ids.chapter);
        const link = page.locator(`a[href="/chapter/${ids.chapter}"]`).first(); await link.click(); await page.waitForURL(url => url.pathname.startsWith(`/chapter/${ids.chapter}`)); await settings();
      });
      await check(prefix + "-select-page", async () => { if (targetPage !== 1) await selectReaderPage(String(targetPage), targetPage); });
      await check(prefix + "-no-prior-progress", async () => { insist(!(await deviceRows()).some(row => row.family === "chapters" && row.id === ids.chapter), "Device progress already existed before the controlled reentry test"); });
      const gate = await browserGate(gateKind);
      try {
        const retained = await check(prefix + "-gate-captured", async () => { await mode("device"); await gate.wait(); return readPage(); });
        await check(prefix + "-old-intent-cancelled", async () => {
          await mode("account"); await gate.release(); await page.waitForTimeout(100);
          insist(!(await deviceRows()).some(row => row.family === "chapters" && row.id === ids.chapter), "The old device intent wrote after its actual mode authority was lost");
        });
        await check(prefix + "-page-retained", async () => { await mode("device"); insist(await readPage() === retained, "Mode selection changed the current reader page"); });
        const observed = await check(prefix + "-durable-progress", async () => {
          const row = await until(deviceRows, rows => rows.some(row => row.family === "chapters" && row.id === ids.chapter && row.page === retained && (retained !== 3 || row.completed)), "Reentered current device context failed to durably save the unchanged page");
          return row.find(row => row.family === "chapters" && row.id === ids.chapter);
        });
        await check(prefix + "-reload-progress", async () => {
          await page.reload({ waitUntil: "domcontentloaded" }); await ready(); const durable = (await deviceRows()).find(row => row.family === "chapters" && row.id === ids.chapter);
          insist(durable?.page === retained && (retained !== 3 || durable.completed), "Device page/completion did not survive reload");
        });
        await check(prefix + "-stable-revision", async () => { await page.waitForTimeout(500); const revision = await deviceRevision(); await stable(async () => await deviceRevision() === revision, "Same-context refresh caused an endless durable progress-save loop"); });
        return { actual_page: retained, completed: !!observed.completed, real_timer_or_lock_gate: gateKind, durable_after_reload: true };
      } finally { await gate.close(); }
    });
  }

  if (phase === "ui12-compact-feed") await step("UI12 compact feed: native505 rows continue beyond200 and bounded burst has a real resume", async () => {
    await check("compact-feed-seed-pagination", async () => { await ensurePagination(); const data = dependency("compact"); insist(data.total >= 501 && data.language === "es", "Compact fixture must contain over500 isolated Spanish chapters"); });
    const data = dependency("compact");
    await check("compact-feed-spanish-preferences", async () => { await chapterLanguagePreference("Spanish"); await page.setViewportSize({ width: 1440, height: 1800 }); });
    const nativeTotal = await check("compact-feed-fixture-total", async () => {
      const probe = await check("compact-feed-fixture-response-received", async () => api("/api/chapter/feed?tlang=es&limit=100", {}, (outcome, failure) => { report.compact_feed_http_outcome = outcome; report.compact_feed_database_failure = failure; h.save?.(); }));
      await check("compact-feed-fixture-http-200", async () => { insist(probe.status === 200, "The actual Spanish fixture feed did not return HTTP200"); });
      await check("compact-feed-fixture-total-integer", async () => { insist(Number.isSafeInteger(probe.body?.total), "The actual Spanish fixture feed did not return an integer total"); });
      await check("compact-feed-fixture-total-at-least-seeded", async () => { insist(probe.body.total >= data.total, "The actual Spanish fixture feed contains fewer chapters than the recorded compact seed"); });
      await check("compact-feed-fixture-total-within-known-scenarios", async () => { insist(probe.body.total <= data.total + (extras.bilingual?.manga_id ? 1 : 0), "The actual Spanish fixture feed contains chapters outside the recorded compact/bilingual scenarios"); });
      insist(probe.status === 200 && [data.total, data.total + (extras.bilingual?.manga_id ? 1 : 0)].includes(probe.body?.total), "Actual Spanish fixture has unrelated rows outside the recorded compact/bilingual scenarios"); return probe.body.total;
    });
    const seen = observe(request => requestPath(request, "/api/chapter/feed"));
    try {
      await check("compact-feed-open-updates", async () => { await open("/updates"); });
      await check("compact-feed-third-page", async () => { await until(() => seen.rows, rows => rows.some(row => row.body?.page >= 3 && row.status === 200), "Compact updates never requested page3 while cards remained trimmed"); });
      const more = page.getByRole("button", { name: "Load more updates", exact: true });
      await check("compact-feed-resume-visible", async () => { await more.waitFor({ state: "visible" }); });
      await check("compact-feed-bounded-burst", async () => { await seen.settle(); const admitted = seen.rows.filter(row => row.status === 200); insist(admitted.length === 5 && admitted.every((row, index) => row.body.page === index + 1), "Automatic compact burst did not stop after its finite four continuation reads"); });
      await check("compact-feed-compact-geometry", async () => { const rect = await page.locator("section footer").last().boundingBox(); insist(rect && rect.y <= 3600, "Compact footer did not stay inside the genuine expanded loading geometry"); });
      await check("compact-feed-trimmed-cards", async () => { insist(await page.getByRole("button", { name: "Show Less", exact: true }).count() === 0, "Test expanded cards instead of exercising compact continuation"); });
      await check("compact-feed-manual-terminal-page", async () => { await more.click(); await until(() => seen.rows, rows => rows.some(row => row.body?.page === Math.ceil(nativeTotal / 100)), "Explicit real Load more did not reach the remaining compact page"); await seen.settle(); });
      const pages = seen.rows.filter(row => row.status === 200).map(row => row.body.page);
      await check("compact-feed-unique-sequential-pages", async () => { insist(new Set(pages).size === pages.length && seen.maximum === 1, "Compact continuation issued duplicate or parallel native page reads"); });
      await check("compact-feed-scenario-membership", async () => { insist(seen.rows.every(row => row.status === 200 && row.body.total === nativeTotal && row.body.data.every(chapter => [data.manga_id, extras.bilingual?.manga_id].includes(chapter.relationships.manga.id))), "Feed was not the actual recorded compact/bilingual backend scenario"); });
      const compactIds = new Set(seen.rows.flatMap(row => row.body.data.filter(chapter => chapter.relationships.manga.id === data.manga_id).map(chapter => chapter.id)));
      await check("compact-feed-all-compact-chapters", async () => { insist(compactIds.size === data.total, "The505 genuine compact chapters did not all become reachable"); });
      await check("compact-feed-terminal-idle", async () => { await stable(async () => seen.rows.length === pages.length, "True compact exhaustion continued to request pages"); });
      await check("compact-feed-resize-retains-entries", async () => { await page.setViewportSize({ width: 1200, height: 800 }); insist(!await bodyHas("No results found"), "Acknowledged compact entries disappeared after resize"); });
      await check("compact-feed-back-restores-entries", async () => { await realm(); await search(); await page.goBack({ waitUntil: "domcontentloaded" }); await sameRealm(); insist(!await bodyHas("No results found"), "Back lost acknowledged public compact results"); });
      return { native_total: nativeTotal, recorded_compact_title_rows: compactIds.size, pages, maximum_parallel_reads: seen.maximum, cards_remained_trimmed: true, genuine_manual_resume: true, coherent_back_restore: true };
    } finally { await seen.close(); }
  });

  if (phase === "ui13-terminal-feed") {
    await step("UI13 terminal: real101→100 withdrawal retains acknowledged results and Back restoration", async () => {
      await ensurePagination();
      const data = dependency("terminal"); insist(data.total === 101 && data.language === "fr", "Terminal fixture must isolate101 French chapters");
      await chapterLanguagePreference("French");
      const seen = observe(request => requestPath(request, "/api/chapter/feed"));
      const held = await hold(origin + "/api/chapter/feed*", request => requestPath(request, "/api/chapter/feed") && new URL(request.url()).searchParams.get("page") === "2", true);
      try {
        await open("/updates"); await held.wait(); await until(() => seen.rows, rows => rows.some(row => row.body?.page === 1 && row.body?.data?.length === 100 && row.body?.total === 101), "Initial native terminal page did not acknowledge100 of101");
        const withdrawn = await privateControl("ui-regressions-withdraw", { scenario: "terminal" }); insist(withdrawn.ui_regressions?.terminal?.current_total === 100 || withdrawn.operation?.ui_regressions?.terminal?.current_total === 100, "Named production withdrawal did not verify its genuine100-row terminal state");
        const current = await api(`/api/chapter/feed?tlang=fr&limit=100&page=2`); insist(current.status === 200 && current.body.total === 100 && current.body.data.length === 0, "Genuine withdrawal did not produce the real terminal empty page");
        await held.release(); await seen.settle(); await until(() => seen.rows, rows => rows.some(row => row.body?.page === 2 && row.body?.total === 100 && row.body?.data?.length === 0), "Actual UI did not receive its real terminal empty response");
        insist(!await bodyHas("No results found") && await page.locator(`a[href^="/title/${data.manga_id}"]`).count() > 0, "Terminal continuation replaced acknowledged results with false initial emptiness");
        await stable(async () => seen.rows.filter(row => row.pathname === "/api/chapter/feed").length === 2, "Terminal empty page caused further continuation reads");
        await realm(); await search(); await page.goBack({ waitUntil: "domcontentloaded" }); await sameRealm(); insist(!await bodyHas("No results found"), "Back restored a false empty public terminal view");
        return { genuine_before: 101, genuine_after: 100, terminal_page: 2, terminal_rows: 0, acknowledged_rows_retained: true, native_withdrawal: true, restoration_coherent: true };
      } finally { await held.close(); await seen.close(); }
    });
    await step("UI13 initial empty: genuine zero-match language keeps the original empty state", async () => {
      await chapterLanguagePreference("Uzbek"); const seen = observe(request => requestPath(request, "/api/chapter/feed"));
      try { await open("/updates"); await page.getByText("No results found", { exact: true }).waitFor({ state: "visible" }); await seen.settle(); insist(seen.rows.some(row => row.status === 200 && row.body?.page === 1 && row.body?.total === 0 && row.body?.data?.length === 0), "Empty-state evidence lacked a genuine initial200-empty response"); return { genuine_initial_empty_200: true }; } finally { await seen.close(); }
    });
  }

  if (phase === "ui13-feed-authority") {
    for (const readiness of ["verified", "offline"]) await step(`UI13 public feed: delayed initial read settles across pending→${readiness}`, async () => {
      // Each fresh page owns a genuine fresh local initialization; the mounted
      // feed is reached through document navigation only for this startup case.
      const held = await hold(origin + "/api/chapter/feed*", request => requestPath(request, "/api/chapter/feed"));
      const scope = await hold(origin + "/api/protocol/capabilities", request => requestPath(request, "/api/protocol/capabilities"));
      try { await open(`/updates?initial=${marker}-${readiness}`); await Promise.all([held.wait(), scope.wait()]); await expand(panel()); await panel().getByText(/Scope verification: pending\./).waitFor({ state: "visible" }); if (readiness === "offline") await scope.fail(); else await scope.release(); await panel().getByText(new RegExp(`Scope verification: ${readiness}\\.`)).waitFor({ state: "visible" }); await held.release(); await until(() => page.locator('section a[href^="/chapter/"]').count(), async count => count > 0 || await bodyHas("No results found") || await page.getByRole("button", { name: "Retry feed", exact: true }).isVisible(), "Public updates remained an orphaned spinner after unrelated local initialization"); insist(await page.locator('section a[href^="/chapter/"]').count() > 0 || await bodyHas("No results found") || await page.getByRole("button", { name: "Retry feed", exact: true }).isVisible(), "Public read never reached results, genuine empty or actionable error"); return { actual_pending_observed: true, actual_pending_settlement: readiness, held_native_response: held.disposition, usable_terminal_state: true }; }
      finally { await held.close(); await scope.close(); }
    });
    await step("UI13 private feed: old account rows cannot attach after selecting device library", async () => {
      await signup(); await ready(); await mode("account");
      const follow = await api(`/api/user/follows/manga/${ids.manga}`, { method: "POST", data: { status: "reading", notification: true } }); insist(follow.status === 204, "Native account-follow prerequisite failed");
      const held = await hold(origin + "/api/user/follows/chapter/feed*", request => requestPath(request, "/api/user/follows/chapter/feed"));
      try { await open("/library/updates"); await held.wait(); await ready(); await mode("device"); await held.release(); await page.getByText("No results found", { exact: true }).waitFor({ state: "visible" }); insist(await page.locator(`section a[href^="/chapter/${ids.chapter}"]`).count() === 0, "Obsolete private account rows attached to the empty device feed"); return { actual_account_read_held: true, actual_device_selection: true, obsolete_rows_absent: true, current_device_empty_settled: true }; } finally { await held.close(); }
    });
  }

  if (phase === "ui14-entity-pagination") {
    await check("entity-seed-pagination", ensurePagination);
    const data = await check("entity-fixture", async () => dependency("entity"));
    for (const [kind, id] of [["group", data.group_id], ["author", data.author_id], ["user", data.user_id]]) for (const order of ["desc", "asc"]) await step(`UI14 ${kind} Uploads: genuine last page retains links back in ${order} order`, async () => {
      const probe = (suffix, operation) => check(`entity-${kind}-${order}-${suffix}`, operation);
      await probe("prerequisite", async () => { insist(typeof id === "string" && data.total === 101, "Isolated101-row entity fixture is required"); });
      const route = `/${kind}/${id}/uploads?page=2${order === "asc" ? "&order=asc" : ""}`, seen = observe(request => requestPath(request, "/api/chapter"));
      try {
        await probe("open", () => open(route));
        await probe("native-last-page", () => until(() => seen.rows, rows => rows.some(row => row.status === 200 && row.body?.page === 2 && row.body?.total === data.total && row.body?.data?.length === 1), "Entity last-page UI did not receive its real one-row native page"));
        // Bits UI gives the retained anchor aria-label="Page 1"; its visible
        // numeral alone is not the accessible name. Keep the footer/link scope.
        const back = page.locator("footer").getByRole("link", { name: "Page 1", exact: true });
        await probe("page-one-control", async () => { await back.waitFor({ state: "visible" }); await back.click(); });
        await probe("page-one-route", () => page.waitForURL(url => url.pathname === `/${kind}/${id}/uploads` && !url.searchParams.has("page")));
        await probe("native-first-page", () => until(() => seen.rows, rows => rows.some(row => row.body?.page === 1 && row.body?.data?.length === 100), "Existing entity paginator did not return to the real first page"));
        return { native_total: data.total, deep_link_page: 2, native_last_page_rows: 1, existing_page1_link_worked: true, order };
      } finally { await seen.close(); }
    });
    await step("UI14 entity high empty page: real count shrink retains recovery to page1", async () => {
      const probe = (suffix, operation) => check("entity-empty-" + suffix, operation);
      await probe("withdrawal", async () => { const removed = await privateControl("ui-regressions-withdraw", { scenario: "entity" }); insist(removed.ui_regressions?.entity?.current_total === 100 || removed.operation?.ui_regressions?.entity?.current_total === 100, "Named production entity withdrawal did not verify its genuine100-row state"); });
      const seen = observe(request => requestPath(request, "/api/chapter"));
      try {
        await probe("open", () => open(`/group/${data.group_id}/uploads?page=2`));
        await probe("empty-visible", () => page.getByText("No results found", { exact: true }).waitFor({ state: "visible" }));
        await probe("native-empty", async () => { await seen.settle(); insist(seen.rows.some(row => row.status === 200 && row.body?.page === 2 && row.body?.total === 100 && row.body?.data?.length === 0), "Empty high-page evidence lacked actual count-shrunk200"); });
        await probe("page-one-control", () => page.locator("footer").getByRole("link", { name: "Page 1", exact: true }).click());
        await probe("native-first-page", () => until(() => seen.rows, rows => rows.some(row => row.body?.page === 1 && row.body?.data?.length === 100), "Count-shrunk entity paginator lost the existing earlier-page recovery"));
        return { real101_to100_shrink: true, empty_high_page: true, page1_recovery: true };
      } finally { await seen.close(); }
    });
  }

  if (phase === "ui14-account-failures" || phase === "ui15-reader-feedback") {
    await step("Account failures: real verified registration and readonly client admission", async () => { await signup(); await ready(); await mode("account"); return { real_registration_verified: true }; });
    const client = await nativeClient();
    try {
      for (const status of [403, 429, 401]) for (const gesture of phase === "ui15-reader-feedback" ? ["reader-entry"] : ["chapter-read", "card-follow", "title-unfollow"]) await step(`UI${gesture === "reader-entry" ? "15" : "14"} ${gesture}: genuine${status} gives owned feedback without replay`, async () => {
        if (status === 401) { const auth = await api("/api/auth/check"); if (auth.status !== 200) await login(); await ready(); await mode("account"); }
        const probe = (suffix, operation) => phase === "ui14-account-failures" && status === 429 && gesture === "title-unfollow" ? check("account429-title-" + suffix, operation) : operation();
        const historyPath = `/api/user/history/manga/${ids.manga}`, followPath = `/api/user/follows/manga/${ids.manga}`, endpoint = gesture === "chapter-read" || gesture === "reader-entry" ? historyPath : followPath;
        if (gesture === "title-unfollow") await probe("prerequisite", async () => { const prepared = await api(followPath, { method: "POST", data: { status: "reading", notification: true } }); insist(prepared.status === 204, "Actual account follow prerequisite failed"); });
        if (gesture === "card-follow") { const removed = await api(followPath, { method: "DELETE" }); insist(removed.status === 204, "Actual cleared follow prerequisite failed"); }
        const title = (await api(`/api/manga/${ids.manga}`)).body?.title;
        await probe("open", () => open(gesture === "card-follow" ? `/search?title=${encodeURIComponent(title)}` : `/title/${ids.manga}`));
        await probe("account-mode", async () => { await ready(); await mode("account"); });
        const [before, followBefore] = await probe("durable-baseline", async () => [await api(historyPath), await api("/api/user/follows/manga?limit=100")]);
        const posts = observe(request => new URL(request.url()).pathname === endpoint && request.method() !== "GET");
        const gestureAction = async () => {
          if (status === 401) await context.clearCookies();
          if (gesture === "reader-entry") { await page.locator(`a[href="/chapter/${ids.chapter}"]`).first().click(); await page.waitForURL(url => url.pathname.startsWith(`/chapter/${ids.chapter}`)); }
          else if (gesture === "chapter-read") await page.locator("article").filter({ has: page.locator(`a[href="/chapter/${ids.chapter}"]`) }).getByRole("button", { name: "Mark as unread", exact: true }).click();
          else if (gesture === "title-unfollow") { await probe("trigger", () => selectListboxTrigger(page).filter({ hasText: /^\s*Reading\s*$/ }).click()); await probe("option", () => clickListboxOption(page, "None")); }
          else { const card = page.locator("article").filter({ has: page.locator(`a[href^="/title/${ids.manga}"]`) }).first(); await card.hover(); await card.locator("button").filter({ has: page.locator("svg.lucide-plus") }).first().click(); }
        };
        const denial = async () => {
          await probe("denial", () => until(() => posts.rows, rows => rows.some(row => row.status === status), `The actual ${gesture} request did not reach its genuine${status} backend denial`));
          const title = gesture === "reader-entry" ? "History save failed" : gesture === "chapter-read" ? "Read history update failed" : "Library update failed";
          await probe("feedback", async () => { await page.getByText(title, { exact: true }).last().waitFor({ state: "visible" }); await page.getByText(status === 401 ? /account session expired/ : status === 429 ? /request was rate limited/ : /does not have permission/).last().waitFor({ state: "visible" }); });
          await probe("no-replay", async () => { await stable(async () => posts.requests === 1, "Rejected native mutation automatically replayed"); await posts.settle(); insist(posts.requests === 1 && posts.rows.length === 1, "A single gesture issued repeated native writes"); });
        };
        const action = async () => { await gestureAction(); await denial(); };
        try {
          if (status === 403) await withBearer(endpoint, client.key, action);
          else if (status === 429) {
            // Let the real public chapter route load before saturating. Hold
            // only its actual admitted history/follow write before forwarding.
            const pattern = origin + endpoint + "*", match = request => new URL(request.url()).pathname === endpoint && request.method() !== "GET";
            if (phase === "ui14-account-failures" && gesture === "title-unfollow") {
              await runHeldAccountDenial({ hold, pattern, match, gesture: gestureAction, capacity: withCapacity, denial, check: probe });
            } else {
              const pressure = await hold(pattern, match, true);
              try { const attempt = action(); attempt.catch(() => {}); await pressure.wait(); await withCapacity(async () => { await pressure.release(); await attempt; }); }
              finally { await pressure.close(); }
            }
          } else await action();
        }
        finally { await posts.close(); }
        if (status === 401) await login();
        await probe("durable-unchanged", async () => {
          const after = await api(historyPath), followAfter = await api("/api/user/follows/manga?limit=100");
          insist(before.status === 200 && after.status === 200 && JSON.stringify(before.body) === JSON.stringify(after.body), "A rejected mutation changed genuine durable account history");
          insist(followBefore.status === 200 && followAfter.status === 200 && JSON.stringify(followBefore.body) === JSON.stringify(followAfter.body), "A rejected mutation changed genuine durable follows");
        });
        return { genuine_status: status, native_denial: status === 403 ? "actual owner readonly bearer lacks mutation permission" : status === 429 ? "bounded owned sockets exhausted actual native request permits" : "real cookie withdrawn with cached UI Session retained", actual_write_count: 1, feedback_visible: true, durable_state_unchanged: true };
      });
      if (phase === "ui15-reader-feedback") {
        await step("UI15 automatic account history: actual200 acknowledgement is durable once across reader rerenders", async () => {
          const endpoint = `/api/user/history/manga/${ids.manga}`; const cleared = await api(endpoint, { method: "POST", data: { unread_ids: [ids.chapter] } }); insist(cleared.status === 200, "Genuine account-history clear prerequisite failed");
          await open(`/title/${ids.manga}`); await ready(); await mode("account"); const posts = countPosts(endpoint);
          try {
            await page.locator(`a[href="/chapter/${ids.chapter}"]`).first().click(); await page.waitForURL(url => url.pathname.startsWith(`/chapter/${ids.chapter}`));
            await until(() => posts.rows, rows => rows.some(row => row.status === 200 && row.body), "Actual reader-entry history did not receive its required200 acknowledgement");
            const durable = await api(endpoint); insist(durable.status === 200 && Object.hasOwn(durable.body[ids.manga] ?? {}, ids.chapter), "Acknowledged reader entry did not persist actual account history");
            await settings(); await selectReaderPage("2", 2); await expand(panel()); const download = page.waitForEvent("download"); await panel().getByRole("button", { name: "Export device backup", exact: true }).click(); await download;
            await stable(async () => posts.requests === 1, "Harmless reader/page/local refresh replayed the successful account-history mutation");
            return { native_acknowledgement200: true, durable_account_history: true, same_authority_posts: posts.requests, actual_page_and_local_refresh: true };
          } finally { await posts.close(); }
        });
        for (const abandoned of [false, true]) await step(`UI15 automatic account history: real committed200 with lost delivery ${abandoned ? "after reader disposal" : "reports uncertainty"}`, async () => {
          const endpoint = `/api/user/history/manga/${ids.manga}`; insist((await api(endpoint, { method: "POST", data: { unread_ids: [ids.chapter] } })).status === 200, "Genuine ambiguous-write clear prerequisite failed");
          await open(`/title/${ids.manga}`); await ready(); await mode("account"); await realm(); const posts = countPosts(endpoint), held = await hold(origin + endpoint, request => requestPath(request, endpoint, "POST"));
          try {
            await page.locator(`a[href="/chapter/${ids.chapter}"]`).first().click(); await page.waitForURL(url => url.pathname.startsWith(`/chapter/${ids.chapter}`)); await held.wait();
            const durable = await api(endpoint); insist(durable.status === 200 && Object.hasOwn(durable.body[ids.manga] ?? {}, ids.chapter), "The withheld actual history response did not follow a genuine durable write");
            if (abandoned) { await readerSettings(); await page.locator('[data-sidebar="header"] a[href="/"]').click(); await page.waitForURL(url => url.pathname === "/"); }
            await held.fail(); insist(held.nativeStatus === 200, "Uncertain-write case did not withhold an actual successful backend acknowledgement");
            if (abandoned) { await stable(async () => !await bodyHas("History save failed"), "Obsolete reader feedback attached after actual disposal"); await sameRealm(); }
            else { await page.getByText("History save failed", { exact: true }).last().waitFor({ state: "visible" }); await page.getByText(/may have succeeded|may have completed|did not complete reliably|reliable response/).last().waitFor({ state: "visible" }); }
            await stable(async () => posts.requests === 1, "Lost-delivery automatic history mutation was replayed");
            return { actual_backend_acknowledgement: 200, durable_commit_before_delivery_fault: true, failed_delivery: held.disposition, actual_posts: posts.requests, current_uncertainty_feedback: !abandoned, obsolete_feedback_absent: abandoned };
          } finally { await held.close(); await posts.close(); }
        });
      }
    } finally { if ((await api("/api/auth/check")).status !== 200) await login(); const revoked = await api(`/api/client/${client.id}`, { method: "DELETE" }); insist(revoked.status === 204, "Synthetic readonly client cleanup failed"); privateCredentials.delete(client.key); }
  }

  if (phase === "ui14-home-failures") {
    await step("UI14 public home Grid: native request-capacity429 settles and current retry recovers", async () => {
      // Own at most32 incomplete, bounded JSON uploads. Json extraction is under
      // real admitted middleware; no account, successful write or fake429 exists.
      const held = await hold(origin + "/api/chapter/updates*", request => requestPath(request, "/api/chapter/updates"), true);
      const seen = observe(request => requestPath(request, "/api/chapter/updates"));
      try {
        await open("/"); await held.wait();
        await withCapacity(async () => {
          await held.release(); await page.getByRole("heading", { name: "Latest Updates", exact: true }).waitFor({ state: "visible" }); await page.getByText("Updates unavailable", { exact: true }).last().waitFor({ state: "visible" });
          await seen.settle(); insist(seen.rows.length === 3 && seen.rows.every(row => row.status === 429 && row.body?.code === "capacity_busy"), "Home Grid did not receive actual bounded final native429 retries");
          insist(!await bodyHas("No updates found"), "Native home read429 masqueraded as a successful empty catalogue");
        });
        const section = page.locator("section").filter({ has: page.getByRole("heading", { name: "Latest Updates", exact: true }) }); await section.getByRole("button", { name: "Retry updates", exact: true }).click();
        await until(() => seen.rows, rows => rows.some(row => row.status === 200 && Array.isArray(row.body?.data)), "Actual home Retry did not recover its genuine current safe read");
        return { genuine_saturation: true, owned_incomplete_json_sockets: 32, genuine_native_429_attempts: 3, no_false_empty: true, real_retry_200: true, all_capacity_socket_close_events_observed: true };
      } finally { await held.close(); await seen.close(); }
    });
    await step("UI14 public home Grid: the owned real PostgreSQL outage gives503 and recovery Retry gives200", async () => {
      const probe = (suffix, operation) => check("home-pg-" + suffix, operation);
      const capability = fixture.fixture_control?.pg_home_fault;
      await probe("capability", async () => { insist(capability?.fixture_only === true && capability.enabled === true && capability.outage_seconds === 15 && capability.independent_restart_watchdog === true && capability.allowed_browser_kind === phase, "UNIMPLEMENTED FIXTURE DEPENDENCY: reviewed opt-in genuine home503 control is required"); });
      const held = await hold(origin + "/api/chapter/updates*", request => requestPath(request, "/api/chapter/updates"), true), seen = observe(request => requestPath(request, "/api/chapter/updates"));
      let active = false, restored;
      try {
        // Healthy document/auth/catalogue initialization happens before the
        // deliberate outage. Only the real mounted update read is withheld.
        await probe("open", () => open(`/?ui14_pg_fault=${marker}`)); await probe("held", () => held.wait());
        const receipt = await probe("begin", () => privateControl("pg-home-fault-begin")), fault = receipt.pg_home_fault ?? receipt.operation;
        active = true;
        await probe("outage", async () => { insist(fault?.fixture_only === true && fault.status === "outage" && fault.active === true && fault.independent_restart_watchdog === true && fault.restart_deadline_unix * 1000 > Date.now() && fault.native503_observed?.status === 503 && fault.native503_observed?.code === "unavailable", "The real fixed-cluster fault receipt did not prove its current native503 outage"); });
        await probe("first-failure", async () => {
          await held.release(); await page.getByText("Updates unavailable", { exact: true }).last().waitFor({ state: "visible" }); await seen.settle();
          insist(seen.rows.length === 1 && seen.rows[0].status === 503 && seen.rows[0].body?.code === "unavailable", "The initial mounted home read did not settle its single genuine dependency503");
        });
        const section = page.locator("section").filter({ has: page.getByRole("heading", { name: "Latest Updates", exact: true }) });
        // The real HTTP wrapper settles503 once. Exercise two explicit visible
        // safe-read retries; do not mislabel these as automatic503 retries.
        await probe("retry-second", () => retryDeniedHomeRead(section, seen, until, insist));
        await probe("retry-third", () => retryDeniedHomeRead(section, seen, until, insist));
        await probe("three-denials", async () => {
          insist(seen.rows.length === 3 && seen.rows.every(row => row.status === 503 && row.body?.code === "unavailable"), "Mounted home Grid did not receive its three real final dependency503 reads");
          await stable(async () => seen.requests === 3, "Home dependency503 reads replayed without an explicit Retry gesture");
        });
        await probe("no-false-empty", async () => { insist(!await bodyHas("No updates found"), "Real home dependency503 became a successful empty catalogue"); });
        const resume = await probe("resume", () => privateControl("pg-home-fault-resume")); restored = resume.pg_home_fault ?? resume.operation; active = false;
        await probe("restored", async () => { insist(restored?.fixture_only === true && restored.status === "ready" && restored.active === false && restored.fixed_cluster_preserved === true && restored.postgres_binary_sha256 === fault.postgres_binary_sha256 && JSON.stringify(restored.cluster_config_hashes) === JSON.stringify(fault.cluster_config_hashes) && restored.native_recovery?.readyz_status === 200 && restored.native_recovery?.read_status === 200, "Real PostgreSQL recovery did not preserve its exact original cluster/executable/config and native readiness"); });
        const boundary = seen.rows.length; await probe("recovery-retry", () => section.getByRole("button", { name: "Retry updates", exact: true }).click());
        await probe("recovery-read", () => until(() => seen.rows.slice(boundary), rows => rows.some(row => row.status === 200 && Array.isArray(row.body?.data)), "Actual home Retry did not recover after the real fixed-cluster restart"));
        return { genuine_dependency503: true, native_503_attempts: 3, explicit_denied_retry_gestures: 2, automatic_503_retries: 0, fixed_outage_seconds: 15, independent_restart_watchdog: true, original_cluster_and_config_preserved: true, real_readyz_and_read200: true, visible_retry200: true, watchdog_terminal_cleanup_required_by_supervisor: true };
      } finally {
        try { if (active) { const receipt = await privateControl("pg-home-fault-resume"); restored = receipt.pg_home_fault ?? receipt.operation; insist(restored?.active === false && restored.native_recovery?.read_status === 200, "The real PG fault did not reach its required finally recovery"); } }
        finally { await held.close(); await seen.close(); }
      }
    });
    await step("UI14 private My Updates: cookie-withdrawn native401 settles with actual Retry", async () => {
      await signup(); await ready(); await mode("account"); await api(`/api/user/follows/manga/${ids.manga}`, { method: "POST", data: { status: "reading", notification: true } }); await open("/search"); await realm();
      const seen = observe(request => requestPath(request, "/api/user/follows/chapter/updates"));
      try { await context.clearCookies(); await spaLink("/"); await sameRealm(); const section = page.locator("section").filter({ has: page.getByRole("heading", { name: "My Updates", exact: true }) }); await section.getByText(/Catalogue request failed: HTTP 401/).waitFor({ state: "visible" }); await section.getByRole("button", { name: "Retry updates", exact: true }).waitFor({ state: "visible" }); await seen.settle(); insist(seen.rows.some(row => row.status === 401), "My Updates failure did not come from the genuine expired-cookie backend"); return { genuine_private401: true, permanent_skeleton_absent: true, current_retry_visible: true, same_document: true }; } finally { await seen.close(); }
    });
    await step("UI14 private My Updates: real restricted recovery cookie gives403 to a cached normal-account widget", async () => {
      const probe = (suffix, operation) => check("home-private403-" + suffix, operation);
      // Obtain the capability using the real visible request form and genuine
      // one-time mail, retaining the cached normal Session in this document.
      await probe("login", login);
      await probe("logout", async () => {
        await page.getByRole("link", { name: "Logout", exact: true }).first().click();
        await until(() => api("/api/auth/check"), value => value.status === 401, "Actual logout did not retire normal account authority before requesting recovery");
      });
      await probe("request-form", async () => { await open("/auth/recovery"); await page.locator('input[name="email"]').fill(account.email); await page.getByRole("button", { name: "Send Recovery Link", exact: true }).click(); });
      const mail = await probe("mail", () => until(async () => { const value = await context.request.get(sink() + `/mail/latest?to=${encodeURIComponent(account.email)}&kind=recovery`, { maxRedirects: 0 }); return value.ok() ? value.json() : undefined; }, value => value?.fixture_only === true && typeof value.link === "string", "Real recovery mail was not delivered to the synthetic fixture recipient"));
      const link = await probe("link", async () => {
        const link = new URL(mail.link); insist(link.origin === origin && link.pathname === "/auth/self-service/recovery" && !!link.searchParams.get("token"), "Actual recovery link escaped its exact native redemption endpoint"); remember(link.href); for (const [, token] of link.searchParams) remember(token); return link;
      });
      await probe("cached-session", async () => { await login(); await ready(); await mode("account"); await open("/search"); await realm(); });
      await probe("redemption", async () => { const redeemed = await context.request.get(link.href, { maxRedirects: 0, timeout: 12000 }); insist(redeemed.status() === 303 && new URL(redeemed.headers().location, origin).pathname === "/auth/recovery", "Actual one-time recovery redemption did not install its restricted server credential"); });
      await probe("restricted-auth", async () => { const capability = await api("/api/auth/check"); insist(capability.status === 200 && capability.body?.session_purpose === "recovery" && capability.body.identity === undefined && capability.body.follows === undefined && capability.body.preferences === undefined && !capability.body.roles?.length && !capability.body.permissions?.length, "Real recovery cookie acquired normal private account capability"); });
      await probe("native-denial", async () => { insist((await api("/api/user/follows/chapter/updates?limit=1")).status === 403, "Actual restricted recovery principal was not denied native private updates"); });
      const seen = observe(request => requestPath(request, "/api/user/follows/chapter/updates"));
      try {
        await probe("widget-open", async () => { await spaLink("/"); await sameRealm(); });
        const section = page.locator("section").filter({ has: page.getByRole("heading", { name: "My Updates", exact: true }) });
        const retry = section.getByRole("button", { name: "Retry updates", exact: true });
        await probe("feedback", async () => { await section.getByText(/Catalogue request failed: HTTP 403/).waitFor({ state: "visible" }); await retry.waitFor({ state: "visible" }); });
        await probe("single-read", async () => { await until(() => seen.rows, rows => rows.length === 1 && rows[0].status === 403, "Cached normal-account widget did not receive its real current restricted-cookie403"); await stable(async () => seen.requests === 1, "Private403 read was automatically replayed or stayed in a skeleton loop"); });
        await probe("retry", () => retry.click());
        await probe("repeated-denial", () => until(() => seen.rows, rows => rows.length === 2 && rows.every(row => row.status === 403), "Visible private Retry did not perform a new genuine currently denied safe read"));
        return { genuine_one_time_recovery_redemption303: true, actual_restricted_capability_without_identity: true, native_private403: true, retained_normal_ui_session: true, real_widget403: true, explicit_retry403: true, permanent_skeleton_absent: true, same_document: true };
      } finally { await seen.close(); await context.clearCookies(); }
    });
  }

  if (phase === "ui15-chapter-filter") await step("UI15 chapter language: English→Spanish→English and Back/Forward use actual applied fields", async () => {
    await ensureReaders();
    const data = dependency("bilingual"); await open(`/title/${data.manga_id}?tlang=en`); await realm();
    const form = page.locator("form").filter({ has: page.getByRole("button", { name: "Apply", exact: true }) });
    const trigger = () => selectListboxTrigger(form).first();
    const applied = async (code, label, chapter) => { await page.waitForURL(url => url.searchParams.getAll("tlang").join(",") === code); await trigger().filter({ hasText: label }).waitFor({ state: "visible" }); insist(!await form.getByRole("button", { name: "Apply", exact: true }).isEnabled(), "Apply baseline did not match the actual current applied query"); await page.locator(`a[href="/chapter/${chapter}"]`).first().waitFor({ state: "visible" }); };
    await applied("en", "English", data.english_chapter_id);
    await selectOptions(trigger(), ["Spanish"]); insist(await form.getByRole("button", { name: "Apply", exact: true }).isEnabled(), "Changed Spanish language could not be applied"); await form.getByRole("button", { name: "Apply", exact: true }).click(); await applied("es", "Spanish", data.spanish_chapter_id);
    await selectOptions(trigger(), ["English"]); insist(await form.getByRole("button", { name: "Apply", exact: true }).isEnabled(), "Returning to the initially selected language is still disabled"); await form.getByRole("button", { name: "Apply", exact: true }).click(); await applied("en", "English", data.english_chapter_id);
    await page.goBack({ waitUntil: "domcontentloaded" }); await applied("es", "Spanish", data.spanish_chapter_id); await page.goForward({ waitUntil: "domcontentloaded" }); await applied("en", "English", data.english_chapter_id); await sameRealm();
    await form.getByRole("button", { name: "Reset", exact: true }).click(); await page.waitForURL(url => !url.searchParams.has("tlang")); await trigger().filter({ hasText: "Any Language" }).waitFor({ state: "visible" });
    return { real_en_es_chapters: true, changed_selection_apply_usable: true, original_selection_reapplied: true, back_forward_fields_coherent: true, reset_works: true, same_document: true };
  });

  if (phase === "ui15-reader-geometry") {
    await ensureReaders();
    const readers = dependency("readers"); insist(Array.isArray(readers) && readers.length >= 2, "Real four-portrait and eight-mixed chapters are required");
    for (const data of readers) for (const offset of [0, 1]) for (const rtl of [false, true]) await step(`UI15 spread selector: native${data.pages.length} pages offset${offset} ${rtl ? "RTL" : "LTR"}`, async () => {
      const native = await api(`/api/chapter/${data.chapter_id}`); insist(native.status === 200 && native.body?.data?.length === data.pages.length && JSON.stringify(native.body.data.map(row => row.dimensions)) === JSON.stringify(data.pages.map(row => row.dimensions)), "Reader fixture geometry differs from its real native Chapter.Get");
      await open(`/chapter/${data.chapter_id}/1`); await settings(rtl ? "Right-to-left" : "Left-to-right", true);
      const slider = page.locator("div").filter({ has: page.locator("label", { hasText: "Spread Offset" }) }).filter({ has: page.getByRole("slider") }).last().getByRole("slider"); await slider.focus(); await slider.press("Home"); if (offset) await slider.press("ArrowRight");
      const spreads = []; for (let index = 0; index < data.pages.length; index++) { const current = data.pages[index], next = data.pages[index + 1]; if (index < offset || current.dimensions[0] > current.dimensions[1] || !next || next.dimensions[0] > next.dimensions[1]) spreads.push([index + 1]); else { spreads.push([index + 1, index + 2]); index++; } }
      for (const spread of spreads) { const label = spread.length === 1 ? String(spread[0]) : (rtl ? [...spread].reverse() : spread).join("-"); await selectReaderPage(label, spread[0]); const value = (await readerSelector().first().innerText()).replace(/\s/g, ""); insist(value.startsWith(label + "/"), "The visible spread selector label differs from its chosen spread"); await until(() => page.locator("main img").filter({ visible: true }).evaluateAll(nodes => nodes.filter(node => node.src.startsWith("blob:") && node.complete && node.naturalWidth > 0).length), count => count >= spread.length, "The selected genuine image spread never decoded visibly"); }
      await page.getByRole("button", { name: "Single", exact: true }).click(); await selectReaderPage("2", 2);
      return { native_pages: data.pages.length, actual_landscape_dimensions: data.pages.filter(row => row.dimensions[0] > row.dimensions[1]).length, offset, rtl, all_advertised_spreads_selected: spreads.length, real_single_index_regression: true };
    });
    const data = readers[0];
    for (const viewport of [{ width: 1200, height: 900 }, { width: 800, height: 900 }, { width: 390, height: 844 }]) for (const rtl of [false, true]) await step(`UI15 tap rectangle: closed left panel ${viewport.width}px ${rtl ? "RTL" : "LTR"}`, async () => {
      await page.setViewportSize(viewport); await open(`/chapter/${data.chapter_id}/2`); await settings(rtl ? "Right-to-left" : "Left-to-right", false); await page.getByRole("button", { name: "Left", exact: true }).click(); await closeReaderPanel();
      const surface = page.locator("main > div").filter({ visible: true }).first(); const rectangle = await surface.boundingBox(); insist(rectangle && rectangle.width > 100, "Real reading surface has no tappable rectangle");
      const y = Math.max(80, Math.min(viewport.height - 80, rectangle.y + rectangle.height / 2));
      await page.mouse.click(rectangle.x + rectangle.width * (rtl ? .25 : .75), y); await until(readPage, value => value === 3, "Next-half tap on the actual closed-panel rectangle selected the wrong side");
      const next = await surface.boundingBox(); await page.mouse.click(next.x + next.width * (rtl ? .75 : .25), Math.max(80, Math.min(viewport.height - 80, next.y + next.height / 2))); await until(readPage, value => value === 2, "Previous-half tap on the actual closed-panel rectangle selected the wrong side");
      return { viewport: viewport.width, closed_left_panel: true, rtl, observed_surface_left: rectangle.x, observed_surface_width: rectangle.width, genuine_next_and_previous_taps: true };
    });
  }

  if (phase === "ui15-16-empty-tabs") {
    await step("UI15 Recommendations: actual200-empty read settles and Back repeats current availability", async () => {
      const id = ids.second_manga, seen = observe(request => requestPath(request, `/api/manga/${id}/recommendations`));
      try { await open(`/title/${id}`); await realm(); await page.getByRole("link", { name: "Recommendations", exact: true }).first().click(); await page.getByText("No recommendations found", { exact: true }).waitFor({ state: "visible" }); await seen.settle(); insist(seen.rows.some(row => row.status === 200 && row.body?.data?.length === 0), "Recommendations empty evidence did not come from real acknowledged backend data"); await search(); await page.goBack({ waitUntil: "domcontentloaded" }); await page.getByText("No recommendations found", { exact: true }).waitFor({ state: "visible" }); await sameRealm(); await seen.settle(); insist(seen.rows.length >= 2, "Recommendations restored a historical DTO without its current availability read"); return { actual200_empty: true, empty_spinner_absent: true, current_read_after_back: true }; } finally { await seen.close(); }
    });
    await step("UI16 Covers: remove the real last cover while its stale parent tab stays visible", async () => {
      await signup("staff"); await open(`/title/${ids.manga}`); await realm(); const coversTab = page.getByRole("link", { name: "Covers", exact: true }).first(); await coversTab.waitFor({ state: "visible" });
      const before = await api(`/api/manga/${ids.manga}/covers`); insist(before.status === 200 && before.body.length === 1 && before.body[0].id === ids.cover, "Last-cover fixture prerequisite is not the actual lone native cover");
      const deleted = await api(`/api/cover/${ids.cover}`, { method: "DELETE" }); insist(deleted.status === 204, "Real authorized last-cover deletion failed");
      const seen = observe(request => requestPath(request, `/api/manga/${ids.manga}/covers`));
      try { await coversTab.click(); await page.getByText("No covers found", { exact: true }).waitFor({ state: "visible" }); await seen.settle(); insist(seen.rows.some(row => row.status === 200 && Array.isArray(row.body) && row.body.length === 0), "Covers empty-state evidence lacked genuine200[] after real deletion"); await sameRealm(); return { initially_visible_cover_tab: true, actual_last_cover_delete204: true, actual_empty_cover_list200: true, settled_empty_tab: true, same_parent_lifetime: true }; } finally { await seen.close(); }
    });
  }

  if (phase === "ui16-home-preferences") await step("UI16 home: acknowledged native rating changes invalidate both actual home queries and caches", async () => {
    await ensurePagination();
    const data = dependency("preference"); await signup(); await open("/settings/preferences");
    for (const id of new Set([data.safe_id, data.erotic_id, data.pornographic_id, data.language_title_id])) { const viewed = await api(`/api/analytics/manga/${id}/views`, { method: "POST" }); insist(viewed.status === 204, "Actual native analytics did not establish the preference target's genuine ranking activity"); }
    for (const selector of ["#safe", "#suggestive", "#erotic", "#hentai"]) await checked(selector, true); await savePreferences(); await realm();
    const seen = observe(request => request.method() === "GET" && ["/api/manga", "/api/manga/top"].includes(new URL(request.url()).pathname));
    try {
      await spaLink("/"); await sameRealm(); await until(() => seen.rows, rows => rows.filter(row => row.status === 200 && row.body?.data?.length).length >= 2, "Genuine home preference fixture did not populate both native public widgets");
      const home = page.locator("section").filter({ has: page.getByRole("heading", { name: /Most Read Titles|Recently Added Titles/ }) });
      insist(await home.locator(`a[href^="/title/${data.erotic_id}"]`).count() + await home.locator(`a[href^="/title/${data.pornographic_id}"]`).count() > 0, "Actual home fixture did not expose an enabled adult-rated target before exclusion");
      await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click(); await checked("#erotic", false); await checked("#hentai", false); await savePreferences(); const boundary = seen.rows.length;
      await spaLink("/"); await sameRealm(); await until(() => seen.rows.slice(boundary), rows => rows.filter(row => row.status === 200).length >= 2, "Revisited home reused old widgets without current acknowledged preference reads");
      insist(await home.locator(`a[href^="/title/${data.erotic_id}"]`).count() === 0 && await home.locator(`a[href^="/title/${data.pornographic_id}"]`).count() === 0, "Newly excluded adult title remained in a home module cache");
      for (const row of seen.rows.slice(boundary)) insist(!(row.body?.data ?? []).some(title => ["erotica", "pornographic"].includes(title.content_rating)), "Native home result contains a newly excluded content rating");
      await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click(); await checked("#safe", false); await checked("#suggestive", false); await savePreferences(); const noneBoundary = seen.rows.length;
      await spaLink("/"); await until(() => seen.rows.slice(noneBoundary), rows => rows.filter(row => row.status === 200).length >= 2, "All-off preference did not cause current home queries");
      insist(seen.rows.slice(noneBoundary).every(row => new URLSearchParams(row.query).getAll("contentRating").join(",") === "__none__" && row.body?.data?.length === 0), "Most Read/Recently Added did not honor the actual all-off native filter convention");
      await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click(); await checked("#safe", true); await savePreferences(); const restoredBoundary = seen.rows.length;
      await spaLink("/"); await until(() => seen.rows.slice(restoredBoundary), rows => rows.filter(row => row.status === 200 && row.body?.data?.length).length >= 2, "An acknowledged-empty cache prevented current widened preference results");

      // These are the actual existing original-language and tag-blacklist
      // controls. The tagged target and its ranking come from real native rows
      // plus the acknowledged analytics events above, never invented counters.
      insist(typeof data.tag_id === "string" && data.tagged_title_id === data.language_title_id && data.language === "es", "UNIMPLEMENTED FIXTURE DEPENDENCY: the real tagged Spanish preference title is required");
      await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click();
      const languages = page.locator("div.flex.flex-col").filter({ has: page.locator("strong", { hasText: "Original Language Filter" }) }).last(); await selectOptions(selectListboxTrigger(languages), ["Spanish"]); await savePreferences(); const languageBoundary = seen.rows.length;
      await spaLink("/"); await until(() => seen.rows.slice(languageBoundary), rows => rows.filter(row => row.status === 200 && new URLSearchParams(row.query).getAll("lang").join(",") === "es").length >= 2, "Both home widgets did not issue their actual acknowledged Spanish-language query");
      insist(await home.locator(`a[href^="/title/${data.language_title_id}"]`).count() >= 2, "The real Spanish target is absent from one of the two current home widgets");
      await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click(); await checked(`#tag-${data.tag_id}`, true); await savePreferences(); const tagBoundary = seen.rows.length;
      await spaLink("/"); await until(() => seen.rows.slice(tagBoundary), rows => rows.filter(row => row.status === 200 && new URLSearchParams(row.query).getAll("tagx").includes(data.tag_id)).length >= 2, "Both home widgets did not issue the actual saved blacklist query");
      insist(await home.locator(`a[href^="/title/${data.tagged_title_id}"]`).count() === 0 && seen.rows.slice(tagBoundary).every(row => !(row.body?.data ?? []).some(title => title.id === data.tagged_title_id)), "The saved genuine tag blacklist left its previously admitted title in a home cache");

      // A real older Most Read response is already acknowledged at the backend
      // when settings change. Releasing its delivery after a new current read
      // must not restore the formerly enabled adult cards into this SPA.
      await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click(); await selectOptions(selectListboxTrigger(languages), []); await checked(`#tag-${data.tag_id}`, false); await checked("#erotic", true); await checked("#hentai", true); await savePreferences();
      const older = await hold(origin + "/api/manga/top*", request => requestPath(request, "/api/manga/top"));
      try {
        await spaLink("/"); await older.wait();
        await page.getByRole("link", { name: "Settings", exact: true }).first().click(); await page.getByRole("link", { name: "Preferences", exact: true }).click(); await checked("#erotic", false); await checked("#hentai", false); await savePreferences(); const currentBoundary = seen.rows.length;
        await spaLink("/"); await until(() => seen.rows.slice(currentBoundary), rows => rows.filter(row => row.status === 200).length >= 2, "Current home queries did not settle while an obsolete native response was still held"); await older.release(); insist(older.nativeStatus === 200, "Older home result was not an actual acknowledged native response");
        await stable(async () => await home.locator(`a[href^="/title/${data.erotic_id}"]`).count() === 0 && await home.locator(`a[href^="/title/${data.pornographic_id}"]`).count() === 0, "The obsolete native home response restored newly excluded cards");
      } finally { await older.close(); }
      await page.goBack({ waitUntil: "domcontentloaded" }); await page.goForward({ waitUntil: "domcontentloaded" }); await sameRealm();
      return { native_preferences_saved: true, genuine_ranking_view_events: true, previously_enabled_adult_target_observed: true, current_home_queries_refetched: true, excluded_cards_absent: true, all_off_actual_none_filter: true, acknowledged_empty_cache_invalidated: true, original_language_applied: true, native_tag_blacklist_applied: true, obsolete_acknowledged_home_response_fenced: true, same_document: true };
    } finally { await seen.close(); }
  });

  if (phase === "ui16-search-cap") {
    await ensurePagination();
    const data = dependency("search"); insist(data.total >= 10000 && typeof data.query === "string", "A genuine10000-title bounded metadata scenario is required");
    for (const size of [28, 42, 56, 70, 84, 98]) await step(`UI16 Search cap: native10000 total has an ordinary terminal page link at limit${size}`, async () => {
      await open("/settings/preferences"); await selectListboxTrigger(page).filter({ hasText: /Items Per Page:/ }).click(); await clickListboxOption(page, String(size)); await savePreferences();
      const seen = observe(request => requestPath(request, "/api/manga"));
      try { await open(`/search?title=${encodeURIComponent(data.query)}`); await until(() => seen.rows, rows => rows.some(row => row.status === 200 && row.body?.total === data.total && row.body?.limit === size), "Actual public search did not acknowledge its genuine capped-fixture total and selected page size"); const last = Math.ceil(Math.min(data.total, 10000) / size); const link = page.getByRole("link", { name: String(last), exact: true }).first(); await link.waitFor({ state: "visible" }); await realm(); await link.click(); await page.waitForURL(url => Number(url.searchParams.get("page")) === last); await until(() => seen.rows, rows => rows.some(row => row.body?.page === last && row.body?.limit === size && row.body?.data?.length > 0), "Visible terminal link did not reach its real last partial/native page"); const receipt = seen.rows.findLast(row => row.body?.page === last); const expected = Math.min(size, data.total - (last - 1) * size); insist(receipt.body.data.length === expected, "Actual last-page row count differs from the native uncapped catalogue contract"); await page.goBack({ waitUntil: "domcontentloaded" }); await page.goForward({ waitUntil: "domcontentloaded" }); await sameRealm(); await link.waitFor({ state: "visible" }); return { genuine_total: data.total, selected_limit: size, ordinary_last_link: last, native_last_rows: receipt.body.data.length, last_and_current_agree: true, back_forward_same_document: true }; } finally { await seen.close(); }
    });
  }
  if (phase === actorFollowPhase) await runActorFollowJourneys({ phase, page, origin, ids, api, step, check, insist, open, signup, hold, requestPath, until, stable, realm, sameRealm });
  finite();
}
