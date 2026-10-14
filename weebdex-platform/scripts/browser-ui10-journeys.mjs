// Independent UI10 regressions against the real disposable platform fixture.
// Importing this module starts no fixture, browser, listener or application.
// All credentials, CSRF values, mail links and client keys remain in memory.
import { randomBytes } from "node:crypto";
import { focusReaderShortcut } from "./browser-reader-controls.mjs";

export const ui10JourneyPhases = Object.freeze([
  "ui10-api-client", "ui10-auth-lifetimes", "ui10-recovery-lifetimes",
  "ui10-preferences", "ui10-reader-transition"
]);

const readerCases = Object.freeze(["403", "404", "delayed", "normal"]);
const readerCheckSuffixes = Object.freeze(["open", "page-one", "image-bytes", "settings", "shortcut-focus", "navigation", "destination"]);
export const ui10DiagnosticChecks = Object.freeze({
  "ui10-reader-transition": Object.freeze(readerCases.flatMap(control => readerCheckSuffixes.map(check => `reader-transition-${control}-${check}`))),
  "ui10-recovery-lifetimes": Object.freeze([
    "recovery-reset-realm", "recovery-reset-hold-session", "recovery-reset-submit",
    "recovery-reset-held-session", "recovery-reset-newer-search", "recovery-reset-late-response",
    "recovery-reset-no-stale-ui", "recovery-reset-logout", "recovery-reset-old-login-form",
    "recovery-reset-old-password", "recovery-reset-old-login-settled", "recovery-reset-old-session-denied",
    "recovery-reset-new-password", "recovery-reset-link-replay"
  ])
});

export async function runUi10Journeys(h) {
  const { phase, page, context, origin, fixture, step, insist, open, redactions } = h;
  const check = (id, operation) => {
    insist(typeof h.check === "function", "The reviewed fixed-check diagnostic recorder is required for this journey");
    return h.check(id, operation);
  };
  insist(ui10JourneyPhases.includes(phase), "Unknown UI10 journey phase");
  const marker = randomBytes(6).toString("hex"), ids = fixture.ids ?? {};
  let account;
  const remember = value => { if (typeof value === "string" && value) redactions.add(value); return value; };
  const rememberFlow = body => {
    remember(body?.id);
    for (const node of body?.ui?.nodes ?? []) {
      if (["csrf_token", "password"].includes(node.attributes?.name)) remember(node.attributes?.value);
    }
    return body;
  };
  const until = async (read, valid, message, timeout = 20000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = await read(); if (valid(value)) return value;
      await page.waitForTimeout(100);
    }
    throw Error(message);
  };
  const api = async (route, options = {}) => {
    insist(route.startsWith("/api/") && !route.includes("://") && !route.startsWith("//"), "UI10 observation escaped fixture API");
    const response = await context.request.fetch(origin + route, { ...options, maxRedirects: 0, headers: { Origin: origin, ...(options.headers ?? {}) } });
    return { status: response.status(), body: /json/.test(response.headers()["content-type"] ?? "") ? await response.json() : undefined };
  };
  const control = async (route, data) => {
    const sink = new URL(fixture.mail_origin ?? fixture.control_origin ?? "http://127.0.0.1:0");
    insist(sink.protocol === "http:" && ["127.0.0.1", "localhost"].includes(sink.hostname) && sink.port && sink.port !== "0" && !sink.username && !sink.password && sink.pathname === "/" && !sink.search && !sink.hash, "UI10 needs the verified local mail/control sink");
    insist(route.startsWith("/fixture/") || route.startsWith("/mail/latest?"), "Unexpected UI10 fixture control");
    return data ? context.request.post(sink.origin + route, { data, maxRedirects: 0 }) : context.request.get(sink.origin + route, { maxRedirects: 0 });
  };
  const mailLink = async kind => {
    const mail = await until(async () => {
      const response = await control(`/mail/latest?to=${encodeURIComponent(account.email)}&kind=${kind}`);
      return response.ok() ? response.json() : undefined;
    }, value => value?.fixture_only === true && typeof value.link === "string", "Genuine local fixture mail was not delivered");
    const link = new URL(mail.link);
    insist(link.origin === origin && link.pathname.startsWith("/auth/"), "One-time mail link escaped the exact local fixture");
    remember(link.href); for (const [, value] of link.searchParams) remember(value);
    return link.href;
  };
  const freshAccount = () => {
    const suffix = randomBytes(5).toString("hex");
    return { email: remember(`ui10_${marker}_${suffix}@example.invalid`), username: `UI10${suffix}`, password: remember(`Fixture!${randomBytes(24).toString("base64url")}9a`) };
  };
  const fillRegistration = async value => {
    await page.locator('input[name="email"]').fill(value.email);
    await page.locator('input[name="username"]').fill(value.username);
    await page.locator('input[name="password"]').fill(value.password);
  };
  const verifyAccount = async () => {
    insist(account, "Synthetic UI registration prerequisite failed");
    const provision = await control("/fixture/provision", { email: account.email, profile: "user", join_seed_group: true });
    insist(provision.status() === 200, "Real synthetic account provisioning failed");
    await open("/auth/verification");
    await page.locator('input[name="email"]').fill(account.email);
    await page.getByRole("button", { name: "Send Verification Link", exact: true }).click();
    await page.goto(await mailLink("verification"), { waitUntil: "domcontentloaded" });
    await until(() => api("/api/auth/check"), value => value.status === 200 && (value.body?.identity?.is_verified || value.body?.roles?.includes("verified")), "One-time local mail did not verify the real account");
    await open("/");
    return { genuine_mail_verification: true, real_fixture_permissions: true };
  };
  const signup = async () => {
    account = freshAccount(); await open("/auth/register"); await fillRegistration(account);
    await page.getByRole("button", { name: "Create Account", exact: true }).click();
    await until(() => api("/api/auth/check"), value => value.status === 200 && value.body?.identity?.username === account.username, "Actual registration form did not establish its session");
    return verifyAccount();
  };
  const logout = async () => {
    await open("/"); await page.getByRole("link", { name: "Logout", exact: true }).first().click();
    await until(() => api("/api/auth/check"), value => [401, 204].includes(value.status), "Actual logout did not retire the session");
    await page.getByRole("link", { name: "Login", exact: true }).first().waitFor({ state: "visible" });
  };
  const fillLogin = async (password = account.password) => {
    await page.locator('input[name="identifier"]').fill(account.email);
    await page.locator('input[name="password"]').fill(password);
  };
  const search = async () => {
    await page.getByRole("link", { name: "Advanced Search", exact: true }).first().click();
    await page.waitForURL(url => url.pathname === "/search");
  };
  const enterAuthFromSearch = async kind => {
    await page.getByRole("link", { name: "Login", exact: true }).first().click();
    await page.locator('input[name="identifier"]').waitFor({ state: "visible" });
    if (kind === "registration") await page.getByRole("link", { name: "Sign up", exact: true }).click();
    if (kind === "recovery") await page.getByRole("link", { name: "Forgot your password?", exact: true }).click();
    await page.waitForURL(url => url.pathname === `/auth/${kind === "registration" ? "register" : kind}`);
  };
  const assertRouteStable = async pathname => {
    // A bounded post-release window catches queued redirects and history writes.
    const deadline = Date.now() + 900;
    while (Date.now() < deadline) { insist(new URL(page.url()).pathname === pathname, "A stale asynchronous form changed newer navigation"); await page.waitForTimeout(75); }
  };
  const realm = async () => { await page.evaluate(value => { window.__ui10RealmProof = value; }, marker); };
  const sameRealm = async () => insist(await page.evaluate(() => window.__ui10RealmProof) === marker, "Journey reloaded the document and would hide the stale-lifetime regression");
  const checked = async (targetPage, selector, value) => {
    const target = targetPage.locator(selector); await target.waitFor({ state: "visible" });
    const actual = await target.evaluate(node => node instanceof HTMLInputElement ? node.checked : node.getAttribute("aria-checked") === "true");
    if (actual !== value) await target.click();
  };
  const isChecked = target => target.evaluate(node => node instanceof HTMLInputElement ? node.checked : node.getAttribute("aria-checked") === "true");

  // Forward the real request once, then withhold only its browser delivery.
  // Cancellation is recorded separately: it is not proof of an absent write.
  const holdOne = async (pattern, predicate, inspect = async () => {}, deliver = async (route, response) => route.fulfill({ response })) => {
    let release, started = false, captured = false, settled = false, disposition = "pending", failure, requests = 0;
    const gate = new Promise(resolve => { release = resolve; });
    const intercept = async route => {
      if (!predicate(route.request())) return route.continue();
      requests++;
      if (started) return route.continue();
      started = true;
      let response;
      try { response = await route.fetch({ maxRedirects: 0 }); await inspect(response); captured = true; }
      catch { failure = "The genuine held request did not return the required fixture response"; settled = true; try { await route.abort("failed"); } catch {} return; }
      await gate;
      try { await deliver(route, response); disposition = "delivered"; }
      catch { disposition = "browser-cancelled"; }
      finally { settled = true; }
    };
    await context.route(pattern, intercept);
    return {
      wait: async () => { await until(() => captured || failure, Boolean, "The intended actual UI request was not captured"); insist(!failure, failure); },
      release: async () => { release(); await until(() => settled || !started, Boolean, "Held browser delivery did not settle"); insist(!failure, failure); },
      close: async () => { release(); if (started) await until(() => settled, Boolean, "Held route did not drain during cleanup"); await context.unroute(pattern, intercept); },
      get requests() { return requests; }, get disposition() { return disposition; }
    };
  };
  const endpoint = (kind, suffix) => origin + `/auth/self-service/${kind}/${suffix}*`;
  const redactAuthResponse = async response => { rememberFlow(await response.json()); };
  const currentFlow = async () => until(() => new URL(page.url()).searchParams.get("flow"), value => typeof value === "string" && value.length > 0, "Current visible form never attached a real server flow");

  const flowReentry = async (kind, readExisting = false) => {
    const routeName = kind === "registration" ? "register" : kind;
    let oldId;
    if (readExisting) { await open(`/auth/${routeName}`); oldId = remember(await currentFlow()); }
    const held = await holdOne(endpoint(kind, readExisting ? "flows" : "browser"), request => request.method() === "GET", async response => {
      insist(response.status() === 200, "Real auth flow read/create was rejected"); const body = rememberFlow(await response.json()); oldId = body.id;
    });
    try {
      await open(`/auth/${routeName}${readExisting ? "?flow=" + encodeURIComponent(oldId) : ""}`); await held.wait(); await realm();
      await search();
      if (readExisting) await enterAuthFromSearch(kind);
      else { await page.goBack({ waitUntil: "domcontentloaded" }); await page.waitForURL(url => url.pathname === `/auth/${routeName}`); }
      const newer = remember(await currentFlow()); insist(newer !== oldId, "Reentry did not establish a distinct current server flow");
      await page.locator(`input[name="${kind === "login" ? "identifier" : "email"}"]`).waitFor({ state: "visible" });
      const csrf = remember(await page.locator('input[name="csrf_token"]').inputValue());
      await held.release(); await assertRouteStable(`/auth/${routeName}`); await sameRealm();
      insist(new URL(page.url()).searchParams.get("flow") === newer, "Disposed form replaced the current route's flow query");
      insist(await page.locator('input[name="csrf_token"]').inputValue() === csrf, "Late flow completion replaced the current form's CSRF binding");
      return { real_flow_request_forwarded: true, held_kind: readExisting ? "flow-read" : "flow-create", distinct_same_route_reentry: true, newer_url_and_csrf_retained: true, old_delivery: held.disposition };
    } finally { await held.close(); }
  };

  if (phase === "ui10-api-client") {
    let clientId, oneTimeKey;
    await step("UI10 client prerequisite: actual registration and local verification", signup);
    await step("UI10 client: real Create201 commits once and exposes its one-time copy control", async () => {
      const before = await api("/api/client"); insist(before.status === 200 && Array.isArray(before.body), "Real private client list unavailable");
      await open("/settings/client"); await page.getByRole("button", { name: "Add New Client", exact: true }).click();
      const dialog = page.getByRole("dialog"); await dialog.getByPlaceholder("Client Name (required)", { exact: true }).fill(`UI10 ${marker}`);
      let posts = 0; const count = request => { if (new URL(request.url()).pathname === "/api/client" && request.method() === "POST") posts++; };
      page.on("request", count);
      try {
        const completed = page.waitForResponse(response => new URL(response.url()).pathname === "/api/client" && response.request().method() === "POST");
        await dialog.getByRole("button", { name: "Create", exact: true }).click(); const response = await completed;
        insist(response.status() === 201, "Actual client creation status disagrees with the retained dialog's201 contract");
        const created = await response.json(); clientId = created.id; oneTimeKey = remember(created.key);
        insist(typeof clientId === "string" && typeof oneTimeKey === "string" && oneTimeKey.startsWith("wd_"), "Real201 did not provide its created identity and one-time credential");
        await dialog.waitFor({ state: "hidden" });
        await page.getByText("Client has been successfully created", { exact: true }).waitFor({ state: "visible" });
        await page.getByRole("button", { name: "Copy Client Secret", exact: true }).waitFor({ state: "visible" });
        const after = await api("/api/client");
        insist(after.status === 200 && after.body.length === before.body.length + 1 && after.body.filter(row => row.id === clientId).length === 1 && after.body.every(row => row.key === undefined), "Created client was not durably listed exactly once without its one-time secret");
        insist(posts === 1, "A single Create action sent multiple client creation writes");
        return { genuine_status: 201, actual_create_posts: posts, durable_new_rows: 1, dialog_closed: true, one_time_copy_control: true, list_omits_credentials: true };
      } finally { page.off("request", count); }
    });
    await step("UI10 client: real clipboard copy, reload secrecy and UI revoke invalidate the credential", async () => {
      insist(clientId && oneTimeKey, "Genuine client creation prerequisite failed");
      await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin });
      try {
        await page.getByRole("button", { name: "Copy Client Secret", exact: true }).click();
        const copied = await page.evaluate(() => navigator.clipboard.readText());
        insist(copied === oneTimeKey, "Actual UI clipboard copy differed from the real one-time credential");
        await page.getByText("Client secret has been successfully copied to the clipboard", { exact: true }).waitFor({ state: "visible" });
        const allowed = await api("/api/user/follows/manga?limit=1", { headers: { Authorization: "Bearer " + oneTimeKey } });
        insist(allowed.status === 200, "The actually copied credential could not read its genuine owner-library scope");
        await page.reload({ waitUntil: "domcontentloaded" });
        await page.getByRole("button", { name: "Regenerate Client Secret", exact: true }).waitFor({ state: "visible" });
        insist(await page.getByRole("button", { name: "Copy Client Secret", exact: true }).count() === 0, "Reload exposed the creation-only credential again");
        await page.getByRole("button", { name: "Delete Client", exact: true }).click();
        const dialog = page.getByRole("dialog"); const removed = page.waitForResponse(response => new URL(response.url()).pathname === `/api/client/${clientId}` && response.request().method() === "DELETE");
        await dialog.getByRole("button", { name: "Delete", exact: true }).click(); insist((await removed).status() === 204, "UI client revoke was not acknowledged by the actual backend");
        await until(() => api("/api/client"), value => value.status === 200 && !value.body.some(row => row.id === clientId), "Revoked client remained in the genuine durable list");
        const denied = await api("/api/user/follows/manga?limit=1", { headers: { Authorization: "Bearer " + oneTimeKey } });
        insist(denied.status === 401, "The genuinely revoked credential remained usable");
        return { actual_clipboard_matches: true, live_owner_scope_200: true, refresh_omits_one_time_key: true, actual_revoke_204: true, durable_row_absent: true, revoked_bearer_401: true };
      } finally { try { await page.evaluate(() => navigator.clipboard.writeText("")); } catch {} await context.clearPermissions(); oneTimeKey = undefined; }
    });
  }

  if (phase === "ui10-auth-lifetimes") {
    for (const kind of ["login", "registration"]) {
      await step(`UI10 ${kind}: delayed real flow creation survives leave and Back reentry`, () => flowReentry(kind));
      await step(`UI10 ${kind}: delayed existing-flow read cannot replace a newer same-route flow`, () => flowReentry(kind, true));
    }
    await step("UI10 registration: delayed real success acknowledgement cannot redirect newer Search or replay signup", async () => {
      account = freshAccount(); await open("/auth/register?redirect=%2Fsettings%2Faccount"); await fillRegistration(account); await realm();
      const held = await holdOne(origin + "/auth/self-service/registration*", request => request.method() === "POST", async response => {
        const body = await response.json(); insist(response.status() === 200 && body.session?.active === true, "Held registration acknowledgement lacked actual successful session");
      });
      try {
        await page.getByRole("button", { name: "Create Account", exact: true }).click(); await held.wait(); await search();
        await held.release(); await assertRouteStable("/search"); await sameRealm();
        const actual = await api("/api/auth/check"); insist(actual.status === 200 && actual.body.identity?.username === account.username && held.requests === 1, "Abandoned registration was not durably acknowledged exactly once");
        await logout();
        return { actual_registration_posts: held.requests, real_account_session_confirmed: true, old_success_did_not_redirect: true, authentication_not_reversed: true, cleanup_real_logout: true };
      } finally { await held.close(); }
    });
    await step("UI10 registration: delayed successful Session refresh cannot redirect newer Search", async () => {
      account = freshAccount(); await open("/auth/register?redirect=%2Fsettings%2Faccount"); await fillRegistration(account); await realm();
      const held = await holdOne(origin + "/api/auth/check", request => request.method() === "GET", async response => {
        const body = await response.json(); insist(response.status() === 200 && body.identity?.username === account.username, "Held post-registration Session refresh lacked genuine new account");
      });
      try {
        let posts = 0; const count = request => { if (new URL(request.url()).pathname === "/auth/self-service/registration" && request.method() === "POST") posts++; }; page.on("request", count);
        try {
          await page.getByRole("button", { name: "Create Account", exact: true }).click(); await held.wait();
          await search(); await held.release(); await assertRouteStable("/search"); await sameRealm();
          const actual = await api("/api/auth/check"); insist(actual.status === 200 && actual.body.identity?.username === account.username, "Sent registration did not remain acknowledged by the actual server");
          insist(posts === 1, "Abandoned registration was replayed");
          return { actual_registration_posts: posts, genuine_account_session: true, late_session_result_did_not_redirect: true, newer_search_retained: true };
        } finally { page.off("request", count); }
      } finally { await held.close(); }
    });
    await step("UI10 auth prerequisite: verify the genuinely registered account", verifyAccount);
    for (const holdRefresh of [false, true]) await step(`UI10 login: delayed real ${holdRefresh ? "Session refresh" : "success acknowledgement"} cannot redirect newer Search`, async () => {
      insist(account, "Actual registered account prerequisite failed"); await logout(); await open("/auth/login?redirect=%2Fsettings%2Faccount"); await fillLogin(); await realm();
      const held = await holdOne(holdRefresh ? origin + "/api/auth/check" : origin + "/auth/self-service/login*", request => request.method() === (holdRefresh ? "GET" : "POST"), async response => {
        const body = await response.json(); insist(response.status() === 200 && (holdRefresh ? body.identity?.username === account.username : body.session?.active === true), "Held login completion was not a genuine successful backend response");
      });
      let posts = 0; const count = request => { if (new URL(request.url()).pathname === "/auth/self-service/login" && request.method() === "POST") posts++; }; page.on("request", count);
      try {
        await page.getByRole("button", { name: "Sign in", exact: true }).click(); await held.wait(); await search();
        await held.release(); await assertRouteStable("/search"); await sameRealm();
        const actual = await api("/api/auth/check"); insist(actual.status === 200 && actual.body.identity?.username === account.username, "Sent login was not durable at the actual server");
        insist(posts === 1, "Discarding an old login UI replayed the authentication write");
        return { held_stage: holdRefresh ? "session-refresh" : "login-acknowledgement", actual_login_posts: posts, actual_session_confirmed: true, stale_redirect_discarded: true, server_authentication_not_reversed: true };
      } finally { page.off("request", count); await held.close(); }
    });
    await step("UI10 settings: delayed initial Session readiness cannot reclaim a departed route", async () => {
      const held = await holdOne(origin + "/api/auth/check", request => request.method() === "GET", async response => { insist(response.status() === 200, "Real settings initial Session read failed"); });
      try {
        await open("/settings/preferences"); await held.wait(); await realm(); await search();
        await held.release(); await assertRouteStable("/search"); await sameRealm();
        return { actual_initial_session_read: true, destroyed_settings_layout_cannot_redirect: true, newer_search_retained: true };
      } finally { await held.close(); }
    });
    await step("UI10 auth: current login still authenticates once and reaches its chosen return target", async () => {
      await logout(); await open("/auth/login?redirect=%2Fsettings%2Faccount"); await fillLogin();
      let posts = 0; const count = request => { if (new URL(request.url()).pathname === "/auth/self-service/login" && request.method() === "POST") posts++; }; page.on("request", count);
      try {
        await page.getByRole("button", { name: "Sign in", exact: true }).click(); await page.waitForURL(url => url.pathname === "/settings/account");
        insist((await api("/api/auth/check")).body?.identity?.username === account.username && posts === 1, "Current flow failed real authentication or replayed it");
        return { current_flow_authenticates: true, actual_login_posts: posts, approved_return_target_reached: true };
      } finally { page.off("request", count); }
    });
  }

  if (phase === "ui10-recovery-lifetimes") {
    await step("UI10 recovery: delayed anonymous flow creation survives leave and Back reentry", () => flowReentry("recovery"));
    await step("UI10 recovery: delayed existing-flow read cannot replace a newer recovery flow", () => flowReentry("recovery", true));
    await step("UI10 recovery prerequisite: actual verified account and real logout", async () => { await signup(); await logout(); return { verified_account: true, actual_logout: true }; });
    let oneTimeLink;
    await step("UI10 recovery: delayed real mail-request response cannot overwrite a reopened request form", async () => {
      await open("/auth/recovery"); await page.locator('input[name="email"]').fill(account.email); await realm();
      const original = remember(await currentFlow());
      const held = await holdOne(origin + "/auth/self-service/recovery*", request => request.method() === "POST", async response => { insist(response.status() === 200, "Genuine recovery mail request was rejected"); await redactAuthResponse(response); });
      try {
        await page.getByRole("button", { name: "Send Recovery Link", exact: true }).click(); await held.wait();
        oneTimeLink = await mailLink("recovery"); await search(); await enterAuthFromSearch("recovery");
        const newer = remember(await currentFlow()); insist(newer !== original, "Recovery request reentry did not create a distinct form");
        await held.release(); await assertRouteStable("/auth/recovery"); await sameRealm();
        insist(new URL(page.url()).searchParams.get("flow") === newer && await page.locator('input[name="email"]').inputValue() === "", "Late mail acknowledgement overwrote the reopened request form");
        return { real_mail_delivered: true, mail_request_not_reversed: true, reopened_flow_preserved: true, late_request_response_discarded: true };
      } finally { await held.close(); }
    });
    await step("UI10 recovery: restricted settings-flow read is owned by the current reset-only form", async () => {
      insist(oneTimeLink, "Actual local recovery mail prerequisite failed");
      let oldFlow;
      const held = await holdOne(endpoint("settings", "browser"), request => request.method() === "GET", async response => { insist(response.status() === 200, "Actual restricted settings flow creation failed"); oldFlow = rememberFlow(await response.json()); });
      try {
        await page.goto(oneTimeLink, { waitUntil: "domcontentloaded" }); await held.wait(); await realm();
        const capability = await api("/api/auth/check"); insist(capability.status === 200 && capability.body?.session_purpose === "recovery" && capability.body.identity === undefined && capability.body.preferences === undefined && capability.body.follows === undefined && !capability.body.roles?.length && !capability.body.permissions?.length, "Recovery credential acquired normal account authority");
        // A recovery capability cannot create a login flow. Back remounts the
        // allowed reset-only route without broadening that server authority.
        await search(); await page.goBack({ waitUntil: "domcontentloaded" });
        await page.waitForURL(url => url.pathname === "/auth/recovery");
        await page.getByRole("button", { name: "Reset Password", exact: true }).waitFor({ state: "visible" });
        const newerCsrf = remember(await page.locator('input[name="csrf_token"]').inputValue());
        const oldCsrf = oldFlow.ui.nodes.find(node => node.attributes?.name === "csrf_token")?.attributes?.value;
        insist(newerCsrf !== oldCsrf, "Restricted reentry did not create a distinct CSRF-bound settings flow");
        await held.release(); await assertRouteStable("/auth/recovery"); await sameRealm();
        insist(await page.locator('input[name="csrf_token"]').inputValue() === newerCsrf && await page.locator('input[name="email"],input[name="username"]').count() === 0, "Old restricted settings response replaced current password-only controls");
        insist((await api("/api/user/follows/manga")).status === 403, "Restricted recovery credential read the private account library");
        return { genuine_recovery_capability: true, current_csrf_retained: true, password_only_controls: true, real_private_library_403: true, late_settings_read_discarded: true };
      } finally { await held.close(); oldFlow = undefined; }
    });
    await step("UI10 recovery: delayed acknowledged reset Session refresh preserves newer navigation and actual password outcome", async () => {
      insist(typeof h.check === "function", "The reviewed fixed-check diagnostic recorder is required for this recovery journey");
      const check = (id, operation) => h.check(id, operation);
      const replacement = remember(`Recovered!${randomBytes(24).toString("base64url")}9a`);
      await check("recovery-reset-realm", realm);
      const held = await check("recovery-reset-hold-session", () => holdOne(origin + "/api/auth/check", request => request.method() === "GET", async response => { const body = await response.json(); insist(response.status() === 200 && body.identity?.username === account.username && body.session_purpose !== "recovery", "Real reset did not rotate to a normal account session"); }));
      let posts = 0; const count = request => { if (new URL(request.url()).pathname === "/auth/self-service/settings" && request.method() === "POST") posts++; }; page.on("request", count);
      try {
        await check("recovery-reset-submit", async () => { await page.locator('input[name="password"]').fill(replacement); await page.getByRole("button", { name: "Reset Password", exact: true }).click(); });
        await check("recovery-reset-held-session", () => held.wait());
        await check("recovery-reset-newer-search", search);
        await check("recovery-reset-late-response", async () => { await held.release(); await assertRouteStable("/search"); await sameRealm(); });
        await check("recovery-reset-no-stale-ui", async () => { insist(await page.getByText("Password reset completed", { exact: true }).count() === 0 && posts === 1, "Old reset completion leaked onto newer UI or replayed its write"); });
        await check("recovery-reset-logout", logout);
        await check("recovery-reset-old-login-form", async () => { await open("/auth/login"); await fillLogin(); });
        await check("recovery-reset-old-password", async () => {
          // Both the real response and click are owned immediately, including
          // failure, so a later login-response timeout cannot go unhandled.
          const [rejected] = await Promise.all([
            page.waitForResponse(response => new URL(response.url()).pathname === "/auth/self-service/login" && response.request().method() === "POST"),
            page.getByRole("button", { name: "Sign in", exact: true }).click()
          ]);
          insist(rejected.status() === 400, "Actual old-password login was not rejected by the server");
        });
        await check("recovery-reset-old-login-settled", () => until(() => page.locator('input[name="password"]').isEnabled(), Boolean, "Rejected old-password login never settled"));
        await check("recovery-reset-old-session-denied", async () => { insist((await api("/api/auth/check")).status === 401, "Old password remained usable after the actually acknowledged reset"); });
        await check("recovery-reset-new-password", async () => {
          account.password = replacement; await fillLogin(); await page.getByRole("button", { name: "Sign in", exact: true }).click();
          await until(() => api("/api/auth/check"), value => value.status === 200 && value.body?.identity?.username === account.username, "Replacement password failed actual login after abandoned reset UI");
        });
        await check("recovery-reset-link-replay", async () => { const replay = await context.request.get(oneTimeLink, { maxRedirects: 0 }); insist(replay.status() === 400, "Actual one-time recovery link remained replayable"); });
        return { actual_password_posts: posts, discarded_stale_reset_ui: true, newer_search_retained: true, old_password_401: true, new_password_authenticated: true, one_time_link_replay_400: true };
      } finally { page.off("request", count); await held.close(); }
    });
  }

  if (phase === "ui10-preferences") {
    await step("UI10 preferences prerequisite: actual verified synthetic account", signup);
    for (const fail of [false, true]) await step(`UI10 preferences: ${fail ? "failed" : "late"} real tag read preserves an edited rating draft`, async () => {
      const actual = await api("/api/auth/check"); insist(actual.status === 200 && actual.body?.preferences?.version > 0, "Actual versioned account preferences unavailable");
      const expected = !actual.body.preferences.show_safe;
      const held = await holdOne(origin + "/api/manga/tag*", request => request.method() === "GET", async response => { insist(response.status() === 200, "Genuine tag catalogue read failed before controlled delay"); }, fail ? (route => route.abort("failed")) : undefined);
      try {
        await open("/settings/preferences"); await held.wait();
        await checked(page, "#safe", expected); insist(await page.getByRole("button", { name: "Save", exact: true }).isEnabled(), "Editing the actual rating did not make the draft dirty");
        await held.release(); await page.waitForTimeout(450);
        insist(await isChecked(page.locator("#safe")) === expected && await page.getByRole("button", { name: "Save", exact: true }).isEnabled(), "Tag-read completion erased the active edited preference draft");
        const beforeReset = await api("/api/auth/check"); insist(beforeReset.body.preferences.version === actual.body.preferences.version && beforeReset.body.preferences.show_safe === !expected, "An unsaved UI draft unexpectedly changed the persisted account");
        await page.getByRole("button", { name: "Revert", exact: true }).click();
        insist(await isChecked(page.locator("#safe")) === !expected && await page.getByRole("button", { name: "Save", exact: true }).isDisabled(), "Explicit Reset did not restore the actual stored preference");
        return { genuine_tag_request_forwarded: true, controlled_failure_after_real_read: fail, dirty_edit_retained: true, unsaved_server_state_unchanged: true, explicit_reset_restores_actual: true };
      } finally { await held.close(); }
    });
    await step("UI10 preferences: genuine current Save persists the reviewed rating once and reload hydrates it", async () => {
      await open("/settings/preferences"); const before = await api("/api/auth/check"), desired = !before.body.preferences.show_safe;
      await checked(page, "#safe", desired);
      let writes = 0; const count = request => { if (new URL(request.url()).pathname === "/api/settings" && request.method() === "PUT") writes++; }; page.on("request", count);
      try {
        const response = page.waitForResponse(value => new URL(value.url()).pathname === "/api/settings" && value.request().method() === "PUT");
        await page.getByRole("button", { name: "Save", exact: true }).click(); insist((await response).status() === 200, "Actual preference Save was rejected");
        await page.getByText("Your changes have been saved.", { exact: true }).waitFor({ state: "visible" });
        const actual = await api("/api/auth/check"); insist(actual.body.preferences.show_safe === desired && actual.body.preferences.version === before.body.preferences.version + 1 && writes === 1, "Reviewed preference did not persist in exactly one acknowledged version");
        await page.reload({ waitUntil: "domcontentloaded" }); await page.locator("#safe").waitFor({ state: "visible" });
        insist(await isChecked(page.locator("#safe")) === desired && await page.getByRole("button", { name: "Save", exact: true }).isDisabled(), "Reload failed to hydrate the acknowledged preference baseline");
        return { actual_put_status: 200, actual_save_writes: writes, exact_version_increment: 1, real_reload_hydrated: true };
      } finally { page.off("request", count); }
    });
    await step("UI10 preferences: two actual tabs produce a real409 without silently rebasing the dirty draft", async () => {
      await open("/settings/preferences"); const baseline = (await api("/api/auth/check")).body.preferences;
      await checked(page, "#safe", !baseline.show_safe);
      const rival = await context.newPage();
      try {
        insist((await rival.goto(origin + "/settings/preferences", { waitUntil: "domcontentloaded" }))?.status() === 200, "Real competing UI tab did not open");
        await checked(rival, "#suggestive", !baseline.show_suggestive);
        const committed = rival.waitForResponse(response => new URL(response.url()).pathname === "/api/settings" && response.request().method() === "PUT");
        await rival.getByRole("button", { name: "Save", exact: true }).click(); insist((await committed).status() === 200, "Competing actual UI Save failed");
        const newer = await api("/api/auth/check"); insist(newer.body.preferences.version === baseline.version + 1 && newer.body.preferences.show_suggestive === !baseline.show_suggestive, "Competing UI Save did not establish its actual newer version");
        const response = page.waitForResponse(value => new URL(value.url()).pathname === "/api/settings" && value.request().method() === "PUT");
        await page.getByRole("button", { name: "Save", exact: true }).click(); insist((await response).status() === 409, "Old dirty preference baseline was silently rebased instead of producing the genuine version conflict");
        await page.getByText("Version Mismatch", { exact: true }).waitFor({ state: "visible" });
        insist(await isChecked(page.locator("#safe")) === !baseline.show_safe && await page.getByRole("button", { name: "Save", exact: true }).isEnabled(), "Conflict erased the user's dirty draft");
        const persisted = await api("/api/auth/check"); insist(persisted.body.preferences.version === newer.body.preferences.version && persisted.body.preferences.show_safe === baseline.show_safe && persisted.body.preferences.show_suggestive === !baseline.show_suggestive, "Rejected stale Save overwrote the competing committed preferences");
        await page.reload({ waitUntil: "domcontentloaded" }); await page.locator("#safe").waitFor({ state: "visible" });
        insist(await isChecked(page.locator("#safe")) === baseline.show_safe && await isChecked(page.locator("#suggestive")) === !baseline.show_suggestive, "Refresh failed to restore actual post-conflict preferences");
        return { competing_real_ui_save_200: true, real_stale_put_409: true, dirty_draft_retained: true, latest_persisted_version_preserved: true, refresh_reconciled_actual: true };
      } finally { await rival.close(); }
    });
  }

  if (phase === "ui10-reader-transition") {
    const previous = ids.chapter, current = ids.next_chapter, unrelated = ids.alternative_language_chapter, manga = ids.manga;
    const deviceRow = async id => page.evaluate(async chapterId => {
      const name = "weebdex-device-library-v1:" + encodeURIComponent(location.origin);
      if (!(await indexedDB.databases()).some(row => row.name === name)) return undefined;
      const db = await new Promise((resolve, reject) => { const query = indexedDB.open(name); query.onsuccess = () => resolve(query.result); query.onerror = () => reject(query.error); });
      try { return await new Promise((resolve, reject) => { const query = db.transaction("records", "readonly").objectStore("records").getAll(); query.onsuccess = () => resolve(query.result.find(row => row.family === "chapters" && row.id === chapterId)); query.onerror = () => reject(query.error); }); }
      finally { db.close(); }
    }, id);
    const readyReader = async (id, prefix) => {
      const checkpoint = (suffix, operation) => prefix ? check(prefix + suffix, operation) : operation();
      await checkpoint("page-one", async () => {
        await until(() => page.title(), value => value.startsWith("Page 1:"), "Reader did not remain at its explicit one-based page-one route");
        insist(new URL(page.url()).pathname === `/chapter/${id}/1`, "Reader changed explicit page-one URL before user reading");
      });
      await checkpoint("image-bytes", () => until(() => page.locator('img[src^="blob:"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0).length), count => count > 0, "Actual fixture image bytes did not decode"));
    };
    const settings = async () => {
      await page.getByRole("button", { name: "Single", exact: true }).click();
      await page.getByRole("button", { name: "Left-to-right", exact: true }).click();
      await checked(page, "#auto_advance", true);
    };
    const prepareReader = async control => {
      const prefix = `reader-transition-${control}-`;
      await check(prefix + "open", () => open(`/chapter/${current}/1`));
      await readyReader(current, prefix);
      await check(prefix + "settings", settings);
      await check(prefix + "shortcut-focus", () => focusReaderShortcut(page, insist));
      await realm();
    };
    const unrelatedFromTitle = async () => {
      await page.waitForURL(url => url.pathname.startsWith(`/title/${manga}`));
      await page.locator(`a[href="/chapter/${unrelated}"]`).first().click();
      // The retained title entry is a normal chapter link; the reader supplies1.
      await readyReader(unrelated); await sameRealm();
      const row = await until(() => deviceRow(unrelated), value => value?.page === 1, "Unrelated reader page-one progress did not persist on this device");
      insist(row.completed === false && !row.readAt, "Unrelated page-one navigation recorded completion without reading");
      await page.waitForTimeout(500); const settled = await deviceRow(unrelated);
      insist(new URL(page.url()).pathname === `/chapter/${unrelated}/1` && settled?.page === 1 && settled.completed === false, "A late previous-chapter intent completed an unrelated reader");
    };
    const unrelatedViaControls = async () => {
      // Keep the same JS realm: a page.goto here would conceal the old module-global flag.
      await page.getByRole("link", { name: "Home", exact: true }).first().click();
      await page.waitForURL(url => url.pathname === "/");
      await page.locator(`a[href^="/title/${manga}"]`).first().click();
      await unrelatedFromTitle();
    };
    await step("UI10 reader prerequisite: real ordered chapters and anonymous device-only authority", async () => {
      insist(previous && current && unrelated && manga && new Set([previous, current, unrelated]).size === 3, "Three distinct recorded fixture chapter identities are required");
      for (const id of [previous, current, unrelated]) { const response = await api(`/api/chapter/${id}`); insist(response.status === 200 && response.body?.data?.length >= 3, "Real distinct multi-page fixture chapter unavailable"); }
      insist((await api("/api/auth/check")).status === 401, "Reader device-only phase unexpectedly has account authority");
      return { real_chapters: 3, multi_page_bytes: true, anonymous_device_mode: true, unrelated_destination_is_distinct_japanese_alternative: true };
    });
    for (const status of [403, 404]) await step(`UI10 reader: controlled previous-detail${status} cannot make an unrelated chapter open last or completed`, async () => {
      await prepareReader(String(status));
      let failures = 0;
      const pattern = origin + `/api/chapter/${previous}`;
      const intercept = async route => {
        if (route.request().method() !== "GET") return route.continue();
        const actual = await route.fetch();
        if (actual.status() !== 200) { await route.fulfill({ response: actual }); return; }
        failures++;
        await route.fulfill({ status, contentType: "application/json", body: JSON.stringify({ error: { message: "Synthetic unavailable previous chapter detail" } }) });
      };
      await context.route(pattern, intercept);
      try {
        await check(`reader-transition-${status}-navigation`, async () => {
          await page.keyboard.press("ArrowLeft");
          await page.getByText(status === 403 ? "403 - Forbidden" : "404 - Not Found", { exact: true }).waitFor({ state: "visible" });
          insist(failures === 1, "Previous-page control did not request the intended genuine chapter once");
        });
      } finally { await context.unroute(pattern, intercept); }
      await check(`reader-transition-${status}-destination`, unrelatedViaControls);
      return { actual_previous_detail_forwarded: true, controlled_ui_error: status, intended_previous_requests: failures, same_document_navigation: true, unrelated_url_page: 1, actual_device_page: 1, actual_device_completed: false };
    });
    await step("UI10 reader: delayed genuine previous navigation cannot supersede unrelated page-one reentry", async () => {
      await prepareReader("delayed");
      const held = await holdOne(origin + `/api/chapter/${previous}`, request => request.method() === "GET", async response => { insist(response.status() === 200, "Actual delayed previous detail was unavailable"); });
      try {
        await check("reader-transition-delayed-navigation", async () => { await page.keyboard.press("ArrowLeft"); await held.wait(); });
        await check("reader-transition-delayed-destination", async () => {
          await page.locator(`a[href^="/title/${manga}"]`).first().click(); await unrelatedFromTitle();
          await held.release(); await assertRouteStable(`/chapter/${unrelated}/1`); await sameRealm();
          const row = await deviceRow(unrelated); insist(row?.page === 1 && row.completed === false, "Superseded previous navigation changed unrelated persisted progress");
          await page.goBack({ waitUntil: "domcontentloaded" }); await page.waitForURL(url => url.pathname.startsWith(`/title/${manga}`));
          await page.goForward({ waitUntil: "domcontentloaded" }); await readyReader(unrelated); await sameRealm();
          const restored = await deviceRow(unrelated); insist(restored?.page === 1 && restored.completed === false, "Back/Forward reentry inherited an abandoned last-page intent");
        });
        return { real_delayed_detail_200: true, actual_title_and_chapter_controls: true, old_delivery: held.disposition, superseding_page_one_retained: true, device_completed: false, back_forward_retains_page_one: true };
      } finally { await held.close(); }
    });
    await step("UI10 reader: normal automatic previous chapter opens only its intended last page", async () => {
      await prepareReader("normal");
      const count = (await api(`/api/chapter/${previous}`)).body.data.length;
      await check("reader-transition-normal-navigation", async () => {
        await page.keyboard.press("ArrowLeft"); await page.waitForURL(url => url.pathname === `/chapter/${previous}/${count}`);
        await until(() => page.title(), value => value.startsWith(`Page ${count}:`), "Successful previous transition did not select the intended chapter's actual last page");
        await sameRealm(); const persisted = await until(() => deviceRow(previous), row => row?.page === count && row.completed === true, "Intended last-page progress was not really persisted");
        insist(persisted.completed, "Normal previous last-page progress lost completion");
      });
      await check("reader-transition-normal-destination", async () => { await page.locator(`a[href^="/title/${manga}"]`).first().click(); await unrelatedFromTitle(); });
      return { successful_previous_target_exact: true, actual_last_page: count, target_progress_completed: true, unrelated_transition_not_inherited: true };
    });
  }
}
