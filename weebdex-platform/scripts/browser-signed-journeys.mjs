// Source-only until the parent runtime audit gate is cleared.
// Every application mutation below uses a visible user control. IndexedDB reads
// are observations only. Fault routes abort or forward a genuine response;
// they never manufacture a successful original-module acknowledgment.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { selectTitleLibraryStatus, titleLibraryTrigger } from "./browser-select-controls.mjs";

export const signedJourneyPhases = ["signed-library", "signed-profile", "signed-social", "signed-context", "signed-source-controls", "signed-namespace", "signed-recipients", "signed-restart"];
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const digest = value => createHash("sha256").update(value).digest("hex");
const hex = value => Buffer.from(value).toString("hex");
const unhex = value => Uint8Array.from(Buffer.from(value, "hex"));
const identifier = value => typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
const eventDigest = event => digest(JSON.stringify(event));

// These provenance bytes and official archives were previously restored with
// lock-integrity checks and no package hooks. Revalidate every admitted file at
// the eventual runtime boundary; do not use upstream or uncertain node_modules.
async function verifiedSchnorr(insist) {
  const manifestPath = path.join(root, "test-evidence/frontend/noble-restore/package-provenance.json");
  const bytes = fs.readFileSync(manifestPath);
  insist(digest(bytes) === "f7cd7c2d6d7b14b9eaec4d38b4405267cc9ba291f0808dc3687822e28a3d24f3", "Synthetic signer provenance changed; obtain fresh source audit clearance");
  const manifest = JSON.parse(bytes), stage = path.join(root, ".build/local-library-noble-2.0.1");
  const archives = { "@noble/curves": "1271ac0dfa27c93e464e64b65ff2a255eea52c449ca86c149cfb0b9786dd9029", "@noble/hashes": "638ffb3053a7e7478c9e54a6e297f3601299ee570a41112e501af7050d086a0a" };
  insist(manifest.package_hooks_executed === false && manifest.original_node_modules_used === false && manifest.packages.length === 2, "Unreviewed synthetic signer dependency provenance");
  for (const item of manifest.packages) {
    insist(Object.hasOwn(archives, item.name) && item.version === "2.0.1" && item.archive_sha256 === archives[item.name], "Unexpected synthetic signer package");
    const short = item.name.split("/")[1], archive = path.join(stage, short + "-2.0.1.tgz"), packageRoot = path.join(stage, "node_modules", item.name);
    insist(fs.lstatSync(archive).isFile() && !fs.lstatSync(archive).isSymbolicLink() && digest(fs.readFileSync(archive)) === archives[item.name], "Synthetic signer archive hash mismatch");
    for (const entry of item.files) {
      const file = path.resolve(packageRoot, entry.path);
      insist(file.startsWith(packageRoot + path.sep) && fs.realpathSync(file) === file && fs.lstatSync(file).isFile(), "Synthetic signer source is outside its verified package");
      const content = fs.readFileSync(file);
      insist(content.length === entry.bytes && digest(content) === entry.sha256, "Synthetic signer installed source hash mismatch");
    }
  }
  return (await import(pathToFileURL(path.join(stage, "node_modules/@noble/curves/secp256k1.js")).href)).schnorr;
}

function exactLoopback(value, protocol, insist, label) {
  const url = new URL(value);
  insist(url.protocol === protocol && ["127.0.0.1", "localhost"].includes(url.hostname) && !!url.port && url.port !== "0" && !url.username && !url.password && url.pathname === "/" && !url.search && !url.hash, `Invalid exact loopback ${label}`);
  return url;
}

export async function runSignedJourneys(h) {
  const { phase, page, context, origin, fixture, step, insist, open, report } = h;
  insist(signedJourneyPhases.includes(phase), "Unknown signed browser journey phase");
  const protocol = exactLoopback(fixture.protocol_origin, "http:", insist, "original service").origin;
  const sink = exactLoopback(fixture.mail_origin ?? fixture.control_origin, "http:", insist, "fixture control").origin;
  const websocket = exactLoopback(fixture.relay_origin, "ws:", insist, "original relay");
  insist("http://" + websocket.host === protocol && identifier(fixture.service_key), "Original relay/service identity contract mismatch");
  const recipients = fixture.protocol_recipients ?? [{ origin: protocol, relay_origin: websocket.href, service_key: fixture.service_key }];
  insist(Array.isArray(recipients) && recipients.length >= 1 && recipients.length <= 2, "Bounded genuine fixture recipient list required");
  const proofOrigins = new Set(), serviceKeys = new Set();
  for (const recipient of recipients) {
    const http = exactLoopback(recipient.origin, "http:", insist, "recipient service"), ws = exactLoopback(recipient.relay_origin, "ws:", insist, "recipient relay");
    insist("http://" + ws.host === http.origin && identifier(recipient.service_key) && !proofOrigins.has(http.origin) && !serviceKeys.has(recipient.service_key), "Invalid or duplicate genuine fixture recipient"); proofOrigins.add(http.origin); serviceKeys.add(recipient.service_key);
  }
  insist(recipients[0].origin === protocol && recipients[0].service_key === fixture.service_key, "Primary genuine fixture recipient changed");
  const second = recipients[1];
  const seeded = fixture.protocol_seed;
  insist(seeded?.fixture_only === true && seeded.fully_ready === true && seeded.series?.canonical_kind === 1002 && seeded.chapters?.length >= 2, "Genuine original publication/projection/import fixture is not ready");
  for (const resource of [seeded.series, ...seeded.chapters]) insist(identifier(resource.event_id) && /^[A-Za-z0-9]{10}$/.test(resource.native_id ?? "") && resource.visible === true && resource.projected === true && resource.mapped === true, "Source seed lacks genuinely visible canonical/native mapping");
  const manga = seeded.series.native_id, chapter = seeded.chapters[0].native_id, nextChapter = seeded.chapters[1].native_id;
  const nativeOnly = fixture.ids?.manga;
  insist(/^[A-Za-z0-9]{10}$/.test(nativeOnly ?? "") && nativeOnly !== manga, "Separate native-only fixture title is required");
  const marker = randomBytes(6).toString("hex"), schnorr = await verifiedSchnorr(insist);
  let secret = schnorr.utils.randomSecretKey(); // Node memory only: never passed to Chromium, fixture control or evidence.
  const owner = hex(schnorr.getPublicKey(secret));
  let deniedConnect = false, denyKind = null, alteredKind = null, heldKind = null, heldResolve, heldSeen = false, deviceList;
  const signedCounts = new Map();
  const mutationRequests = [];
  const verify = event => {
    try {
      if (!identifier(event?.pubkey) || !identifier(event.id) || !/^[0-9a-f]{128}$/.test(event.sig) || !Number.isSafeInteger(event.kind) || !Number.isSafeInteger(event.created_at) || typeof event.content !== "string" || !Array.isArray(event.tags) || event.tags.some(tag => !Array.isArray(tag) || tag.some(value => typeof value !== "string"))) return false;
      const id = digest(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]));
      return id === event.id && schnorr.verify(unhex(event.sig), unhex(id), unhex(event.pubkey));
    } catch { return false; }
  };
  const originalAt = async (endpoint, route) => {
    insist(proofOrigins.has(endpoint), "Original observation targeted an unapproved service");
    insist(route.startsWith("/") && !route.startsWith("//") && !route.includes("://"), "Original observation escaped exact fixture origin");
    const response = await context.request.get(endpoint + route, { headers: { Origin: origin }, maxRedirects: 0 });
    const bytes = await response.body(); insist(bytes.length <= 1024 * 1024, "Original observation response exceeded budget");
    return { status: response.status(), body: /json/.test(response.headers()["content-type"] ?? "") ? JSON.parse(bytes.toString("utf8")) : undefined };
  };
  const original = route => originalAt(protocol, route);
  const native = async (route, data) => {
    insist(route.startsWith("/api/") && !route.includes("://"), "Native observation escaped fixture API");
    const response = data ? await context.request.post(origin + route, { data, headers: { Origin: origin }, maxRedirects: 0 }) : await context.request.get(origin + route, { headers: { Origin: origin }, maxRedirects: 0 });
    return { status: response.status(), body: /json/.test(response.headers()["content-type"] ?? "") ? await response.json() : undefined };
  };
  const admit = async (allowed, recipient = protocol) => {
    insist(proofOrigins.has(recipient) && (recipient === protocol || second?.origin === recipient), "Account admission target is outside the recorded fixture");
    const response = await context.request.post(sink + (recipient === protocol ? "/fixture/protocol/account" : "/fixture/protocol/recipient-account"), { data: { pubkey: owner, allowed }, maxRedirects: 0 });
    insist(response.status() === 200 && (await response.json()).fixture_only === true, "Real original synthetic public-key account admission control failed");
  };
  const until = async (read, valid, message, timeout = 30000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) { const value = await read(); if (valid(value)) return value; await page.waitForTimeout(150); }
    throw Error(message);
  };
  // These commands are private fixture-owner seams, never HTTP production APIs.
  // They carry only public fixture identities and explicit lifecycle intent.
  const privateControl = async (action, extra = {}) => {
    const control = fixture.fixture_control, directory = path.resolve(fixture.run_directory ?? "");
    insist(control?.fixture_only === true && control.allowed_actions?.includes(action) && path.dirname(directory) === path.join(root, "test-runtime") && /^browser-final-[A-Za-z0-9_-]+$/.test(path.basename(directory)) && fs.realpathSync(directory) === directory && (fs.statSync(directory).mode & 0o077) === 0, "Private fixture control root/action is unverified");
    const source = path.join(directory, "supervisor.json"), command = path.join(directory, "fixture-control-command.json"), results = path.join(directory, "control-results");
    insist(control.command_file === command && control.results_dir === results && path.resolve(process.env.BROWSER_FIXTURE_FILE ?? "") === source, "Private fixture control paths differ from the owned supervisor");
    const readPrivate = (file, max = 1024 * 1024) => { insist(fs.realpathSync(file) === file && fs.lstatSync(file).isFile() && fs.statSync(file).size <= max, "Unbounded or linked private fixture receipt"); return JSON.parse(fs.readFileSync(file, "utf8")); };
    const markerFile = readPrivate(path.join(directory, "fixture-owned.json"), 4096); insist(markerFile.fixture_only === true && markerFile.database === "weebdex_browser_test", "Disposable fixture ownership marker mismatch");
    const before = readPrivate(source), owned = Object.entries(before.children ?? {}).filter(([name, child]) => name.startsWith("browser-") && child.pid === process.pid);
    insist(owned.length === 1, "The current runner is not a recorded owned fixture browser process");
    const browserPhase = owned[0][0].slice("browser-".length), requestId = "signed-" + marker + "-" + randomBytes(6).toString("hex");
    insist(/^[A-Za-z0-9_-]{1,48}$/.test(browserPhase) && before.platform_origin === origin && before.protocol_origin === protocol && before.service_key === fixture.service_key, "Owned phase or original fixture identity changed");
    const restarting = ["source-restore", "catalogue-rebuild", "runtime-restart"].includes(action);
    insist(!restarting || before.deadline_unix * 1000 - Date.now() >= 430000, "Remaining fixture lifetime cannot contain the explicit listener restart");
    const payload = { request_id: requestId, action, browser_phase: browserPhase, ...extra, ...(restarting ? { allow_listener_restart: true } : {}) }, bytes = Buffer.from(JSON.stringify(payload));
    insist(bytes.length <= 4096 && !fs.existsSync(command), "Private fixture command queue is occupied or oversized");
    const temporary = command + "." + requestId + ".new";
    fs.writeFileSync(temporary, bytes, { flag: "wx", mode: 0o600 }); fs.renameSync(temporary, command);
    const file = path.join(results, requestId + ".json"), deadline = Date.now() + (restarting ? 415000 : 100000);
    while (!fs.existsSync(file) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 150));
    insist(fs.existsSync(file), "Owned fixture control did not produce a terminal receipt before its finite deadline");
    const result = readPrivate(file); insist(result.fixture_only === true && result.request_id === requestId && result.action === action && result.success === true && !result.cleanup_required, "Genuine fixture control failed; acceptance cannot be inferred");
    let current = readPrivate(source); const receiptDeadline = Date.now() + 5000;
    while ((!['ready', 'ready_with_intentionally_revoked_fixture_source'].includes(current.status) || current.protocol_seed?.catalog_namespace !== result.protocol_seed?.catalog_namespace) && Date.now() < receiptDeadline) { await new Promise(resolve => setTimeout(resolve, 100)); current = readPrivate(source); }
    insist(['ready', 'ready_with_intentionally_revoked_fixture_source'].includes(current.status) && current.protocol_seed?.catalog_namespace === result.protocol_seed?.catalog_namespace, "Terminal fixture control receipt has no matching current supervisor readiness");
    insist(current.platform_origin === origin && current.protocol_origin === protocol && current.service_key === fixture.service_key && result.service_key === fixture.service_key, "Genuine control changed the approved service identity or origin");
    if (restarting) {
      const joined = Object.values(result.listeners_stopped ?? {}); insist(joined.length >= 1 && joined.every(row => row.returncode === 0 && row.verified_stopped === true && row.forced === false && row.remaining_group_members?.length === 0), "Listener restart lacked genuine terminal owned-group drain");
      insist(current.children.platform?.pid !== before.children.platform?.pid && current.frontend?.index_sha256 === before.frontend?.index_sha256, "Real restart did not establish a new process with the same immutable frontend");
    }
    return { result, current };
  };
  const library = () => page.locator("[data-local-library-panel]").first();
  const identity = () => page.locator("[data-protocol-identity-panel]").first();
  const expand = async target => { await target.waitFor({ state: "visible" }); if (await target.getAttribute("open") === null) await target.locator(":scope > summary").click(); };
  const section = async (target, name) => { const summary = target.locator("summary").filter({ hasText: new RegExp("^" + name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "$") }); const detail = summary.locator(".."); if (await detail.getAttribute("open") === null) await summary.click(); };
  const idle = async target => until(() => target.innerText(), value => !value.includes("Working:") && !value.includes("Waiting for the selected signer or instance"), "Actual UI operation did not settle", 45000);
  const consentDialog = async (button, expected) => {
    const dialog = page.waitForEvent("dialog"); const click = button.click(); const prompt = await dialog;
    insist(prompt.type() === "confirm" && prompt.message().includes(expected), "Unexpected browser confirmation"); await prompt.accept(); await click;
  };
  const follow = status => selectTitleLibraryStatus(page, status);
  const databases = async () => page.evaluate(async () => {
    const available = await indexedDB.databases();
    async function read(name, stores, keys) {
      if (!available.some(item => item.name === name)) return undefined;
      const db = await new Promise((resolve, reject) => { const request = indexedDB.open(name); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error); });
      try { return await new Promise((resolve, reject) => { const tx = db.transaction(stores, "readonly"), output = {}; for (const store of stores) { const query = tx.objectStore(store).getAll(); query.onsuccess = () => { output[store] = query.result; }; } for (const key of keys) { const query = tx.objectStore("meta").get(key); query.onsuccess = () => { output[key] = query.result; }; } tx.oncomplete = () => resolve(output); tx.onerror = () => reject(tx.error); }); } finally { db.close(); }
    }
    return { device: await read("weebdex-device-library-v1:" + encodeURIComponent(location.origin), ["records", "meta"], ["instanceScope", "revision", "scopeMigrationReason"]), protocol: await read("weebdex-protocol-library-v1", ["records", "meta", "jobs"], ["relays", "relayRevision", "revision", "weebdexSignedPublicActions-v1"]) };
  });
  const jobs = async () => (await databases()).protocol?.jobs ?? [];
  const actions = async () => (await databases()).protocol?.["weebdexSignedPublicActions-v1"] ?? [];
  const jobCard = id => library().locator("div.rounded.border.p-2").filter({ has: page.locator("p").filter({ hasText: id + " · device revision" }) }).last();
  const actionCard = id => identity().locator("div.rounded.border.p-2.space-y-1").filter({ has: page.locator("p").filter({ hasText: id + " · kind" }) });
  const counterpartAt = async (endpoint, id) => until(() => originalAt(endpoint, "/events/" + id), value => value.status === 200 && value.body?.id === id && verify(value.body), "Actual backend did not project the exact verified event", 60000);
  const counterpart = id => counterpartAt(protocol, id);
  const downloadBytes = async button => { const pending = page.waitForEvent("download"); await button.click(); const item = await pending; const stream = await item.createReadStream(); insist(stream, "UI download bytes unavailable"); const chunks = []; for await (const chunk of stream) chunks.push(chunk); return Buffer.concat(chunks); };
  const observeMutation = request => { const url = new URL(request.url()); if (proofOrigins.has(url.origin) && request.method() === "POST") mutationRequests.push({ origin: url.origin, path: url.pathname, bodyDigest: digest(request.postDataBuffer() ?? Buffer.alloc(0)) }); };
  page.on("request", observeMutation);
  const binding = "__fixtureNip07_" + marker;
  const signerCall = async (source, operation, input) => {
    insist(source.page === page && new URL(source.frame.url()).origin === origin, "Synthetic signer called outside its explicitly selected fixture page");
    if (operation === "getPublicKey") { if (deniedConnect) throw Error("Synthetic signer selection denied"); return owner; }
    insist(operation === "signEvent" && input && typeof input === "object" && input.pubkey === owner && [0, 3, 5, 7, 1111, 10002, 11999, 27235].includes(input.kind) && Buffer.byteLength(JSON.stringify(input)) <= 16384, "Synthetic signer rejected unapproved event template");
    if (input.kind === 27235) {
      const urls = input.tags.filter(tag => tag[0] === "u"), method = input.tags.filter(tag => tag[0] === "method");
      insist(urls.length === 1 && method.length === 1 && ["GET", "POST"].includes(method[0][1]) && proofOrigins.has(new URL(urls[0][1]).origin), "Authentication proof recipient escaped exact original fixture");
    }
    if (denyKind === input.kind) { denyKind = null; throw Error("Synthetic signer denied event kind " + input.kind); }
    if (heldKind === input.kind) { heldSeen = true; await new Promise(resolve => { heldResolve = resolve; }); heldKind = null; heldResolve = undefined; }
    insist(secret !== null, "The ephemeral synthetic signer has been retired");
    const template = structuredClone(input);
    if (alteredKind === input.kind) { alteredKind = null; template.content += " synthetic altered result"; }
    const id = digest(JSON.stringify([0, owner, template.created_at, template.kind, template.tags, template.content]));
    const event = { ...template, id, sig: hex(schnorr.sign(unhex(id), secret)) };
    insist(verify(event), "Synthetic NIP07 genuine Schnorr signature verification failed");
    signedCounts.set(event.kind, (signedCounts.get(event.kind) ?? 0) + 1);
    return event;
  };
  const installSigner = async () => { await context.exposeBinding(binding, signerCall); await context.addInitScript(name => { window.nostr = { getPublicKey: () => window[name]("getPublicKey"), signEvent: template => window[name]("signEvent", template) }; }, binding); await page.evaluate(name => { window.nostr = { getPublicKey: () => window[name]("getPublicKey"), signEvent: template => window[name]("signEvent", template) }; }, binding); };
  const selectSigner = async () => { await library().getByRole("button", { name: "Connect external browser signer", exact: true }).click(); await idle(library()); await library().getByText("Signer connected. No library data sent.", { exact: true }).waitFor({ state: "visible" }); };
  const reviewSnapshot = async () => { await library().getByRole("button", { name: "Preview public snapshot", exact: true }).click(); await library().getByText(/^Reviewed device revision/).waitFor({ state: "visible", timeout: 45000 }); const send = library().getByRole("button", { name: "Sign and publish reviewed snapshot", exact: true }); insist(await send.isDisabled(), "Snapshot sending enabled before explicit public consent"); await library().getByLabel("I understand that this reviewed snapshot becomes public at these recipients.", { exact: true }).check(); return send; };
  const prepareIdentity = async () => { await expand(identity()); await identity().getByRole("button", { name: "Refresh verified instances", exact: true }).click(); await idle(identity()); await identity().getByLabel("Verified recipient", { exact: true }).selectOption(protocol); };
  const target = async (kind, id) => { await section(identity(), "Public source comments and reactions"); await identity().getByLabel("Target type", { exact: true }).selectOption(kind); await identity().getByLabel("Current instance catalogue ID", { exact: true }).fill(id); };
  const loadConversation = async (kind, id, eventId) => { await target(kind, id); await identity().getByRole("button", { name: "Load verified public conversation", exact: true }).click(); await identity().getByText("Verified canonical source: " + eventId, { exact: false }).waitFor({ state: "visible", timeout: 45000 }); };
  const sendAction = async () => { const button = identity().getByRole("button", { name: "Sign and send reviewed public action", exact: true }); insist(await button.isDisabled(), "Public action enabled before exact-action consent"); await identity().getByLabel("I approve this exact public action and recipient.", { exact: true }).check(); await button.click(); await idle(identity()); };
  const reviewComment = async text => { await identity().getByLabel("Public comment", { exact: true }).fill(text); await identity().getByRole("button", { name: "Review public comment", exact: true }).click(); await identity().getByText("Publish this public source comment. Recipient: " + protocol, { exact: false }).waitFor({ state: "visible" }); };
  const changedSourceNotice = "The selected target, recipient or signing identity changed.";
  const lifecycleCoverage = { source_revocation_restoration: false, real_namespace_rebuild: false, joined_runtime_restart: false, two_real_recipients: false };
  const prepareLifecycleLibrary = async () => {
    await admit(true); await (await reviewSnapshot()).click(); await idle(library());
    const job = (await jobs()).at(-1); insist(job?.recipients[0].state === "accepted" && verify(job.event), "Lifecycle prerequisite has no genuine accepted library snapshot"); await counterpart(job.id);
    await library().getByRole("button", { name: "Preview verified public recovery", exact: true }).click(); await library().getByText("Verified public recovery: " + protocol, { exact: false }).waitFor({ state: "visible", timeout: 60000 });
    await consentDialog(library().getByRole("button", { name: "Restore reviewed canonical records", exact: true }), "verified recovery"); await idle(library());
    insist((await databases()).protocol.records.length >= 4, "Lifecycle prerequisite did not restore real canonical library families"); return job;
  };

  try {
    await step("genuine original module fixture and current canonical aliases are browser-visible", async () => {
      const policy = await native("/api/protocol/capabilities");
      insist(policy.status === 200 && policy.body.available === true && policy.body.schema_ready === true && policy.body.origin === protocol && policy.body.service_key === fixture.service_key && policy.body.catalog_namespace === seeded.catalog_namespace && policy.body.external_signer_required === true && policy.body.account_admission_required === true && policy.body.native_account_is_signer === false && policy.body.connect_origins.includes(protocol), "Actual protocol capabilities differ from reviewed fixture contract");
      const mappings = await native("/api/protocol/resources", { resources: [{ kind: "manga", id: manga }, { kind: "chapter", id: chapter }, { kind: "chapter", id: nextChapter }, { kind: "manga", id: nativeOnly }] });
      insist(mappings.status === 200 && mappings.body.catalog_namespace === seeded.catalog_namespace && mappings.body.data.length === 3 && !mappings.body.data.some(row => row.id === nativeOnly), "Native/canonical separation or real source mapping failed");
      for (const resource of [seeded.series, ...seeded.chapters.slice(0, 2)]) { const event = await counterpart(resource.event_id); insist(event.body.kind === resource.canonical_kind, "Canonical source kind mismatch"); }
      await open("/library");
      const relayEvent = await page.evaluate(({ url, id, subscription }) => new Promise((resolve, reject) => {
        const socket = new WebSocket(url); let settled = false;
        const finish = (error, event) => { if (settled) return; settled = true; clearTimeout(timer); if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(["CLOSE", subscription])); socket.close(); if (error) reject(Error(error)); else resolve(event); };
        const timer = setTimeout(() => finish("Genuine original root WebSocket did not return the seeded event"), 15000);
        socket.onopen = () => socket.send(JSON.stringify(["REQ", subscription, { ids: [id], limit: 1 }]));
        socket.onerror = () => finish("Genuine exact-origin original root WebSocket failed");
        socket.onmessage = message => { let value; try { value = JSON.parse(message.data); } catch { return finish("Invalid original relay frame"); } if (value[0] === "EVENT" && value[1] === subscription && value[2]?.id === id) finish(undefined, value[2]); else if ((value[0] === "EOSE" || value[0] === "CLOSED") && value[1] === subscription) finish("Original relay did not expose the genuinely seeded source"); };
      }), { url: websocket.href, id: seeded.series.event_id, subscription: "fixture-" + marker });
      insist(verify(relayEvent) && relayEvent.id === seeded.series.event_id && relayEvent.kind === 1002, "Root WebSocket returned a different or unverified source event");
      return { real_original_seed_ready: true, canonical_namespace: seeded.catalog_namespace, real_mappings: mappings.body.data.length, native_only_excluded: true, exact_original_origin: protocol, genuine_root_websocket_read: true };
    });
    await step("anonymous device controls persist mapped and native-only records without signer or publication", async () => {
      await open(`/title/${manga}`); await follow("Reading");
      await until(databases, state => state.device?.records.some(row => row.family === "series" && row.id === manga && row.status === "reading"), "Visible follow did not persist mapped device series");
      await open(`/title/${nativeOnly}`); await follow("Plan to Read");
      await open(`/chapter/${chapter}/1`); await until(() => page.locator('img[src^="blob:"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0).length), count => count > 0, "Canonical chapter reader did not decode real local bytes");
      await until(databases, state => state.device?.records.some(row => row.family === "chapters" && row.id === chapter && row.page >= 1), "Canonical reader progress did not persist locally");
      await open("/library"); await expand(library()); await section(library(), "Optional Nostr publication and verified recovery");
      await section(library(), "Custom device lists"); await library().getByLabel("List name", { exact: true }).fill("Synthetic signed list " + marker); await library().getByRole("button", { name: "Create list", exact: true }).click(); await idle(library());
      deviceList = (await databases()).device.records.find(row => row.family === "lists" && row.name === "Synthetic signed list " + marker); insist(deviceList && /^[0-9a-f]{32}$/.test(deviceList.id), "Visible custom list control did not persist its original 128-bit ID");
      await library().getByLabel("List", { exact: true }).selectOption(deviceList.id); await library().getByLabel("Series", { exact: true }).selectOption(manga); await library().getByLabel("Position", { exact: true }).fill("2"); await library().getByRole("button", { name: "Save member", exact: true }).click(); await idle(library());
      await until(databases, state => state.device.records.some(row => row.family === "members" && row.list === deviceList.id && row.series === manga && row.position === 2), "Visible list membership control did not persist scoped native reference");
      const state = await databases(); insist(state.device.instanceScope.catalogNamespace === seeded.catalog_namespace && state.device.instanceScope.serviceKey === fixture.service_key && state.protocol.jobs.length === 0 && mutationRequests.length === 0, "Anonymous local controls published or lost their namespace");
      insist((await native("/api/auth/check")).status !== 200 && await page.evaluate(() => typeof window.nostr === "undefined"), "A password session or signer was silently created");
      await library().getByRole("button", { name: "Connect external browser signer", exact: true }).click(); await idle(library()); await library().getByText("A compatible NIP-07 browser signer is required", { exact: false }).waitFor({ state: "visible" });
      return { anonymous_scope_preserved: true, actual_reader_bytes_decoded: true, original_list_id: deviceList.id, scoped_list_membership: true, no_auto_signer: true, original_mutation_requests: 0, outbox_jobs: 0 };
    });
    await step("external signer and actual service recipient require explicit independent selection", async () => {
      await installSigner(); deniedConnect = true; await library().getByRole("button", { name: "Connect external browser signer", exact: true }).click(); await idle(library()); await library().getByText("Synthetic signer selection denied", { exact: false }).waitFor({ state: "visible" }); insist((await jobs()).length === 0, "Denied signer selection queued a snapshot");
      deniedConnect = false; await selectSigner();
      await library().getByLabel("Instance origin", { exact: true }).fill(protocol); await library().getByLabel("Expected service public key", { exact: true }).fill("0".repeat(64));
      await consentDialog(library().getByRole("button", { name: "Verify and remember instance", exact: true }), "Verify and remember"); await idle(library()); await library().getByText("Service identity mismatch", { exact: false }).waitFor({ state: "visible" });
      insist(((await databases()).protocol.relays ?? []).length === 0, "Wrong service proof was remembered");
      await library().getByLabel("Instance origin", { exact: true }).fill("http://127.0.0.1:1");
      await consentDialog(library().getByRole("button", { name: "Verify and remember instance", exact: true }), "Verify and remember"); await idle(library()); await library().getByText("This origin is not allowed by the instance relay/CSP policy", { exact: false }).waitFor({ state: "visible" });
      await library().getByLabel("Instance origin", { exact: true }).fill(protocol); await library().getByLabel("Expected service public key", { exact: true }).fill(fixture.service_key);
      await consentDialog(library().getByRole("button", { name: "Verify and remember instance", exact: true }), "Verify and remember"); await idle(library());
      const remembered = (await databases()).protocol.relays; insist(remembered.length === 1 && remembered[0].origin === protocol && remembered[0].serviceKey === fixture.service_key && remembered[0].selected === false, "Verified instance was automatically selected");
      await library().getByLabel("Select " + protocol, { exact: true }).check(); await idle(library());
      return { explicit_memory_only_nip07: true, synthetic_public_key: owner, wrong_service_key_rejected: true, unapproved_recipient_rejected: true, actual_identity_proof_verified: true, initial_recipient_unselected: true };
    });

    if (phase === "signed-source-controls") {
      let revoked, before;
      await step("genuine source moderation removes current mapping and fences both stale signed reviews", async () => {
        await admit(true); await prepareIdentity(); await loadConversation("chapter", chapter, seeded.chapters[0].event_id); await reviewComment("Synthetic revoked-source review " + marker); const send = await reviewSnapshot(); before = await databases();
        const snapshots = signedCounts.get(11999) ?? 0, comments = signedCounts.get(1111) ?? 0;
        revoked = await privateControl("source-revoke", { event_id: seeded.chapters[0].event_id });
        insist(revoked.result.operation.operator_approval === true && revoked.current.protocol_seed.fully_ready === false && (await original("/events/" + seeded.chapters[0].event_id)).status === 404, "Actual approved source removal was not terminal and publicly hidden");
        const mapping = await native("/api/protocol/resources", { resources: [{ kind: "chapter", id: chapter }] }); insist(mapping.status === 200 && mapping.body.data.length === 0, "Revoked original source retained a current native mapping");
        await send.click(); await idle(library()); await library().getByText("Library, canonical mapping or recipient selection changed. Review again.", { exact: true }).waitFor({ state: "visible" });
        await sendAction(); await identity().getByText("This resource has no visible verified canonical source", { exact: false }).waitFor({ state: "visible" });
        const after = await databases(); insist(eventDigest(after.device.records) === eventDigest(before.device.records) && (await jobs()).length === 0 && (await actions()).length === 0 && (signedCounts.get(11999) ?? 0) === snapshots && (signedCounts.get(1111) ?? 0) === comments, "Revoked source review signed, sent or rebound local state");
        return { genuine_operator_approval: true, source_id: seeded.chapters[0].event_id, original_public_404: true, real_native_mapping_absent: true, stale_library_and_social_reviews_fenced: true, local_records_preserved: true };
      });
      await step("exact source restoration joins listeners and reprojects identical recorded publication", async () => {
        insist(revoked?.result.success === true && before, "Genuine revocation prerequisite did not complete");
        const restored = await privateControl("source-restore", { event_id: seeded.chapters[0].event_id }); insist(restored.current.protocol_seed.fully_ready === true && restored.current.protocol_seed.catalog_namespace === seeded.catalog_namespace, "Exact source restore failed readiness or changed native namespace");
        const source = restored.current.protocol_seed.chapters.find(row => row.event_id === seeded.chapters[0].event_id); insist(source?.native_id === chapter && source.visible && source.projected && source.mapped, "Source restore changed exact identity or native alias"); await counterpart(source.event_id);
        const mapping = await native("/api/protocol/resources", { resources: [{ kind: "chapter", id: chapter }] }); insist(mapping.body.data.some(row => row.event_id === source.event_id && row.id === chapter) && eventDigest((await databases()).device.records) === eventDigest(before.device.records), "Restored original did not regain its exact scoped native interpretation");
        await page.reload({ waitUntil: "domcontentloaded" }); await expand(library()); await section(library(), "Optional Nostr publication and verified recovery"); await selectSigner(); await prepareIdentity(); await loadConversation("chapter", chapter, source.event_id); lifecycleCoverage.source_revocation_restoration = true;
        return { source_id: source.event_id, exact_original_event_restored: true, same_catalogue_namespace: true, real_projection_and_mapping_restored: true, joined_listener_restart: true, unchanged_local_records: true };
      });
    } else if (phase === "signed-namespace") {
      let before, rebuilt, job;
      await step("genuine platform migration rebuild changes namespace while preserving original publication and old device scope", async () => {
        job = await prepareLifecycleLibrary(); before = await databases(); const backup = JSON.parse((await downloadBytes(library().getByRole("button", { name: "Export device backup", exact: true }))).toString("utf8")); const send = await reviewSnapshot(), count = signedCounts.get(11999);
        rebuilt = await privateControl("catalogue-rebuild"); const operation = rebuilt.result.operation;
        insist(operation.migration_generated_namespace === true && operation.namespace_uuid_updated_by_fixture === false && operation.original_schema_preserved === true && operation.old_catalog_namespace === seeded.catalog_namespace && operation.catalog_namespace !== seeded.catalog_namespace && rebuilt.current.protocol_seed.catalog_namespace === operation.catalog_namespace, "Real migration-generated namespace rebuild did not meet its contract");
        const policy = await native("/api/protocol/capabilities"); insist(policy.status === 200 && policy.body.catalog_namespace === operation.catalog_namespace && policy.body.service_key === fixture.service_key, "Actual capabilities did not expose the new genuine namespace"); await counterpart(job.id);
        await send.click(); await idle(library()); await library().getByText("This device catalogue namespace is unadopted or changed", { exact: false }).waitFor({ state: "visible" }); insist(signedCounts.get(11999) === count && (await jobs()).length === 1, "Old-scope reviewed snapshot was signed into the rebuilt catalogue");
        await library().getByRole("button", { name: "Verify current catalogue scope", exact: true }).click(); await idle(library()); await library().getByText("Scope verification: migration_required.", { exact: false }).waitFor({ state: "visible" });
        const after = await databases(); insist(after.device.instanceScope.catalogNamespace === backup.scope.catalogNamespace && eventDigest(after.device.records) === eventDigest(before.device.records) && eventDigest(after.protocol.records) === eventDigest(before.protocol.records) && after.device.scopeMigrationReason, "Namespace rebuild silently rebound or discarded saved native/canonical data"); lifecycleCoverage.real_namespace_rebuild = true;
        return { old_namespace: seeded.catalog_namespace, migration_generated_new_namespace: operation.catalog_namespace, original_event_id: job.id, original_service_identity_preserved: true, stale_publication_fenced: true, old_scoped_records_and_backup_preserved: true, independent_canonical_rows_preserved: true };
      });
      await step("ordinary reload keeps genuine migration fence and refuses current title identity adoption", async () => {
        insist(rebuilt?.result.success && before, "Genuine namespace rebuild prerequisite failed"); await page.reload({ waitUntil: "domcontentloaded" }); await expand(library()); await library().getByText("Scope verification: migration_required.", { exact: false }).waitFor({ state: "visible" });
        const backup = JSON.parse((await downloadBytes(library().getByRole("button", { name: "Export device backup", exact: true }))).toString("utf8")); insist(backup.scope.catalogNamespace === seeded.catalog_namespace && eventDigest(backup.records) === eventDigest(before.device.records), "Reload/export lost the previous saved-device namespace");
        await open("/title/" + rebuilt.current.protocol_seed.series.native_id); await expand(library()); await library().getByText("Scope verification: migration_required.", { exact: false }).waitFor({ state: "visible" }); insist(await titleLibraryTrigger(page).count() === 0, "Current catalogue title offered native save controls under a mismatched old scope");
        const state = await databases(); insist(eventDigest(state.device.records) === eventDigest(before.device.records) && state.device.instanceScope.catalogNamespace === seeded.catalog_namespace, "Current title navigation rebound old IDs after real rebuild");
        return { reload_kept_migration_required: true, old_backup_scope_retained: true, new_catalogue_native_controls_fenced: true, no_implicit_scope_adoption: true };
      });
    } else if (phase === "signed-restart") {
      await step("joined real runtime restart retains exact outbox canonical recovery and device data", async () => {
        const job = await prepareLifecycleLibrary(), before = await databases(); const restarted = await privateControl("runtime-restart"); insist(restarted.current.protocol_seed.fully_ready && restarted.current.protocol_seed.catalog_namespace === seeded.catalog_namespace, "Joined restart changed namespace or lost genuine source readiness");
        const visible = await counterpart(job.id); insist(eventDigest(visible.body) === eventDigest(job.event), "Restarted original event differs from saved exact publication"); await page.reload({ waitUntil: "domcontentloaded" }); await expand(library()); await section(library(), "Optional Nostr publication and verified recovery"); await selectSigner();
        const after = await databases(); insist(eventDigest(after.device.records) === eventDigest(before.device.records) && eventDigest(after.protocol.records) === eventDigest(before.protocol.records) && eventDigest(after.protocol.jobs) === eventDigest(before.protocol.jobs), "Real runtime restart/reload lost or rebound local state/outbox");
        await library().getByRole("button", { name: "Preview verified public recovery", exact: true }).click(); await library().getByText("Verified public recovery: " + protocol, { exact: false }).filter({ hasText: job.id }).waitFor({ state: "visible", timeout: 60000 }); await library().getByRole("button", { name: "Cancel recovery", exact: true }).click(); lifecycleCoverage.joined_runtime_restart = true;
        return { exact_event_id: job.id, genuine_joined_old_processes: true, real_new_process_generation: true, same_service_origin_identity_namespace_and_ui_hash: true, native_canonical_outbox_survived: true, actual_recovery_bytes_verified_after_restart: true };
      });
    } else if (phase === "signed-recipients") {
      insist(second && second.service_key !== fixture.service_key, "Two actual distinct fixture services are required; a blocked/missing recipient is not passing coverage");
      await step("two independently verified real recipients accept the identical reviewed library event", async () => {
        await admit(true); await admit(true, second.origin); await library().getByLabel("Instance origin", { exact: true }).fill(second.origin); await library().getByLabel("Expected service public key", { exact: true }).fill(second.service_key); await consentDialog(library().getByRole("button", { name: "Verify and remember instance", exact: true }), "Verify and remember"); await idle(library());
        const remembered = (await databases()).protocol.relays.find(row => row.origin === second.origin); insist(remembered?.serviceKey === second.service_key && remembered.selected === false, "Second real service proof was not independently pinned/unselected"); await library().getByLabel("Select " + second.origin, { exact: true }).check(); await idle(library());
        await (await reviewSnapshot()).click(); await idle(library()); const job = (await jobs()).at(-1); insist(job.recipients.length === 2 && job.recipients.every(row => row.state === "accepted") && new Set(job.recipients.map(row => row.origin)).size === 2, "One genuine recipient failed or a fake second receipt was used");
        for (const recipient of recipients) { const event = await counterpartAt(recipient.origin, job.id); insist(eventDigest(event.body) === eventDigest(job.event), "Actual recipient projected different signed library bytes"); }
        lifecycleCoverage.two_real_recipients = true; return { event_id: job.id, genuine_service_origins: recipients.map(row => row.origin), actual_independent_service_proofs: true, real_library_receipts_and_projections: 2, identical_signed_event: true };
      });
      await step("real recipient selection changes discard review and exact-body retry retains original approved service", async () => {
        await prepareIdentity(); await section(identity(), "Public profile and follow/relay lists"); await identity().getByLabel("Public username", { exact: true }).fill("SyntheticRecipient" + marker); await identity().getByRole("button", { name: "Review public profile", exact: true }).click(); await identity().getByLabel("Verified recipient", { exact: true }).selectOption(second.origin);
        await until(() => identity().getByRole("button", { name: "Sign and send reviewed public action", exact: true }).count(), count => count === 0, "Changing to another genuine recipient retained old review/consent"); insist((signedCounts.get(0) ?? 0) === 0, "Recipient change signed an unapproved profile");
        await identity().getByLabel("Verified recipient", { exact: true }).selectOption(protocol); await identity().getByRole("button", { name: "Review public profile", exact: true }).click(); let interrupted = 0; const intercept = async route => { if (route.request().method() !== "POST") return route.continue(); interrupted++; await route.abort("failed"); };
        await context.route(protocol + "/account-metadata", intercept); try { await sendAction(); } finally { await context.unroute(protocol + "/account-metadata", intercept); }
        const saved = (await actions()).filter(row => row.event.kind === 0).at(-1); insist(interrupted === 1 && saved?.state === "outcome_unknown" && saved.relay.origin === protocol && saved.relay.serviceKey === fixture.service_key && (await original("/events/" + saved.event.id)).status === 404, "Failed original-recipient action was lost or rebound");
        const exact = eventDigest(saved.event), signed = signedCounts.get(0); await identity().getByLabel("Verified recipient", { exact: true }).selectOption(second.origin); await section(identity(), "Saved exact signed public-action outbox"); await consentDialog(actionCard(saved.event.id).getByRole("button", { name: "Retry exact reviewed signed body", exact: true }), saved.event.id); await idle(identity()); await counterpart(saved.event.id);
        const done = (await actions()).find(row => row.event.id === saved.event.id), posts = mutationRequests.filter(row => row.path === "/account-metadata" && row.bodyDigest === exact); insist(done.state === "accepted" && eventDigest(done.event) === exact && done.relay.origin === protocol && done.relay.serviceKey === fixture.service_key && signedCounts.get(0) === signed && posts.length === 2 && posts.every(row => row.origin === protocol), "Exact-body retry switched to the newly selected genuine service");
        return { original_recipient: protocol, newly_selected_recipient: second.origin, exact_event_id: saved.event.id, exact_body_digest: exact, original_recipient_only_browser_transport: true, no_event_resigning: true };
      });
      await step("actual mutual peer revocation never grants browser accounts federation authority", async () => {
        for (const allowed of [false, true]) {
          const control = await privateControl("peer-policy", { allowed }); insist(control.result.operation.mutual_recorded_peer_approval === allowed && control.result.operation.arbitrary_endpoint_received === false, "Actual recorded service peer policy did not change");
          for (const recipient of recipients) { const response = await originalAt(recipient.origin, "/federation/libraries/" + owner); insist([401, 403].includes(response.status) && !response.body?.event, "Browser public account bypassed exact service-peer authority"); const head = await originalAt(recipient.origin, "/libraries/" + owner); insist(head.status === 200 && verify(head.body.event), "Peer revocation incorrectly disabled the separate genuine public library read policy"); }
        }
        return { actual_mutual_peer_revoke_and_restore: true, only_recorded_services_changed: true, browser_federation_denied_at_both_services: true, separate_public_library_heads_remained_verified: true };
      });
    } else if (phase === "signed-library") {
      let firstJob, backup;
      await step("reviewed snapshot signer denial preserves all independent local rows", async () => {
        const before = await databases(), send = await reviewSnapshot(); await library().getByText("Excluded IDs:", { exact: false }).filter({ hasText: nativeOnly }).waitFor({ state: "visible" }); denyKind = 11999; await send.click(); await idle(library());
        await library().getByText("Synthetic signer denied event kind 11999", { exact: false }).waitFor({ state: "visible" }); const after = await databases();
        insist(eventDigest(before.device.records) === eventDigest(after.device.records) && after.protocol.jobs.length === 0 && mutationRequests.length === 0, "Denied reviewed snapshot changed rows or transmitted a public event");
        return { explicit_review_and_consent: true, signer_denied: true, device_rows_unchanged: true, original_post_count: 0, job_count: 0 };
      });
      await step("denied authentication proof and actual unadmitted key retain one exact approved snapshot", async () => {
        await admit(false); denyKind = 27235; await (await reviewSnapshot()).click(); await idle(library());
        firstJob = await until(jobs, value => value.length === 1 && value[0].recipients[0].state === "retryable", "Denied authentication did not retain a retryable exact signed snapshot"); firstJob = firstJob[0];
        insist(firstJob.event.kind === 11999 && verify(firstJob.event) && firstJob.recipients[0].origin === protocol && firstJob.recipients[0].serviceKey === fixture.service_key && mutationRequests.length === 0, "Denied proof sent or rebound the snapshot");
        await consentDialog(jobCard(firstJob.id).getByRole("button", { name: "Retry approved snapshot", exact: true }), "original recipients"); await idle(library());
        const rejected = (await jobs()).find(job => job.id === firstJob.id); insist(rejected.recipients[0].state === "rejected" && /HTTP(?:401|403)/.test(rejected.recipients[0].message) && (await original("/events/" + firstJob.id)).status === 404 && eventDigest(rejected.event) === eventDigest(firstJob.event), "Real account admission did not refuse the exact unadmitted publication");
        return { snapshot_id: firstJob.id, kind: 11999, genuine_signature: true, denied_authentication_no_post: true, real_admission_refusal: rejected.recipients[0].message, exact_body_retained: true };
      });
      await step("explicit original account admission and exact approved retry reach genuine public projection", async () => {
        await admit(true); const count = signedCounts.get(11999);
        await consentDialog(jobCard(firstJob.id).getByRole("button", { name: "Retry approved snapshot", exact: true }), "original recipients"); await idle(library());
        const accepted = (await jobs()).find(job => job.id === firstJob.id); insist(accepted.recipients[0].state === "accepted" && eventDigest(accepted.event) === eventDigest(firstJob.event) && signedCounts.get(11999) === count, "Explicit retry signed different snapshot or lacked a real receipt");
        const visible = await counterpart(firstJob.id); insist(eventDigest(visible.body) === eventDigest(firstJob.event), "Projected original counterpart differs from the saved event");
        const peerOnly = await original("/federation/libraries/" + owner); insist([401, 403].includes(peerOnly.status) && !peerOnly.body?.event, "Public library read policy bypassed approved-peer federation authority");
        return { actual_library_staging_post: mutationRequests.some(row => row.path === "/library/batches"), actual_library_publish_post: mutationRequests.some(row => row.path === "/libraries"), event_id: firstJob.id, recipient_state: "accepted", projected_exact_counterpart_verified: true, no_snapshot_resigning: true, federation_peer_only_status: peerOnly.status };
      });
      await step("lost genuine library acceptance response retains exact-event retry through ordinary reload", async () => {
        await section(library(), "Edit saved-device series without catalogue lookup"); await library().getByLabel("Status for saved " + manga, { exact: true }).selectOption("completed"); await idle(library());
        let lost = 0; const intercept = async route => { if (route.request().method() !== "POST") return route.continue(); const response = await route.fetch({ maxRedirects: 0 }); insist([200, 202].includes(response.status()), "Original library was not accepted before deliberate response loss"); lost++; await route.abort("failed"); };
        await context.route(protocol + "/libraries", intercept);
        try { await (await reviewSnapshot()).click(); await idle(library()); } finally { await context.unroute(protocol + "/libraries", intercept); }
        const unknown = (await jobs()).find(job => job.id !== firstJob.id); insist(lost === 1 && unknown?.recipients[0].state === "retryable" && !unknown.cancelled, "Lost acceptance was claimed successful or replayed automatically");
        const exact = eventDigest(unknown.event), count = signedCounts.get(11999); await counterpart(unknown.id); await page.reload({ waitUntil: "domcontentloaded" }); await expand(library()); await section(library(), "Optional Nostr publication and verified recovery"); await selectSigner();
        const restored = (await jobs()).find(job => job.id === unknown.id); insist(eventDigest(restored.event) === exact && restored.recipients[0].origin === protocol, "Reload rebound saved uncertain publication");
        await consentDialog(jobCard(unknown.id).getByRole("button", { name: "Retry approved snapshot", exact: true }), "original recipients"); await idle(library());
        insist((await jobs()).find(job => job.id === unknown.id).recipients[0].state === "accepted" && signedCounts.get(11999) === count, "Unknown-outcome retry failed or resigned public snapshot"); firstJob = (await jobs()).find(job => job.id === unknown.id);
        return { actual_acceptance_response_lost: true, no_automatic_replay: true, reload_retained_exact_event: true, event_id: firstJob.id, event_digest: exact, explicit_same_event_retry: true };
      });
      await step("verified real relay recovery restores portable canonical rows while native rows remain separate", async () => {
        const before = await databases(); await library().getByRole("button", { name: "Preview verified public recovery", exact: true }).click(); await library().getByText("Verified public recovery: " + protocol, { exact: false }).waitFor({ state: "visible", timeout: 60000 });
        await consentDialog(library().getByRole("button", { name: "Restore reviewed canonical records", exact: true }), "verified recovery"); await idle(library());
        const state = await databases(); insist(eventDigest(state.device.records) === eventDigest(before.device.records), "Canonical recovery overwrote scoped native rows");
        insist(state.protocol.records.some(row => row.family === "series" && row.record.id === seeded.series.event_id && row.record.status === "completed") && state.protocol.records.some(row => row.family === "chapters" && row.record.id === seeded.chapters[0].event_id) && state.protocol.records.some(row => row.family === "lists" && row.record.id === deviceList.id && row.record.name === deviceList.name) && state.protocol.records.some(row => row.family === "members" && row.record.list === deviceList.id && row.record.series === seeded.series.event_id && row.record.position === 2) && state.protocol.records.every(row => !JSON.stringify(row.record).includes(nativeOnly) && !JSON.stringify(row.record).includes(manga)), "Recovery did not contain real canonical IDs/list references or leaked native IDs");
        backup = await downloadBytes(library().getByRole("button", { name: "Export portable canonical backup", exact: true })); const lines = backup.toString("utf8").trimEnd().split("\n").map(line => JSON.parse(line)); insist(lines[0].format === "manga-local-library" && lines[0].version === 2 && lines.at(-1).end === lines.length - 2 && lines.slice(1, -1).some(([family, row]) => family === "series" && row.id === seeded.series.event_id), "Actual UI canonical export was not portable reviewed JSONL");
        return { genuine_library_head_and_objects_verified_by_ui: true, canonical_series_id: seeded.series.event_id, canonical_chapter_id: seeded.chapters[0].event_id, original_list_id: deviceList.id, canonical_member_series: seeded.series.event_id, native_only_absent: true, native_rows_preserved: true, local_backup_digest: digest(backup), records: lines.at(-1).end };
      });
      await step("ordinary reload and reviewed canonical backup import preserve scope and never autopublish", async () => {
        const before = await databases(), posts = mutationRequests.length; await page.reload({ waitUntil: "domcontentloaded" }); await expand(library()); await section(library(), "Optional Nostr publication and verified recovery");
        const loaded = await databases(); insist(eventDigest(loaded.device.records) === eventDigest(before.device.records) && eventDigest(loaded.protocol.records) === eventDigest(before.protocol.records), "Ordinary reload lost native or canonical data");
        await library().getByLabel("Preview portable canonical backup", { exact: true }).setInputFiles({ name: "synthetic-canonical.jsonl", mimeType: "application/x-ndjson", buffer: backup }); await library().getByText("Canonical backup preview:", { exact: false }).waitFor({ state: "visible" });
        await consentDialog(library().getByRole("button", { name: "Restore reviewed canonical backup", exact: true }), "reviewed backup"); await idle(library());
        const imported = await databases(); insist(eventDigest(imported.device.records) === eventDigest(before.device.records) && eventDigest(imported.protocol.records) === eventDigest(before.protocol.records) && mutationRequests.length === posts, "Local canonical import rebound native scope or published");
        const deviceBackup = JSON.parse((await downloadBytes(library().getByRole("button", { name: "Export device backup", exact: true }))).toString("utf8")); deviceBackup.scope.catalogNamespace = randomUUID();
        await library().getByLabel("Preview device backup", { exact: true }).setInputFiles({ name: "different-catalogue-namespace.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(deviceBackup)) }); await library().getByText("Native IDs belong to a different instance.", { exact: false }).waitFor({ state: "visible" }); insist(await library().getByRole("button", { name: "Merge reviewed import", exact: true }).count() === 0, "Foreign catalogue namespace could silently merge");
        await library().getByRole("button", { name: "Cancel import", exact: true }).click();
        return { reload_retained_both_libraries: true, reviewed_portable_import: true, foreign_namespace_native_import_blocked: true, original_mutation_count_unchanged: true, live_namespace_rebuild_and_source_revocation: "requires genuine fixture owner seam; not tested by a backup mismatch" };
      });
    } else {
      await step("public action fixture admission and dynamic target panel remain independent from password identity", async () => { await admit(true); await prepareIdentity(); return { public_key_explicitly_admitted: true, verified_recipient: protocol, password_session: false }; });
      if (phase === "signed-profile") {
        await section(identity(), "Public profile and follow/relay lists");
        for (const kind of [0, 3, 10002]) await step(`visible reviewed kind ${kind} metadata reaches genuine current-head projection`, async () => {
          if (kind === 0) { await identity().getByLabel("Public username", { exact: true }).fill("Synthetic" + marker); await identity().getByLabel("Public display name", { exact: true }).fill("Disposable profile " + marker); await identity().getByLabel("Public bio", { exact: true }).fill("Synthetic browser acceptance profile."); await identity().getByRole("button", { name: "Review public profile", exact: true }).click(); }
          if (kind === 3) { await identity().getByLabel("Public follow keys, one 64-character hex key per line", { exact: true }).fill(fixture.operator_pubkey); await identity().getByRole("button", { name: "Review replacement public follow list", exact: true }).click(); }
          if (kind === 10002) { await identity().getByLabel("Public write relay URLs, one WSS URL per line", { exact: true }).fill(websocket.href); await identity().getByRole("button", { name: "Review replacement public relay list", exact: true }).click(); }
          await sendAction(); const saved = (await actions()).filter(row => row.event.kind === kind).at(-1); insist(saved?.state === "accepted" && saved.relay.origin === protocol && verify(saved.event), "Original metadata exact receipt missing");
          const event = await counterpart(saved.event.id); const head = await until(() => original(`/accounts/${owner}/metadata/${kind}`), value => value.status === 200 && value.body.id === saved.event.id && value.body.projected === true, "Real replaceable metadata head did not project", 60000);
          insist(eventDigest(event.body) === eventDigest(saved.event) && head.body.publisher === owner && head.body.kind === kind, "Current public metadata is not the exact signed event");
          await identity().getByRole("button", { name: "Read verified current public metadata", exact: true }).click(); await idle(identity()); await identity().getByText(`Kind ${kind}, event ${saved.event.id}, projected`, { exact: true }).waitFor({ state: "visible" });
          return { kind, event_id: saved.event.id, actual_account_metadata_post: true, projected_current_head: true, current_verified_event_rendered: true, recipient: protocol };
        });
        await step("authored metadata withdrawal uses visible exact-event review and real original kind5 endpoint", async () => {
          const profile = (await actions()).find(row => row.event.kind === 0); const item = identity().locator("div.rounded.border.p-2").filter({ has: page.getByText(`Kind 0, event ${profile.event.id}, projected`, { exact: true }) }).last(); await item.getByRole("button", { name: "Review exact authored metadata withdrawal", exact: true }).click(); await sendAction();
          const withdrawal = (await actions()).filter(row => row.event.kind === 5).at(-1); insist(withdrawal?.state === "accepted" && withdrawal.event.tags.length === 1 && withdrawal.event.tags[0][1] === profile.event.id, "Withdrawal was not bound to the exact authored profile");
          const actual = await until(() => original("/account-deletions/" + withdrawal.event.id), value => value.status === 200 && verify(value.body) && value.body.id === withdrawal.event.id, "Genuine original withdrawal counterpart missing");
          await until(() => original(`/accounts/${owner}/metadata/0`), value => value.status === 404, "Authored metadata remained current after real withdrawal projection", 60000);
          return { kind: 5, event_id: actual.body.id, exact_authored_target: profile.event.id, genuine_withdrawal_counterpart: true, current_head_removed: true };
        });
      } else if (phase === "signed-social") {
        let commentEvent;
        await step("real canonical source conversation supports reviewed signed comment and reaction", async () => {
          await loadConversation("manga", manga, seeded.series.event_id); await reviewComment("Synthetic public comment " + marker); await sendAction(); commentEvent = (await actions()).filter(row => row.event.kind === 1111).at(-1).event; await counterpart(commentEvent.id);
          await identity().getByLabel("Public reaction", { exact: true }).fill("+"); await identity().getByRole("button", { name: "Review public reaction", exact: true }).click(); await sendAction(); const reaction = (await actions()).filter(row => row.event.kind === 7).at(-1).event; await counterpart(reaction.id);
          const feed = await until(() => original(`/social?target=${seeded.series.event_id}&limit=50&offset=0`), value => value.status === 200 && value.body.public === true && [commentEvent.id, reaction.id].every(id => value.body.events.some(event => event.id === id && verify(event))), "Real social feed did not contain exact signed comment/reaction", 60000);
          await identity().getByRole("button", { name: "Load verified public conversation", exact: true }).click(); await idle(identity()); await identity().getByText("Synthetic public comment " + marker, { exact: true }).waitFor({ state: "visible" });
          return { canonical_target: seeded.series.event_id, actual_social_post: true, comment_id: commentEvent.id, reaction_id: reaction.id, real_verified_feed_count: feed.body.events.length, rendered_public_comment: true };
        });
        await step("visible reply review signs canonical root and immediate verified parent exactly", async () => {
          const article = identity().locator("article").filter({ has: page.getByText("Synthetic public comment " + marker, { exact: true }) }); await article.getByRole("button", { name: "Reply to this verified comment", exact: true }).click(); await identity().getByLabel("Public comment", { exact: true }).fill("Synthetic verified reply " + marker); await identity().getByRole("button", { name: "Review public reply", exact: true }).click(); await sendAction();
          const reply = (await actions()).filter(row => row.event.kind === 1111).at(-1).event; await counterpart(reply.id); insist(reply.tags.some(tag => tag[0] === "E" && tag[1] === seeded.series.event_id) && reply.tags.some(tag => tag[0] === "e" && tag[1] === commentEvent.id), "Reply lost exact canonical root or parent identity");
          return { reply_id: reply.id, canonical_root: seeded.series.event_id, immediate_parent: commentEvent.id, original_projection_verified: true };
        });
        await step("lost actual social acceptance response reconciles exact public counterpart without replay", async () => {
          await identity().getByRole("button", { name: "Cancel reply", exact: true }).click(); await reviewComment("Synthetic lost-response comment " + marker); let posts = 0;
          const intercept = async route => { if (route.request().method() !== "POST") return route.continue(); posts++; const response = await route.fetch({ maxRedirects: 0 }); insist([200, 202].includes(response.status()), "Social action was not genuinely accepted before response loss"); await route.abort("failed"); };
          await context.route(protocol + "/social", intercept); try { await sendAction(); } finally { await context.unroute(protocol + "/social", intercept); }
          const unknown = (await actions()).find(row => row.event.content === "Synthetic lost-response comment " + marker); insist(posts === 1 && unknown?.state === "outcome_unknown", "Unknown social outcome was called accepted or automatically retried"); await counterpart(unknown.event.id); const body = eventDigest(unknown.event), count = signedCounts.get(1111);
          await section(identity(), "Saved exact signed public-action outbox"); await actionCard(unknown.event.id).getByRole("button", { name: "Reconcile exact public counterpart", exact: true }).click(); await idle(identity()); const reconciled = (await actions()).find(row => row.event.id === unknown.event.id);
          insist(reconciled.state === "accepted" && eventDigest(reconciled.event) === body && signedCounts.get(1111) === count && posts === 1, "Actual exact-event reconciliation re-signed or replayed uncertain social action");
          return { event_id: unknown.event.id, original_response_lost: true, exact_body_digest: body, actual_counterpart_reconciled: true, no_resigning_or_replay: true };
        });
        await step("failed transport remains unknown after missing counterpart and explicit retry sends only saved event", async () => {
          await reviewComment("Synthetic failed-transport comment " + marker); let intercepted = 0;
          const intercept = async route => { if (route.request().method() !== "POST") return route.continue(); intercepted++; await route.abort("failed"); };
          await context.route(protocol + "/social", intercept); try { await sendAction(); } finally { await context.unroute(protocol + "/social", intercept); }
          const unknown = (await actions()).find(row => row.event.content === "Synthetic failed-transport comment " + marker); insist(intercepted === 1 && unknown?.state === "outcome_unknown" && (await original("/events/" + unknown.event.id)).status === 404, "Failed transport claimed acceptance or created a visible counterpart");
          const exact = eventDigest(unknown.event), count = signedCounts.get(1111); await actionCard(unknown.event.id).getByRole("button", { name: "Reconcile exact public counterpart", exact: true }).click(); await idle(identity()); const unresolved = (await actions()).find(row => row.event.id === unknown.event.id);
          insist(unresolved.state === "outcome_unknown" && eventDigest(unresolved.event) === exact && unresolved.message.includes("No visible exact counterpart yet"), "A missing public projection was treated as proof of refusal or rewrote the signed event");
          await consentDialog(actionCard(unknown.event.id).getByRole("button", { name: "Retry exact reviewed signed body", exact: true }), unknown.event.id); await idle(identity()); await counterpart(unknown.event.id); const accepted = (await actions()).find(row => row.event.id === unknown.event.id);
          insist(accepted.state === "accepted" && eventDigest(accepted.event) === exact && accepted.relay.origin === protocol && signedCounts.get(1111) === count, "Exact failed-transport retry changed public body, signature or recipient");
          return { event_id: unknown.event.id, failed_request_did_not_reach_service: true, absence_remained_unknown: true, exact_body_digest: exact, explicit_original_recipient_retry: true, projected_counterpart_verified: true, no_event_resigning: true };
        });
        await step("authored source comment withdrawal is verified through real endpoint and public feed", async () => {
          await identity().getByRole("button", { name: "Load verified public conversation", exact: true }).click(); await idle(identity()); const article = identity().locator("article").filter({ has: page.getByText("Synthetic public comment " + marker, { exact: true }) }); await article.getByRole("button", { name: "Review authored event withdrawal", exact: true }).click(); await sendAction();
          const withdrawal = (await actions()).filter(row => row.event.kind === 5).at(-1); insist(withdrawal?.state === "accepted" && withdrawal.event.tags[0][1] === commentEvent.id, "Authored comment withdrawal target was rebound");
          await until(() => original("/account-deletions/" + withdrawal.event.id), value => value.status === 200 && verify(value.body), "Original authored comment withdrawal counterpart missing");
          await until(() => original(`/social?target=${seeded.series.event_id}&limit=50&offset=0`), value => value.status === 200 && !value.body.events.some(event => event.id === commentEvent.id), "Withdrawn authored comment remained in real public feed", 60000);
          return { withdrawal_id: withdrawal.event.id, exact_authored_comment: commentEvent.id, real_public_feed_withdrawal: true, source_publication_moderation: "separate genuine operator seam remains required" };
        });
      } else {
        await step("target change during a real pending read cannot attach old source or conversation", async () => {
          await target("manga", manga); let release, waiting = false; const intercept = async route => { waiting = true; await new Promise(resolve => { release = resolve; }); await route.continue(); };
          await context.route(protocol + "/events/" + seeded.series.event_id, intercept);
          try { await identity().getByRole("button", { name: "Load verified public conversation", exact: true }).click(); await until(() => waiting, Boolean, "Real source read did not reach pending fault seam"); await target("chapter", chapter); release(); await idle(identity()); await identity().getByText(changedSourceNotice, { exact: false }).waitFor({ state: "visible" }); insist(await identity().getByText("Verified canonical source:", { exact: false }).count() === 0, "Old pending source attached to changed target"); }
          finally { release?.(); await context.unroute(protocol + "/events/" + seeded.series.event_id, intercept); }
          await loadConversation("chapter", chapter, seeded.chapters[0].event_id);
          return { genuine_read_delayed: true, target_change_cleared_old_results: true, current_source: seeded.chapters[0].event_id };
        });
        await step("unsent review cancellation and target changes discard consent without signing", async () => {
          await loadConversation("manga", manga, seeded.series.event_id); const before = signedCounts.get(1111) ?? 0, beforeActions = (await actions()).length; await reviewComment("Synthetic cancelled review " + marker); await identity().getByRole("button", { name: "Cancel review", exact: true }).click(); insist(await identity().getByRole("button", { name: "Sign and send reviewed public action", exact: true }).count() === 0, "Cancel retained public consent");
          await reviewComment("Synthetic changed-target review " + marker); await target("chapter", chapter); await until(() => identity().getByRole("button", { name: "Sign and send reviewed public action", exact: true }).count(), count => count === 0, "Target change retained old review"); insist((signedCounts.get(1111) ?? 0) === before && (await actions()).length === beforeActions, "Unsent review signed or persisted a public action");
          return { cancel_review_control: true, target_change_invalidated_review: true, no_signed_intent_created: true };
        });
        await step("signer-return target race saves exact original intent and cancellation stops every transport", async () => {
          await loadConversation("manga", manga, seeded.series.event_id); await reviewComment("Synthetic retained cancelled intent " + marker); heldKind = 1111; heldSeen = false;
          const beforePosts = mutationRequests.filter(row => row.path === "/social").length;
          await identity().getByLabel("I approve this exact public action and recipient.", { exact: true }).check(); await identity().getByRole("button", { name: "Sign and send reviewed public action", exact: true }).click(); await until(() => heldSeen, Boolean, "External signer event did not reach held prompt"); await target("chapter", chapter); heldResolve(); await idle(identity());
          const retained = (await actions()).find(row => row.event.content === "Synthetic retained cancelled intent " + marker); insist(retained?.state === "prepared" && retained.relay.origin === protocol && retained.relay.serviceKey === fixture.service_key && retained.event.tags.some(tag => tag[0] === "E" && tag[1] === seeded.series.event_id) && !retained.event.tags.some(tag => tag.includes(seeded.chapters[0].event_id)) && mutationRequests.filter(row => row.path === "/social").length === beforePosts, "Signer-return race rebound signed intent or began transport");
          await section(identity(), "Saved exact signed public-action outbox"); await actionCard(retained.event.id).getByRole("button", { name: "Cancel further sending", exact: true }).click(); await until(actions, rows => rows.find(row => row.event.id === retained.event.id)?.cancelled === true, "Exact intent cancellation was not persisted"); insist(await actionCard(retained.event.id).getByRole("button", { name: "Retry exact reviewed signed body", exact: true }).count() === 0 && (await original("/events/" + retained.event.id)).status === 404, "Cancelled unsent exact intent had a public counterpart or retry control");
          return { event_id: retained.event.id, original_recipient: protocol, original_canonical_target: seeded.series.event_id, changed_current_target: seeded.chapters[0].event_id, exact_intent_saved_before_context_abort: true, cancellation_persisted: true, transport_not_started: true };
        });
        await step("explicit exact-body retry remains bound to original recipient and source after target change", async () => {
          await loadConversation("manga", manga, seeded.series.event_id); await reviewComment("Synthetic retained retry intent " + marker); heldKind = 1111; heldSeen = false;
          await identity().getByLabel("I approve this exact public action and recipient.", { exact: true }).check(); await identity().getByRole("button", { name: "Sign and send reviewed public action", exact: true }).click(); await until(() => heldSeen, Boolean, "Second held signer prompt was not reached"); await target("chapter", nextChapter); heldResolve(); await idle(identity());
          const retained = (await actions()).find(row => row.event.content === "Synthetic retained retry intent " + marker), exact = eventDigest(retained.event), count = signedCounts.get(1111); insist(retained.state === "prepared", "Changed context started old intent transport");
          await consentDialog(actionCard(retained.event.id).getByRole("button", { name: "Retry exact reviewed signed body", exact: true }), retained.event.id); await idle(identity()); await counterpart(retained.event.id); const done = (await actions()).find(row => row.event.id === retained.event.id); insist(done.state === "accepted" && eventDigest(done.event) === exact && done.relay.origin === protocol && signedCounts.get(1111) === count, "Explicit outbox retry changed body, recipient or signature");
          const originalFeed = await original(`/social?target=${seeded.series.event_id}&limit=50&offset=0`), changedFeed = await original(`/social?target=${seeded.chapters[1].event_id}&limit=50&offset=0`); insist(originalFeed.body.events.some(event => event.id === retained.event.id) && !changedFeed.body.events.some(event => event.id === retained.event.id), "Retried action attached to newly selected target");
          return { event_id: retained.event.id, exact_body_digest: exact, original_target_only: true, original_recipient: protocol, no_event_resigning: true };
        });
        await step("a genuinely signed altered NIP07 result is rejected by application exact-template validation", async () => {
          await loadConversation("manga", manga, seeded.series.event_id); await reviewComment("Synthetic altered signer response " + marker); alteredKind = 1111; const before = (await actions()).length, posts = mutationRequests.filter(row => row.path === "/social").length; await sendAction(); await identity().getByText("Signer changed the requested event or returned an invalid signature", { exact: false }).waitFor({ state: "visible" }); insist((await actions()).length === before && mutationRequests.filter(row => row.path === "/social").length === posts, "Altered signer result was queued or transported");
          return { genuine_altered_signature: true, exact_template_module_validation_rejected: true, signed_action_not_saved: true, transport_not_started: true };
        });
      }
    }
    report.signed_protocol_scope = { phase, original_origin: protocol, catalogue_namespace: seeded.catalog_namespace, synthetic_public_key: owner, signer_secret: "ephemeral Node memory only, removed at journey teardown", fixture_responses_mocked: false, native_ids_substituted_for_canonical: false, genuine_lifecycle_coverage: lifecycleCoverage, acceptance_boundary: "Only terminal actual steps establish runtime evidence; source preparation and pending controls are not passing acceptance" };
  } finally {
    heldResolve?.(); secret.fill(0); secret = null; page.off("request", observeMutation);
    await page.evaluate(() => { delete window.nostr; }).catch(() => {});
  }
}
