#!/usr/bin/env node
// Real Chromium acceptance against the disposable local platform fixture.
// Never record credentials, cookies, authentication query strings or request bodies.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { loadBrowserRuntime } from "./ci/browser-runtime.mjs";
import { observeSeedTitleReadiness, waitForSeedTitleReady } from "./startup-readiness-probe.mjs";
import { createJourneyCheck } from "./journey-checks.mjs";
import { installRunnerLifecycle } from "./runner-lifecycle.mjs";
import { createBrowserDenyProxy } from "./browser-deny-proxy.mjs";
import { runJourneys } from "./browser-journeys.mjs";
import { runSignedJourneys, signedJourneyPhases } from "./browser-signed-journeys.mjs";
import { runUi10Journeys, ui10JourneyPhases } from "./browser-ui10-journeys.mjs";
import { runUi11To16Journeys, ui11To16JourneyPhases } from "./browser-ui11-16-journeys.mjs";
import { runPublicUploadRecovery, publicUploadRecoveryPhase } from "./public-upload-recovery-browser.mjs";
import { runPublicFeatureRecovery, publicFeatureRecoveryPhase } from "./public-feature-recovery-browser.mjs";
import { runPublicSignedFeatures, publicSignedFeaturePhase } from "./public-signed-feature-browser.mjs";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// The supervisor strips NODE_PATH. The portable seam permits only the exact
// reviewed local package or the isolated lock-integrity-installed CI generation.
const { chromium, executablePath, description: browserDescription } = loadBrowserRuntime();
const phase = process.env.BROWSER_PHASE ?? "startup";
const fixture = process.env.BROWSER_FIXTURE_FILE ? JSON.parse(fs.readFileSync(process.env.BROWSER_FIXTURE_FILE, "utf8")) : {};
const origin = new URL(process.env.BROWSER_ORIGIN ?? fixture.origin ?? fixture.platform_origin ?? "http://127.0.0.1:0").origin;
const local = new URL(origin);
if (local.protocol !== "http:" || !["127.0.0.1", "localhost"].includes(local.hostname) || local.port === "0") throw new Error("A verified loopback fixture origin is required");
const allowedHttpOrigins = new Set([origin]);
const allowedWsUrls = new Set();
for (const field of ["protocol_origin", "mail_origin", "control_origin"]) if (fixture[field]) {
  const value = new URL(fixture[field]);
  if (value.protocol !== "http:" || !["127.0.0.1", "localhost"].includes(value.hostname) || !value.port || value.username || value.password || value.pathname !== "/" || value.search || value.hash) throw Error(`Invalid local fixture ${field}`);
  allowedHttpOrigins.add(value.origin);
}
if (fixture.relay_origin) {
  const value = new URL(fixture.relay_origin);
  if (value.protocol !== "ws:" || !["127.0.0.1", "localhost"].includes(value.hostname) || !value.port || value.username || value.password || value.pathname !== "/" || value.search || value.hash || !allowedHttpOrigins.has("http://" + value.host)) throw Error("Invalid same-fixture original WebSocket endpoint");
  allowedWsUrls.add(value.href);
}
if (fixture.protocol_recipients !== undefined) {
  const recipients = fixture.protocol_recipients, origins = new Set(), keys = new Set();
  if (!Array.isArray(recipients) || recipients.length < 1 || recipients.length > 2) throw Error("Expected one or two exact genuine protocol recipients");
  for (const recipient of recipients) {
    if (!recipient || typeof recipient.origin !== "string" || typeof recipient.relay_origin !== "string" || typeof recipient.service_key !== "string" || !/^[0-9a-f]{64}$/.test(recipient.service_key)) throw Error("Invalid recorded protocol recipient");
    const http = new URL(recipient.origin), ws = new URL(recipient.relay_origin);
    if (http.protocol !== "http:" || !["127.0.0.1", "localhost"].includes(http.hostname) || !http.port || http.port === "0" || http.username || http.password || http.pathname !== "/" || http.search || http.hash || ws.protocol !== "ws:" || ws.host !== http.host || ws.username || ws.password || ws.pathname !== "/" || ws.search || ws.hash || origins.has(http.origin) || keys.has(recipient.service_key)) throw Error("Invalid or duplicate exact loopback recipient HTTP/root WebSocket identity");
    origins.add(http.origin); keys.add(recipient.service_key);
    allowedHttpOrigins.add(http.origin); allowedWsUrls.add(ws.href);
  }
  const primary = recipients[0];
  if (!fixture.protocol_origin || new URL(primary.origin).origin !== new URL(fixture.protocol_origin).origin || primary.service_key !== fixture.service_key || !fixture.relay_origin || new URL(primary.relay_origin).href !== new URL(fixture.relay_origin).href) throw Error("Primary protocol recipient differs from the verified fixture");
}
const runId = process.env.BROWSER_RUN_ID ?? "first";
if (!/^[a-zA-Z0-9_-]{1,60}$/.test(runId)) throw new Error("Unsafe evidence run identifier");
const output = path.join(root, "test-evidence/frontend/browser", `${phase}-${runId}`);
fs.mkdirSync(output, { recursive: true });
const report = { phase, origin, browser: browserDescription, source_artifact: "artifacts/frontend", started_utc: new Date().toISOString(), steps: [], pages: [], external_requests_blocked: [], proxy_requests_blocked: [], page_errors: [], console_errors: [], csp_violations: [], response_errors: [], screenshots: [], limitations: ["Supplied archive lacks an independently runnable original-render baseline. Source/DOM/style preservation and browser operation are distinct from pixel parity."] };
if (phase === "startup") { report.startup_network = []; report.startup_observations_truncated = false; }
let seedTitleCapture = false;
if (phase === "startup") report.seed_title_network = [];
const safeUrl = value => { try { const url = new URL(value); return url.origin + url.pathname + (url.pathname.startsWith("/api/") && !/auth|client/.test(url.pathname) ? url.search : ""); } catch { return "invalid URL"; } };
const redactions = new Set();
const safeError = error => {
	let value = String(error?.message ?? error).replace(/https?:\/\/[^\s'"<>]+/g, safeUrl);
	for (const secret of redactions) value = value.split(secret).join("[redacted synthetic credential]");
	return value.length > 9000 ? value.slice(0, 2000) + "\n[diagnostic middle omitted]\n" + value.slice(-7000) : value;
};
function save() { fs.writeFileSync(path.join(output, "acceptance.json"), JSON.stringify(report, null, 2) + "\n"); }
const runnerStage = installRunnerLifecycle(report, save);
const check = createJourneyCheck(report, save);
function insist(condition, message) { if (!condition) throw new Error(message); }
async function step(name, action) {
	const entry = { name, started_utc: new Date().toISOString() };
	report.steps.push(entry); save();
	try { entry.observation = await action(); entry.status = "passed"; }
	catch (error) { entry.status = "failed"; entry.error = safeError(error); }
	entry.completed_utc = new Date().toISOString(); save();
	process.stdout.write(JSON.stringify({ phase, step: name, status: entry.status }) + "\n");
}
let browser, context;
report.cleanup = { context_created: false, context_closed: false, browser_created: false, browser_closed: false, page_created: false, proxy_closed: false, failures: [] };
const { proxy, close: closeDenyProxy } = createBrowserDenyProxy(report, safeUrl, save);
try {
	await new Promise((resolve, reject) => { proxy.once('error', reject); proxy.listen(0, "127.0.0.1", () => { proxy.off('error', reject); resolve(); }); });
	const proxyAddress = proxy.address();
	runnerStage("launching-browser");
	browser = await chromium.launch({ executablePath, headless: true, chromiumSandbox: true,
		proxy: { server: `http://127.0.0.1:${proxyAddress.port}`, bypass: "127.0.0.1,localhost" },
		args: ["--disable-background-networking", "--disable-component-update", "--disable-sync", "--no-first-run", "--disable-default-apps", "--disable-features=MediaRouter"] });
	report.cleanup.browser_created = true;
	runnerStage("creating-context");
	context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1, serviceWorkers: phase === "pwa" ? "allow" : "block" });
	report.cleanup.context_created = true;
	await context.route("**/*", async route => {
		const request = route.request(); const target = new URL(request.url());
		if (["data:", "blob:"].includes(target.protocol) || allowedHttpOrigins.has(target.origin)) return route.continue();
		report.external_requests_blocked.push({ method: request.method(), url: safeUrl(request.url()) });
		return route.abort("blockedbyclient");
	});
	await context.routeWebSocket("**/*", route => {
    if (allowedWsUrls.has(new URL(route.url()).href)) route.connectToServer();
    else { report.external_requests_blocked.push({ method: "WEBSOCKET", url: safeUrl(route.url()) }); route.close({ code: 1008, reason: "Outside exact local fixture" }); }
  });
	await context.addInitScript(() => { window.__acceptanceCsp = []; document.addEventListener("securitypolicyviolation", event => { window.__acceptanceCsp.push({ directive: event.effectiveDirective, blocked_uri: event.blockedURI?.split("?")[0], disposition: event.disposition }); }); });
	context.on("page", page => {
		page.setDefaultTimeout(7000); page.setDefaultNavigationTimeout(15000);
		page.on("pageerror", error => report.page_errors.push({ url: safeUrl(page.url()), error: safeError(error), error_class: error.name }));
		page.on("console", message => { if (message.type() === "error") report.console_errors.push({ url: safeUrl(page.url()), error: safeError(message.text()) }); });
		page.on("response", response => { if (response.status() >= 400) report.response_errors.push({ status: response.status(), method: response.request().method(), url: safeUrl(response.url()) }); });
		if (phase === "startup") {
			const observe = value => { if (report.startup_network.length < 256) report.startup_network.push(value); else report.startup_observations_truncated = true; };
			const seedObserve = value => { if (seedTitleCapture) { if (report.seed_title_network.length < 64) report.seed_title_network.push(value); else report.seed_title_network_truncated = true; } };
			page.on("response", response => { const value = { url: safeUrl(response.url()), resource: response.request().resourceType(), status: response.status(), content_type: response.headers()["content-type"] ?? "" }; observe(value); if (["document", "fetch", "xhr"].includes(value.resource)) seedObserve(value); });
			page.on("requestfailed", request => { const value = { url: safeUrl(request.url()), resource: request.resourceType(), status: 0, failure: request.failure()?.errorText ?? "" }; observe(value); seedObserve(value); });
		}
	});
	const page = await context.newPage();
	report.cleanup.page_created = true;
	runnerStage("running-checks");
	async function open(route) { const response = await page.goto(origin + route, { waitUntil: "domcontentloaded" }); insist(response?.status() === 200, `Navigation returned ${response?.status()}`); if (phase === "startup" && route === "/title/SeedManga1") await waitForSeedTitleReady(page); else await page.waitForTimeout(1200); return response; }
	async function screenshot(name) { const target = path.join(output, name + ".png"); await page.screenshot({ path: target, fullPage: true }); report.screenshots.push(path.relative(root, target)); }
	async function snapshot() {
		const state = await page.evaluate(() => ({ title: document.title, text: document.body.innerText.slice(0, 1400), background: getComputedStyle(document.body).backgroundColor, font_family: getComputedStyle(document.body).fontFamily, stylesheets: [...document.styleSheets].map(sheet => sheet.href).filter(Boolean), horizontal_overflow: document.documentElement.scrollWidth > innerWidth + 1, csp: window.__acceptanceCsp ?? [] }));
		report.csp_violations.push(...state.csp.map(value => ({ ...value, page: safeUrl(page.url()) }))); delete state.csp;
		return { route: safeUrl(page.url()), ...state };
	}
	if (phase === "startup") {
		await step("homepage bootstraps under exact generated CSP", async () => {
			const response = await open("/"); const policy = response.headers()["content-security-policy"] ?? "";
			const expected = JSON.parse(fs.readFileSync(path.join(root, "test-evidence/frontend/bootstrap-csp.json"), "utf8"));
			insist(expected.hash_sources.every(value => policy.includes(value)), "Served CSP is missing current exact bootstrap hash");
			insist(!policy.includes("unsafe-eval") && !/script-src[^;]*unsafe-inline/.test(policy), "Script CSP unexpectedly weakened");
			await page.getByRole("link", { name: "Advanced Search", exact: true }).waitFor({ state: "visible" });
			const observation = await snapshot(); report.pages.push(observation); await screenshot("homepage");
			insist(!report.page_errors.length, "Homepage has JavaScript page errors");
			return { title: observation.title, current_bootstrap_hash_allowed: true, external_tracking_disallowed: true, stylesheets: observation.stylesheets.length };
		});
		const routes = ["/search", "/groups", "/users", "/popular", "/trending", "/updates", "/announcements", "/about", "/guidelines", "/terms", "/privacy", "/auth/login", "/auth/register", "/title/random"];
		const ids = fixture.ids ?? {};
		for (const [kind, prefix] of [["manga", "/title/"], ["group", "/group/"], ["chapter", "/chapter/"]]) if (ids[kind]) routes.push(prefix + ids[kind]);
		for (const route of routes) await step(`fresh deep link ${route}`, async () => {
			seedTitleCapture = route === "/title/SeedManga1";
			const before = report.page_errors.length; await open(route); const observation = await snapshot(); report.pages.push(observation);
			if (route === "/title/SeedManga1") { report.seed_title_readiness = await observeSeedTitleReadiness(page, observation.text.trim().length > 20); save(); }
			seedTitleCapture = false;
			insist(observation.text.trim().length > 20, "Page never rendered meaningful content");
			insist(report.page_errors.length === before, "Deep link has JavaScript page errors");
			if (["/search", "/announcements", "/title/random"].includes(route)) await screenshot(route.slice(1).replaceAll("/", "-"));
			return { title: observation.title, rendered_text_length: observation.text.length, horizontal_overflow: observation.horizontal_overflow };
		});
		await step("missing built/static assets produce real 404", async () => {
			const results = [];
			for (const route of ["/_app/immutable/no-such-script.js", "/fonts/no-such-font.woff2", "/images/no-such-image.png"]) {
				const response = await context.request.get(origin + route); results.push({ route, status: response.status(), content_type: response.headers()["content-type"] }); insist(response.status() === 404, `Missing asset returned ${response.status()}`);
			}
			return results;
		});
	} else await (phase === publicSignedFeaturePhase ? runPublicSignedFeatures : phase === publicFeatureRecoveryPhase ? runPublicFeatureRecovery : phase === publicUploadRecoveryPhase ? runPublicUploadRecovery : signedJourneyPhases.includes(phase) ? runSignedJourneys : ui10JourneyPhases.includes(phase) ? runUi10Journeys : ui11To16JourneyPhases.includes(phase) ? runUi11To16Journeys : runJourneys)({ phase, page, context, origin, fixture, step, check, save, insist, open, report, redactions, runnerStage });
	insist(report.page_errors.length === 0, "Application JavaScript page errors were observed during the journey");
} catch (error) { runnerStage.caught(error); report.harness_error = safeError(error); }
finally {
	const cleanupFailure = (stage, error) => { let detail; try { detail = safeError(error); } catch { detail = 'Owned cleanup error unavailable'; } report.cleanup.failures.push({ stage, error: detail }); };
	const cleanupStage = stage => { try { runnerStage(stage); } catch (error) { cleanupFailure('evidence-' + stage, error); } };
	cleanupStage("closing-context");
	if (context) try { await context.close(); report.cleanup.context_closed = true; }
	catch (error) { cleanupFailure('context', error); }
	if (report.public_upload) report.public_upload.actual_context_closed = report.cleanup.context_closed;
	if (report.public_features) report.public_features.actual_context_closed = report.cleanup.context_closed;
	if (report.signed_features) report.signed_features.actual_context_closed = report.cleanup.context_closed;
	cleanupStage("closing-browser");
	if (browser) try { await browser.close(); report.cleanup.browser_closed = true; }
	catch (error) { cleanupFailure('browser', error); }
	cleanupStage("closing-proxy");
	try { await closeDenyProxy(); report.cleanup.proxy_closed = true; }
	catch (error) { cleanupFailure('proxy', error); }
	cleanupStage("finalizing-report");
	cleanupStage("complete");
	report.completed_utc = new Date().toISOString(); report.passed = !report.harness_error && !report.cleanup.failures.length && report.cleanup.context_closed && report.cleanup.browser_closed && report.cleanup.proxy_closed && report.steps.length > 0 && report.steps.every(entry => entry.status === "passed");
	try { save(); } catch { report.passed = false; report.harness_error ??= 'Final evidence persistence failed'; process.exitCode = 1; }
	process.stdout.write(JSON.stringify({ phase, passed: report.passed, steps: report.steps.length, failed_steps: report.steps.filter(entry => entry.status === "failed").map(entry => entry.name), page_errors: report.page_errors.length, network_guard: "Exact validated fixture-origin HTTP/WS routing plus owned external-deny proxy", evidence: path.relative(root, output) }) + "\n");
}
process.exitCode = report.passed ? 0 : 1;
