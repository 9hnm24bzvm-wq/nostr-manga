#!/usr/bin/env node
// Real Chromium acceptance against the disposable local platform fixture.
// Never record credentials, cookies, authentication query strings or request bodies.
import fs from "node:fs";
import path from "node:path";
import http from "node:http";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { runJourneys } from "./browser-journeys.mjs";
import { runSignedJourneys, signedJourneyPhases } from "./browser-signed-journeys.mjs";
import { runUi10Journeys, ui10JourneyPhases } from "./browser-ui10-journeys.mjs";
import { runUi11To16Journeys, ui11To16JourneyPhases } from "./browser-ui11-16-journeys.mjs";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// The supervised fixture deliberately strips NODE_PATH. Resolve only the
// already-installed official runtime package, never arbitrary inherited paths.
const playwrightRoot = "/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright";
const playwrightPins = { "package.json": "ca170ec143a88ed3043ac953eb3b2377b2b97304104f4e1e23316684ce2c35af", "index.js": "4e98f65f0a9d9bcab8cffc0c5cfdd87fd3f5b0be74fc494ff166a466b204cd44" };
for (const [file, expected] of Object.entries(playwrightPins)) {
  const absolute = path.join(playwrightRoot, file);
  if (fs.realpathSync(absolute) !== absolute || createHash("sha256").update(fs.readFileSync(absolute)).digest("hex") !== expected) throw Error("Reviewed installed Playwright source changed; inspect before browser launch");
}
const playwrightPackage = JSON.parse(fs.readFileSync(path.join(playwrightRoot, "package.json"), "utf8"));
if (playwrightPackage.name !== "playwright" || playwrightPackage.version !== "1.62.1" || playwrightPackage.repository?.url !== "git+https://github.com/microsoft/playwright.git") throw Error("Unrecognized installed browser package");
const require = createRequire(import.meta.url);
const { chromium } = require(path.join(playwrightRoot, "index.js"));
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
const report = { phase, origin, browser: "system Chromium via installed Playwright", source_artifact: "artifacts/frontend", started_utc: new Date().toISOString(), steps: [], pages: [], external_requests_blocked: [], proxy_requests_blocked: [], page_errors: [], console_errors: [], csp_violations: [], response_errors: [], screenshots: [], limitations: ["Supplied archive lacks an independently runnable original-render baseline. Source/DOM/style preservation and browser operation are distinct from pixel parity."] };
const safeUrl = value => { try { const url = new URL(value); return url.origin + url.pathname + (url.pathname.startsWith("/api/") && !/auth|client/.test(url.pathname) ? url.search : ""); } catch { return "invalid URL"; } };
const redactions = new Set();
const safeError = error => {
	let value = String(error?.message ?? error).replace(/https?:\/\/[^\s'"<>]+/g, safeUrl);
	for (const secret of redactions) value = value.split(secret).join("[redacted synthetic credential]");
	return value.length > 9000 ? value.slice(0, 2000) + "\n[diagnostic middle omitted]\n" + value.slice(-7000) : value;
};
function save() { fs.writeFileSync(path.join(output, "acceptance.json"), JSON.stringify(report, null, 2) + "\n"); }
function insist(condition, message) { if (!condition) throw new Error(message); }
async function step(name, action) {
	const entry = { name, started_utc: new Date().toISOString() };
	report.steps.push(entry); save();
	try { entry.observation = await action(); entry.status = "passed"; }
	catch (error) { entry.status = "failed"; entry.error = safeError(error); }
	entry.completed_utc = new Date().toISOString(); save();
	process.stdout.write(JSON.stringify({ phase, step: name, status: entry.status }) + "\n");
}
let browser;
const sockets = new Set();
const proxy = http.createServer((request, response) => {
	report.proxy_requests_blocked.push({ method: request.method, url: safeUrl(request.url ?? "") });
	response.writeHead(403, { "Content-Type": "text/plain" }); response.end("External browser traffic blocked by acceptance fixture");
});
proxy.on("connect", (request, socket) => { report.proxy_requests_blocked.push({ method: "CONNECT", destination: String(request.url).split("?")[0] }); socket.end("HTTP/1.1 403 Forbidden\r\n\r\n"); });
proxy.on("connection", socket => { sockets.add(socket); socket.on("close", () => sockets.delete(socket)); });
await new Promise(resolve => proxy.listen(0, "127.0.0.1", resolve));
const proxyAddress = proxy.address();
try {
	browser = await chromium.launch({ executablePath: "/usr/bin/chromium", headless: true,
		proxy: { server: `http://127.0.0.1:${proxyAddress.port}`, bypass: "127.0.0.1,localhost" },
		args: ["--disable-background-networking", "--disable-component-update", "--disable-sync", "--no-first-run", "--disable-default-apps", "--disable-features=MediaRouter"] });
	const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1, serviceWorkers: phase === "pwa" ? "allow" : "block" });
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
		page.on("pageerror", error => report.page_errors.push({ url: safeUrl(page.url()), error: safeError(error) }));
		page.on("console", message => { if (message.type() === "error") report.console_errors.push({ url: safeUrl(page.url()), error: safeError(message.text()) }); });
		page.on("response", response => { if (response.status() >= 400) report.response_errors.push({ status: response.status(), method: response.request().method(), url: safeUrl(response.url()) }); });
	});
	const page = await context.newPage();
	async function open(route) { const response = await page.goto(origin + route, { waitUntil: "domcontentloaded" }); await page.waitForTimeout(1200); insist(response?.status() === 200, `Navigation returned ${response?.status()}`); return response; }
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
			const before = report.page_errors.length; await open(route); const observation = await snapshot(); report.pages.push(observation);
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
	} else await (signedJourneyPhases.includes(phase) ? runSignedJourneys : ui10JourneyPhases.includes(phase) ? runUi10Journeys : ui11To16JourneyPhases.includes(phase) ? runUi11To16Journeys : runJourneys)({ phase, page, context, origin, fixture, step, insist, open, report, redactions });
	insist(report.page_errors.length === 0, "Application JavaScript page errors were observed during the journey");
	await context.close();
} catch (error) { report.harness_error = safeError(error); }
finally {
	if (browser) await browser.close();
	for (const socket of sockets) socket.destroy();
	await new Promise(resolve => proxy.close(resolve));
	report.completed_utc = new Date().toISOString(); report.passed = !report.harness_error && report.steps.length > 0 && report.steps.every(entry => entry.status === "passed"); save();
	process.stdout.write(JSON.stringify({ phase, passed: report.passed, steps: report.steps.length, failed_steps: report.steps.filter(entry => entry.status === "failed").map(entry => entry.name), page_errors: report.page_errors.length, network_guard: "Exact validated fixture-origin HTTP/WS routing plus owned external-deny proxy", evidence: path.relative(root, output) }) + "\n");
}
process.exitCode = report.passed ? 0 : 1;
