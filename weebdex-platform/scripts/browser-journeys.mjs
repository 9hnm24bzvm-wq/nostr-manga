// Real user-control journeys. Synthetic secrets stay only in harness memory.
import { randomBytes } from "node:crypto";
import { deflateSync } from "node:zlib";
import { createAccountRecoveryChecks, submitRecoveryPassword } from "./browser-account-recovery.mjs";
import { selectTitleLibraryStatus, titleLibraryTrigger, titleFollowCheckSuffixes } from "./browser-select-controls.mjs";
import { expandDetailsPanel } from "./browser-details-controls.mjs";
export const localLibraryDiagnosticChecks = Object.freeze({ 'local-library': Object.freeze([
  'device-title-navigation', ...titleFollowCheckSuffixes.map(suffix => 'device-follow-' + suffix),
  'device-status-durable', 'device-authority-unpublished', 'device-reload-status', 'device-library-navigation',
  'custom-list-navigation', 'custom-list-durable', 'custom-member-durable', 'custom-backup-export',
  'custom-foreign-backup-fenced', 'custom-backup-merged', 'account-mode-ready',
  ...titleFollowCheckSuffixes.map(suffix => 'account-follow-' + suffix), 'account-status-durable',
  'account-device-preserved', 'account-import-reviewed', 'signer-section-open', 'signer-binding-installed',
  'signer-dismissal-visible', 'signer-outbox-empty'
].map(suffix => 'local-library-' + suffix)) });
export async function runJourneys(h) {
	const { phase, page, context, origin, fixture, step, insist, open, report, redactions } = h;
	const ids = fixture.ids ?? {};
	const manga = ids.manga ?? "SeedManga1", chapter = ids.chapter ?? "SeedChapt1";
	const sink = new URL(fixture.mail_origin ?? fixture.control_origin ?? "http://127.0.0.1:0");
	const validSink = ["127.0.0.1", "localhost"].includes(sink.hostname) && sink.protocol === "http:" && sink.port !== "0";
	let account, uploaded;
	const marker = randomBytes(6).toString("hex");
	const api = async (route, options = {}) => {
		insist(route.startsWith("/") && !route.startsWith("//") && !route.includes("://"), "API must stay inside the local fixture");
		const apiRoute = route.startsWith("/api/") ? route : "/api" + route;
		const response = await context.request.fetch(origin + apiRoute, { ...options, headers: { Origin: origin, ...(options.headers ?? {}) } });
		return { status: response.status(), headers: response.headers(), body: /json/.test(response.headers()["content-type"] ?? "") ? await response.json() : undefined };
	};
	const control = async (route, data) => {
		insist(validSink, "Verified local fixture mail/control origin required");
		insist(route.startsWith("/fixture/") || route.startsWith("/mail/latest?"), "Unexpected fixture control route");
		return data ? context.request.post(sink.origin + route, { data }) : context.request.get(sink.origin + route);
	};
	const until = async (read, valid, message, timeout = 18000) => {
		const deadline = Date.now() + timeout;
		while (Date.now() < deadline) { const value = await read(); if (valid(value)) return value; await page.waitForTimeout(250); }
		throw new Error(message);
	};
	const checked = async (selector, value) => {
		const target = page.locator(selector);
		await target.waitFor({ state: "visible" });
		const old = await target.evaluate(node => node instanceof HTMLInputElement ? node.checked : node.getAttribute("aria-checked") === "true");
		if (old !== value) await target.click();
	};
	const signup = async (profile = "user") => {
		const email = `browser_${marker}@example.invalid`, username = `Browser${marker}`, password = `Fixture!${randomBytes(24).toString("base64url")}9a`;
		redactions.add(password); redactions.add(email);
		await open("/auth/register");
		await page.locator('input[name="email"]').fill(email);
		await page.locator('input[name="username"]').fill(username);
		await page.locator('input[name="password"]').fill(password);
		await page.getByRole("button", { name: "Create Account", exact: true }).click();
		await until(() => api("/auth/check"), value => value.status === 200 && value.body?.identity?.username === username, "Registration did not establish a real session");
		const provision = await control("/fixture/provision", { email, profile, join_seed_group: true });
		insist(provision.status() === 200, "Fixture could not provision synthetic account permissions");
		account = { email, username, password };
		await open("/auth/verification");
		await page.locator('input[name="email"]').fill(email);
		await page.getByRole("button", { name: "Send Verification Link", exact: true }).click();
		const mail = await until(async () => { const response = await control(`/mail/latest?to=${encodeURIComponent(email)}&kind=verification`); return response.ok() ? response.json() : undefined; }, value => value?.fixture_only && value.link, "Local verification email was not delivered");
		const link = new URL(mail.link); insist(link.origin === origin, "Fixture verification link escaped local platform");
		redactions.add(link.href); for (const [, value] of link.searchParams) redactions.add(value);
		await page.goto(link.href, { waitUntil: "domcontentloaded" });
		await until(() => api("/auth/check"), value => value.status === 200 && (value.body?.identity?.is_verified || value.body?.roles?.includes("verified")), "One-time verification link did not verify account");
		await open("/");
		return { registered_by_ui: true, verified_by_one_time_local_mail: true, profile, session_cookie_present: (await context.cookies()).some(cookie => /session/i.test(cookie.name)) };
	};
	const logout = async () => {
		await open("/");
		const link = page.getByRole("link", { name: "Logout", exact: true }).first();
		await link.waitFor({ state: "visible" }); await link.click();
		await until(() => api("/auth/check"), value => value.status === 401 || value.status === 204, "Logout did not invalidate the real session");
	};
	const login = async password => {
		await open("/auth/login");
		await page.locator('input[name="identifier"]').fill(account.email);
		await page.locator('input[name="password"]').fill(password);
		await page.getByRole("button", { name: "Sign in", exact: true }).click();
	};
	const device = async () => page.evaluate(async () => {
		const name = 'weebdex-device-library-v1:' + encodeURIComponent(location.origin);
		if (!(await indexedDB.databases()).some(db => db.name === name)) return undefined;
		const db = await new Promise((resolve, reject) => { const query = indexedDB.open(name); query.onsuccess = () => resolve(query.result); query.onerror = () => reject(query.error); });
		try { return await new Promise((resolve, reject) => { const tx = db.transaction(['records', 'meta'], 'readonly'), rows = tx.objectStore('records').getAll(), scope = tx.objectStore('meta').get('instanceScope'), revision = tx.objectStore('meta').get('revision'); tx.oncomplete = () => resolve({ records: rows.result, scope: scope.result, revision: revision.result }); tx.onerror = () => reject(tx.error); }); } finally { db.close(); }
	});
	const outboxCount = async () => page.evaluate(async () => {
		if (!(await indexedDB.databases()).some(db => db.name === 'weebdex-protocol-library-v1')) return 0;
		const db = await new Promise((resolve, reject) => { const query = indexedDB.open('weebdex-protocol-library-v1'); query.onsuccess = () => resolve(query.result); query.onerror = () => reject(query.error); });
		try { return await new Promise((resolve, reject) => { const query = db.transaction('jobs').objectStore('jobs').count(); query.onsuccess = () => resolve(query.result); query.onerror = () => reject(query.error); }); } finally { db.close(); }
	});
	const panel = () => page.locator('[data-local-library-panel]').first();
	const expandPanel = () => expandDetailsPanel(panel());
	const follow = status => selectTitleLibraryStatus(page, status);
	const confirm = async name => { await page.getByRole('button', { name, exact: true }).first().click(); const dialog = page.getByRole('dialog'); await dialog.getByRole('button', { name, exact: true }).click(); };
	const image = png(64, 96, marker);
	if (["account", "account-recovery", "upload", "announcements", "community", "editorial"].includes(phase)) await step("register and verify synthetic account using real forms and local email", () => signup(["announcements", "editorial"].includes(phase) ? "staff" : phase === "community" ? "moderator" : "user"));
	if (phase === "account") {
		await step("preference save uses authoritative version and persists after reload", async () => {
			insist(account, "Registration prerequisite failed");
			const before = await api("/auth/check"); insist(before.body?.preferences?.version > 0, "Session preferences have no authoritative version");
			await open("/settings/preferences");
			await checked("#safe", false); await checked("#suggestive", false); await checked("#erotic", false); await checked("#hentai", false);
			await page.getByRole("button", { name: "Save", exact: true }).click();
			const saved = await until(() => api("/auth/check"), value => value.body?.preferences?.version > before.body.preferences.version && ["safe", "suggestive", "erotic", "hentai"].every(key => value.body.preferences[`show_${key}`] === false), "Preferences did not persist");
			await page.reload({ waitUntil: "domcontentloaded" }); await page.locator("#safe").waitFor();
			insist(await page.locator("#safe").getAttribute("aria-checked") === "false", "Reload lost saved rating state");
			return { version_advanced: true, all_false_persisted: true, reload_hydrated: true };
		});
		await step("SPA logout, rejected login, successful login refresh real Session", async () => {
			insist(account, "Registration prerequisite failed"); await logout();
			const wrong = `Wrong!${randomBytes(12).toString("hex")}`; redactions.add(wrong); await login(wrong); await page.waitForTimeout(1200);
			insist((await api("/auth/check")).status !== 200, "Invalid password established a session");
			await login(account.password); await until(() => api("/auth/check"), value => value.status === 200 && value.body?.identity?.username === account.username, "Successful login did not refresh session");
			await page.getByRole("link", { name: "Logout", exact: true }).first().waitFor({ state: "visible" });
			return { logout_invalidated: true, wrong_password_rejected: true, spa_session_controls_updated: true };
		});
	} else if (phase === "account-recovery") {
    let localBefore, oneTimeLink, newPassword, restrictedReady = false;
    const recoveryCheck = createAccountRecoveryChecks(report, h.save);
    await step("real mail recovery mounts reset-only UI without personal account or library authority", async () => {
      await recoveryCheck('device-library-seeded', async () => {
        insist(account, "Synthetic registration prerequisite failed"); await open(`/title/${manga}`); await follow("Reading"); localBefore = await until(device, value => value?.records.some(row => row.family === "series" && row.id === manga), "Device record was not persisted before recovery");
      });
      await recoveryCheck('logout-before-recovery', logout);
      const mail = await recoveryCheck('recovery-mail-delivered', async () => {
        await open("/auth/recovery"); await page.locator('input[name="email"]').fill(account.email); await page.getByRole("button", { name: "Send Recovery Link", exact: true }).click();
        return until(async () => { const response = await control(`/mail/latest?to=${encodeURIComponent(account.email)}&kind=recovery`); return response.ok() ? response.json() : undefined; }, value => value?.fixture_only && value.link, "Actual local recovery email was not delivered");
      });
      const link = await recoveryCheck('mail-link-local', async () => {
        const link = new URL(mail.link); insist(link.origin === origin, "Recovery email link left the exact local fixture"); oneTimeLink = link.href; redactions.add(link.href); for (const [, value] of link.searchParams) redactions.add(value); return link;
      });
      await recoveryCheck('reset-ui-mounted', async () => { await page.goto(link.href, { waitUntil: "domcontentloaded" }); await page.getByRole("button", { name: "Reset Password", exact: true }).waitFor({ state: "visible" }); });
      await recoveryCheck('reset-route', async () => { insist(new URL(page.url()).pathname === "/auth/recovery", "Recovery link redirected into inaccessible account-settings guards"); });
      await recoveryCheck('restricted-capability', async () => { const capability = await api("/auth/check"); insist(capability.status === 200 && capability.body?.session_purpose === "recovery" && capability.body?.recovery?.can_reset_password === true && capability.body.identity === undefined && capability.body.follows === undefined && capability.body.preferences === undefined && !capability.body.roles.length && !capability.body.permissions.length, "Recovery check exposed normal Session/personal authority"); });
      await recoveryCheck('password-only-controls', async () => { insist(await page.locator('input[name="email"],input[name="username"]').count() === 0 && await page.getByRole("link", { name: "Logout", exact: true }).count() === 0, "Reset-only page mounted normal account/profile controls"); });
      await recoveryCheck('private-library-denied', async () => { const privateRead = await api("/user/follows/manga"); insist(privateRead.status === 403, "Recovery credential read private account library"); });
      await recoveryCheck('device-library-retained', async () => { insist(JSON.stringify((await device()).records) === JSON.stringify(localBefore.records), "Recovery silently changed the separate device library"); });
      restrictedReady = true;
      return { actual_mail_link_redeemed: true, correct_mounted_route: true, password_only_ui: true, capability_not_session: true, private_library_403: true, device_records_unchanged: true };
    });
    await step("password-only CSRF form rotates session and only the new password authenticates", async () => {
      insist(restrictedReady, "The actual restricted recovery prerequisite did not pass; password reset was not attempted");
      newPassword = `Recovered!${randomBytes(24).toString("base64url")}9a`; redactions.add(newPassword);
      await recoveryCheck('password-reset-accepted', async () => { const response = await submitRecoveryPassword(page, newPassword); insist(response.status() === 200, "Real CSRF-bound password reset was rejected"); });
      await recoveryCheck('reset-completion-visible', async () => { await page.getByText("Password reset completed", { exact: true }).waitFor({ state: "visible" }); });
      await recoveryCheck('normal-session-confirmed', async () => { const check = await api("/auth/check"); insist(check.status === 200 && check.body?.identity?.username === account.username && check.body.session_purpose !== "recovery", "Password reset did not rotate into an actual normal account session"); });
      await recoveryCheck('account-settings-reached', async () => { await page.getByRole("button", { name: "Continue to Account Settings", exact: true }).click(); await page.waitForURL(url => url.pathname === "/settings/account"); });
      await recoveryCheck('old-password-rejected', async () => { await logout(); await login(account.password); await page.waitForTimeout(1000); insist((await api("/auth/check")).status === 401, "Old password remained valid after reset"); });
      await recoveryCheck('new-password-authenticated', async () => { await login(newPassword); await until(() => api("/auth/check"), value => value.body?.identity?.username === account.username, "New password could not establish a real session"); });
      await recoveryCheck('device-library-retained-after-reset', async () => { insist(JSON.stringify((await device()).records) === JSON.stringify(localBefore.records), "Password reset changed device library"); });
      await recoveryCheck('recovery-link-replay-rejected', async () => { const replay = await context.request.get(oneTimeLink); insist(replay.status() === 400, "One-time recovery link was reusable"); });
      return { genuine_settings_post_200: true, normal_session_rehydrated: true, old_password_rejected: true, new_password_accepted: true, one_time_link_consumed: true, local_library_retained: true };
    });
	} else if (phase === "upload") {
		await step("lost successful begin response is not replayed and owner staging reconciles", async () => {
			insist(account, "Registration prerequisite failed"); let begins = 0;
			const intercept = async route => {
				if (route.request().method() !== "POST") return route.continue();
				begins++; const response = await route.fetch(); insist(response.status() === 201, "Begin was not accepted before simulated response loss"); await route.abort("failed");
			};
			await context.route(origin + "/api/upload/begin", intercept);
			try {
				await open(`/title/${manga}/upload`);
				await page.getByRole("button", { name: /Not affiliated with any group/i }).click();
				await page.getByText("Request outcome unknown", { exact: false }).waitFor({ state: "visible" });
				await page.getByPlaceholder("Chapter Name", { exact: true }).waitFor({ state: "visible" });
				await page.waitForTimeout(2500); insist(begins === 1, "An ambiguous POST was replayed");
				const current = await api("/upload"); insist(current.status === 200 && current.body?.id, "Accepted staging was not restored");
				return { accepted_post_count: begins, honest_uncertainty_visible: true, owner_staging_restored: true };
			} finally { await context.unroute(origin + "/api/upload/begin", intercept); }
		});
		await step("corrupt image and corrupt archive release upload controls without fake success", async () => {
			const input = page.locator('input[type="file"][accept*="image/jpeg"]');
			await input.setInputFiles({ name: "broken.png", mimeType: "image/png", buffer: Buffer.from("not an image") });
			await page.getByText(/Failed to read|Failed to load|Invalid image|Could not decode/i).first().waitFor({ state: "visible" });
			await input.setInputFiles({ name: "broken.zip", mimeType: "application/zip", buffer: Buffer.from("not an archive") });
			await page.getByText("Failed to extract archive", { exact: false }).first().waitFor({ state: "visible" });
			const current = await api("/upload"); insist(current.body?.relationships?.files?.length === 0, "Rejected files became persisted pages");
			return { corrupt_image_rejected: true, corrupt_zip_rejected: true, persisted_files: 0 };
		});
		await step("single failed batch remains visibly recoverable and cancellation409 preserves staging", async () => {
      const original = await api("/upload"); insist(original.status === 200 && original.body.relationships.files.length === 0, "Expected an empty owned stage");
      const endpoint = origin + `/api/upload/${original.body.id}`; let posts = 0;
      const rejectPost = async route => { if (route.request().method() !== "POST") return route.continue(); posts++; await route.fulfill({ status: 500, contentType: "application/json", body: '{"error":"synthetic batch failure before acceptance"}' }); };
      await context.route(endpoint, rejectPost);
      try { await page.locator('input[type="file"][accept*="image/jpeg"]').setInputFiles({ name: "failed-single.png", mimeType: "image/png", buffer: image }); await page.getByText("Upload was not confirmed", { exact: true }).last().waitFor({ state: "visible" }); }
      finally { await context.unroute(endpoint, rejectPost); }
      const card = page.locator('div[class~="!visible"]').filter({ has: page.getByText("failed-single.png", { exact: true }) });
      insist(await card.getByRole("button").count() >= 2, "Failed single page has no per-item Replace/Remove controls");
      await card.getByRole("button").last().click(); await page.getByText("Page outcome is still unknown", { exact: true }).last().waitFor({ state: "visible" });
      insist(posts === 1 && (await api("/upload")).body.id === original.body.id, "Ambiguous page was replayed or staging discarded");
      const rejectCancel = async route => { if (route.request().method() === "DELETE") return route.fulfill({ status: 409, contentType: "application/json", body: '{"error":"synthetic immutable-stage rejection"}' }); return route.continue(); };
      await context.route(endpoint, rejectCancel);
      try { await confirm("Cancel staged upload"); await page.getByText("Upload session changed", { exact: true }).last().waitFor({ state: "visible" }); insist((await api("/upload")).body.id === original.body.id && await card.count() === 1, "Cancel409 discarded private staged state"); }
      finally { await context.unroute(endpoint, rejectCancel); }
      const cancelled = page.waitForResponse(response => response.url() === endpoint && response.request().method() === "DELETE"); await confirm("Cancel staged upload"); insist((await cancelled).status() === 204 && (await api("/upload")).status === 204, "Explicit successful cancellation was not persisted");
      await page.getByRole("button", { name: /Not affiliated with any group/i }).click(); await page.getByPlaceholder("Chapter Name", { exact: true }).waitFor();
      return { one_post_only: posts === 1, per_item_failure_controls: true, unknown_not_replayed: true, cancel409_retains_state: true, confirmed204_releases_stage_and_navigation: true };
    });
		await step("real image upload privately saves metadata and exact order then resumes on reload", async () => {
			await page.locator('input[type="file"][accept*="image/jpeg"]').setInputFiles([{ name: "page1.png", mimeType: "image/png", buffer: image }, { name: "page2.png", mimeType: "image/png", buffer: png(64, 96, marker + "next") }]);
			await page.getByPlaceholder("Chapter Name", { exact: true }).fill(`Staged ${marker}`);
			await page.getByPlaceholder("Volume No.", { exact: true }).fill("1"); await page.getByPlaceholder("Chapter No.", { exact: true }).fill("7");
			await page.getByText("Select Language", { exact: true }).click(); await page.getByText("English", { exact: true }).last().click();
			const saved = await until(() => api("/upload"), value => value.body?.relationships?.files?.length === 2 && value.body?.draft?.title === `Staged ${marker}` && value.body?.draft?.language === "en" && value.body?.page_order?.length === 2, "Private staging metadata/order did not autosave", 25000);
			await page.reload({ waitUntil: "domcontentloaded" });
			await until(() => page.getByPlaceholder("Chapter Name", { exact: true }).inputValue(), value => value === `Staged ${marker}`, "Reload lost chapter draft");
			const restored = await api("/upload"); insist(restored.body.id === saved.body.id && JSON.stringify(restored.body.page_order) === JSON.stringify(saved.body.page_order), "Reload replaced or reordered staged work");
			const previews = await page.locator('img[src*="/uploads/"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0).length);
			insist(previews >= 2, "Resumed private image previews did not decode");
			return { session_preserved: true, draft_title_restored: true, exact_order_restored: true, genuine_previews: previews };
		});
		await step("replacement rejects invalid images before mutation and reconciles one lost atomic response", async () => {
      const current = await api("/upload"), previous = current.body.relationships.files[0]; let writes = 0, deletes = 0;
      const replaceControl = () => page.locator('div[class~="!visible"]').filter({ has: page.getByText(previous.original_file_name, { exact: true }) }).getByRole('button').first();
      const watch = request => { const path = new URL(request.url()).pathname; if (path.includes(`/upload/${current.body.id}/files/`) && request.method() === "DELETE") deletes++; if (path.endsWith("/replace") && request.method() === "POST") writes++; };
      page.on("request", watch);
      try {
        const rejectedChooser = page.waitForEvent("filechooser"); await replaceControl().click(); await (await rejectedChooser).setFiles({ name: "corrupt-replacement.png", mimeType: "image/png", buffer: Buffer.from("not an image") });
        await page.getByText(/Existing page is retained/, { exact: false }).first().waitFor({ state: "visible" });
        const untouched = await api("/upload"); insist(untouched.body.relationships.files.find(row => row.id === previous.id)?.file_hash === previous.file_hash && writes === 0 && deletes === 0, "Replacement admission removed or changed the valid old page");
        const endpoint = origin + `/api/upload/${current.body.id}/files/${previous.id}/replace`;
        const loseReply = async route => { const response = await route.fetch(); insist(response.status() === 200, "Atomic replacement was rejected"); await route.abort("failed"); };
        await context.route(endpoint, loseReply);
        try { const chooser = page.waitForEvent("filechooser"); await replaceControl().click(); await (await chooser).setFiles({ name: "replacement.png", mimeType: "image/png", buffer: png(80, 100, marker + "replace") }); await page.getByText("Replacement confirmed from current staging", { exact: true }).waitFor({ state: "visible", timeout: 15000 }); }
        finally { await context.unroute(endpoint, loseReply); }
        const latest = await api("/upload"), actual = latest.body.relationships.files.find(row => row.id === previous.id);
        insist(actual && actual.file_hash !== previous.file_hash && actual.original_file_name === "replacement.png" && writes === 1 && deletes === 0 && JSON.stringify(latest.body.page_order) === JSON.stringify(untouched.body.page_order), "Atomic replacement changed identity/order or replayed its mutation");
        return { invalid_replacement_kept_old_bytes: true, atomic_post_count: writes, delete_requests: deletes, stable_file_identity_order: true, lost_reply_reconciled_by_read: true };
      } finally { page.off("request", watch); }
    });
		await step("commit actual image pages using form then read ordered published images", async () => {
			await checked("#terms", true);
			const responsePromise = page.waitForResponse(response => /\/api\/upload\/[^/]+\/commit$/.test(new URL(response.url()).pathname) && response.request().method() === "POST");
			await page.getByRole("button", { name: "Upload", exact: true }).click();
			const response = await responsePromise; insist(response.status() === 200, "Commit did not succeed"); uploaded = await response.json();
			insist(uploaded.publication_scope === "instance-local" && uploaded.publication_status === "local" && uploaded.data?.length === 2, "Local fixture publication receipt was not truthful");
			await page.getByText("Chapter uploaded to this instance", { exact: true }).waitFor({ state: "visible" }); await page.getByRole("button", { name: "Close", exact: true }).last().click();
			await page.waitForURL(url => url.pathname.startsWith(`/chapter/${uploaded.id}/`));
			await until(() => page.locator('img[src^="blob:"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0).length), value => value > 0, "Published reader did not decode genuine page bytes");
			return { committed_pages: uploaded.data.length, truthful_local_receipt: true, reader_opened_from_control: true, decoded_images: true };
		});
		await step("invalid dirty chapter metadata can Revert without a mutation or navigation trap", async () => {
      insist(uploaded?.id, "Uploaded chapter prerequisite failed"); await open(`/chapter/${uploaded.id}/edit`);
      const field = page.getByPlaceholder("Chapter No.", { exact: true }), original = await field.inputValue(); let writes = 0;
      const watch = request => { if (new URL(request.url()).pathname === `/api/chapter/${uploaded.id}` && request.method() === "PUT") writes++; }; page.on("request", watch);
      try { await field.fill("invalid-number"); const revert = page.getByRole("button", { name: "Revert", exact: true }); insist(await revert.isEnabled(), "Invalid dirty metadata blocks its local Revert control"); await revert.click(); insist(await field.inputValue() === original && writes === 0, "Revert wrote metadata or failed to restore initial value"); await page.locator(`a[href^="/title/${manga}"]`).first().click(); await page.waitForURL(url => url.pathname.startsWith(`/title/${manga}`)); }
      finally { page.off("request", watch); }
      return { invalid_local_value_reverted: true, no_put_sent: writes === 0, clean_spa_exit: true };
    });
		await step("Edit Pages and Replace Pages submit without native document navigation", async () => {
      insist(uploaded?.id, "Uploaded chapter prerequisite failed"); await open(`/chapter/${uploaded.id}/edit`); let navigations = 0, begins = 0;
      const watchFrame = frame => { if (frame === page.mainFrame()) navigations++; }, watchRequest = request => { if (new URL(request.url()).pathname === `/api/upload/begin/${uploaded.id}` && request.method() === "POST") begins++; };
      page.on("framenavigated", watchFrame); page.on("request", watchRequest);
      try {
        const titleField = page.getByPlaceholder("Chapter Name", { exact: true }); const originalTitle = await titleField.inputValue(); await titleField.fill(`Unacknowledged ${marker}`);
        await page.getByRole("button", { name: "Edit Pages", exact: true }).click(); await page.getByText("Save or revert chapter metadata first", { exact: true }).waitFor({ state: "visible" });
        insist(begins === 0 && navigations === 0 && await titleField.inputValue() !== originalTitle && (await api("/upload")).status === 204, "Dirty metadata was dropped or page staging began before Save/Revert");
        await page.getByRole("button", { name: "Revert", exact: true }).click(); insist(await titleField.inputValue() === originalTitle, "Displayed metadata did not revert before page admission");
        await page.getByRole("button", { name: "Edit Pages", exact: true }).click(); const edit = await until(() => api("/upload"), value => value.status === 200 && value.body.chapter_id === uploaded.id && value.body.relationships.files.length === 2, "Edit Pages did not restore actual ordered pages");
        insist(navigations === 0 && new URL(page.url()).pathname === `/chapter/${uploaded.id}/edit`, "Edit Pages caused a native form GET navigation");
        await page.getByRole("button", { name: "Cancel", exact: true }).first().click(); await until(() => api("/upload"), value => value.status === 204, "Edited-page session did not cancel");
        await page.getByRole("button", { name: "Replace Pages", exact: true }).click(); const replace = await until(() => api("/upload"), value => value.status === 200 && value.body.chapter_id === uploaded.id && value.body.relationships.files.length === 0, "Replace Pages did not create an empty owned staging session");
        insist(navigations === 0 && begins === 2 && replace.body.id !== edit.body.id, "Page editing navigated or replayed a begin mutation");
        await page.getByRole("button", { name: "Cancel", exact: true }).first().click(); await until(() => api("/upload"), value => value.status === 204, "Replacement staging did not cancel");
      } finally { page.off("framenavigated", watchFrame); page.off("request", watchRequest); }
      return { synchronous_default_prevention: true, native_document_navigations: navigations, actual_begin_requests: begins, edit_preserves_pages: true, replace_starts_empty: true };
    });
	} else if (phase === "reader") {
		await step("reader image failures expose real Retry and recover genuine decoded bytes", async () => {
			let blocked = 0; const intercept = async route => { blocked++; await route.fulfill({ status: 503, contentType: "application/json", body: '{"error":"synthetic temporary page failure"}' }); };
			await context.route(origin + `/data/${chapter}/**`, intercept);
			try { await open(`/chapter/${chapter}/1`); await page.getByText("Failed to load page 1", { exact: false }).waitFor({ state: "visible", timeout: 12000 }); }
			finally { await context.unroute(origin + `/data/${chapter}/**`, intercept); }
			await page.getByRole("button", { name: "Retry", exact: true }).first().click();
			await until(() => page.locator('img[src^="blob:"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0).length), value => value > 0, "Retry did not decode real server image");
			return { real_failed_state: true, failed_requests: blocked, retry_user_control: true, genuine_image_decoded: true };
		});
		await step("double-spread second-page deep link stays visible after reload and quality changes", async () => {
      await open(`/chapter/${chapter}/2`); await page.getByRole("button", { name: "Double", exact: true }).click();
      const visibleImages = () => page.locator('img[src^="blob:"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0 && image.getClientRects().length && getComputedStyle(image).visibility !== "hidden").length);
      await until(visibleImages, count => count > 0, "Selected spread's second member blanked reader");
      await page.reload({ waitUntil: "domcontentloaded" }); await until(visibleImages, count => count > 0, "Double-spread page2 became blank after reload"); insist(/\/2$/.test(new URL(page.url()).pathname), "Reload changed the selected one-based page");
      await page.getByRole("button", { name: "Optimized", exact: true }).click(); await until(visibleImages, count => count > 0, "Quality change blanked the selected spread");
      await page.getByRole("button", { name: "Single", exact: true }).click();
      return { selected_second_member_visible: true, ordinary_reload_retains_page2: true, quality_change_retains_images: true };
    });
		await step("same-title language change rejects an old pending reader aggregate", async () => {
      const alternate = ids.alternative_language_chapter; insist(alternate, "Real same-title alternative-language fixture is required");
      const seen = []; let release; const held = new Promise(resolve => { release = resolve; }); let holding = false;
      const intercept = async route => {
        const query = new URL(route.request().url()).searchParams, language = query.get("tlang"); seen.push(language);
        const response = await route.fetch();
        if (language === "en" && !holding) { holding = true; await held; }
        await route.fulfill({ response });
      };
      const endpoint = origin + `/api/manga/${manga}/aggregate*`; await context.route(endpoint, intercept);
      try {
        await open(`/chapter/${chapter}/1`);
        await page.locator(`a[href^="/title/${manga}"]`).first().click(); await page.locator(`a[href="/chapter/${alternate}"]`).first().click();
        await until(() => seen, rows => rows.includes("ja"), "SPA language change reused old aggregate instead of querying ja"); release();
        await page.waitForTimeout(1000); insist(new URL(page.url()).pathname.startsWith(`/chapter/${alternate}`), "Late English aggregate changed the selected Japanese chapter");
        const actual = await api(`/manga/${manga}/aggregate?tlang=ja`); insist(actual.status === 200 && actual.body, "Actual Japanese aggregate was unavailable");
        return { real_spa_title_language_switch: true, current_aggregate_language_ja: true, late_english_completion_ignored: true, observed_languages: seen };
      } finally { release(); await context.unroute(endpoint, intercept); }
    });
    await step("long-strip width slider refreshes actual tall-page geometry and progress", async () => {
      await open(`/chapter/${chapter}/1`); await page.getByRole("button", { name: "Single", exact: true }).click(); await page.getByRole("button", { name: "Long Strip", exact: true }).click(); await page.getByRole("button", { name: "Width", exact: true }).first().click();
      const slider = page.getByText(/^Maximum Page Width \(/).locator('..').getByRole("slider"); await slider.focus(); await page.keyboard.press("Home"); await page.waitForTimeout(250); await page.keyboard.press("End");
      const image = page.locator('img[src^="blob:"]').first(); await until(() => image.evaluate(node => ({ height: node.getBoundingClientRect().height, natural: node.naturalHeight, viewport: innerHeight })), value => value.natural >= 3600 && value.height > 3 * value.viewport, "Fixture did not display a genuinely tall image after width100");
      await page.locator('img[src^="blob:"]').nth(1).scrollIntoViewIfNeeded(); await page.mouse.wheel(0, 300);
      await until(() => new URL(page.url()).pathname, value => /\/2$/.test(value), "Tall-strip intersection never advanced selected page after geometry resize");
      return { real_600x3600_page: true, actual_slider_10_to_100: true, selected_page_advanced: true };
    });
    await step("held keyboard scrolling stops after reader destruction and blur", async () => {
      await open(`/chapter/${chapter}/1`); await page.getByRole("button", { name: "Long Strip", exact: true }).click(); await page.keyboard.down("ArrowDown"); await page.waitForTimeout(300);
      await page.locator(`a[href^="/title/${manga}"]`).first().click(); await page.waitForURL(url => url.pathname.startsWith(`/title/${manga}`));
      await page.waitForTimeout(500); const first = await page.evaluate(() => scrollY); await page.waitForTimeout(700); const second = await page.evaluate(() => scrollY); await page.keyboard.up("ArrowDown");
      insist(Math.abs(second - first) <= 2, "Reader keyboard RAF kept scrolling the subsequent title route");
      await open(`/chapter/${chapter}/1`); await page.getByRole("button", { name: "Long Strip", exact: true }).click(); await page.keyboard.down("ArrowDown"); await page.waitForTimeout(200); await page.evaluate(() => dispatchEvent(new Event("blur"))); await page.waitForTimeout(500); const blurred = await page.evaluate(() => scrollY); await page.waitForTimeout(700); const after = await page.evaluate(() => scrollY); await page.keyboard.up("ArrowDown");
      insist(Math.abs(after - blurred) <= 2, "Keyboard RAF survived actual blur notification"); return { navigation_without_keyup_released_frame: true, blur_released_frame: true };
    });
		await step("reader keyboard navigation and real image byte range retain correct order", async () => {
      await open(`/chapter/${chapter}/1`);
			await page.keyboard.press("ArrowRight"); await page.waitForURL(url => /\/2$/.test(url.pathname));
			insist((await page.title()).startsWith("Page 2:"), "Keyboard navigation did not select page 2");
			const resource = await api(`/chapter/${chapter}`); insist(resource.status === 200 && resource.body?.data?.length >= 3, "Reader fixture chapter missing ordered pages");
			const response = await context.request.get(origin + `/data/${chapter}/${resource.body.data[0].name}`, { headers: { Range: "bytes=0-7" } });
			insist(response.status() === 206 && (await response.body()).length === 8 && /^bytes 0-7\//.test(response.headers()["content-range"] ?? ""), "Actual original page byte range is incorrect");
			return { keyboard_page_2_confirmed: true, current_route: new URL(page.url()).pathname, real_range_206: true, range_bytes: 8 };
		});
	} else if (phase === "announcements") {
		let news;
		await step("staff creates private announcement with visible form and anonymous isolation", async () => {
			insist(account, "Registration prerequisite failed"); await open("/announcements/manage");
			await page.locator("#announcement-title").fill(`Browser announcement ${marker}`); await page.locator("#announcement-body").fill(`Synthetic persisted Markdown **${marker}**`);
			await page.getByRole("button", { name: "Save announcement", exact: true }).click();
			const feed = await until(() => api("/announcement?include_drafts=1&limit=100"), value => value.body?.data?.some(item => item.title === `Browser announcement ${marker}`), "Staff draft was not persisted"); news = feed.body.data.find(item => item.title === `Browser announcement ${marker}`);
			const anonymous = await context.browser().newContext({ serviceWorkers: "block" }); try { const response = await anonymous.request.get(origin + `/api/announcement/${news.id}`); insist(response.status() === 404, "Anonymous reader saw unpublished draft"); } finally { await anonymous.close(); }
			return { staff_ui_created: true, unpublished_persisted: true, anonymous_detail_404: true };
		});
		await step("staff edits publishes pins and deletes exact versioned announcement", async () => {
			insist(news, "Announcement creation prerequisite failed"); const card = page.locator('[data-slot="card"]').filter({ has: page.getByRole("link", { name: news.title, exact: true }) }); await card.getByRole("button", { name: "Edit", exact: true }).click();
			await page.locator("#announcement-title").fill(`Published ${marker}`); await checked("#announcement-published", true); await checked("#announcement-pinned", true); await page.getByRole("button", { name: "Save announcement", exact: true }).click();
			const published = await until(() => api(`/announcement/${news.id}`), value => value.body?.published && value.body?.pinned && value.body?.version > news.version, "Announcement edit/publish did not persist");
			await open("/"); await page.getByRole("link", { name: `Published ${marker}`, exact: true }).waitFor({ state: "visible" }); await page.getByRole("link", { name: `Published ${marker}`, exact: true }).click(); await page.getByText(`Synthetic persisted Markdown ${marker}`, { exact: false }).waitFor({ state: "visible" });
			await open("/announcements/manage"); const changedCard = page.locator('[data-slot="card"]').filter({ has: page.getByRole("link", { name: `Published ${marker}`, exact: true }) }); page.once("dialog", dialog => dialog.accept()); await changedCard.getByRole("button", { name: "Delete", exact: true }).click();
			await until(() => api(`/announcement/${news.id}`), value => value.status === 404, "Delete control did not hide announcement");
			return { version_advanced: published.body.version > news.version, published_homepage_link_clicked: true, real_detail_rendered: true, delete_control_persisted: true };
		});
	} else if (phase === 'filters') {
		await step('empty explicit ratings emit sentinel and genuinely return zero results', async () => {
			await open('/search'); for (const rating of ['Safe', 'Suggestive', 'Erotica', 'Pornographic']) { await page.getByRole('combobox', { name: /Safe|Suggestive|Erotica|Pornographic|Content Rating/ }).click(); const option = page.getByRole('option', { name: rating, exact: true }); if (await option.getAttribute('aria-selected') === 'true') await option.click(); await page.keyboard.press('Escape'); }
			const result = page.waitForResponse(response => new URL(response.url()).pathname === '/api/manga' && new URL(response.url()).searchParams.get('contentRating') === '__none__'); await page.getByRole('button', { name: 'Search', exact: true }).click(); const response = await result; const feed = await response.json(); insist(response.status() === 200 && feed.total === 0 && feed.data.length === 0, 'Empty rating selection was silently treated as default ratings');
			await page.getByText('No results found', { exact: true }).waitFor({ state: 'visible' }); return { explicit_empty_wire: '__none__', actual_total: feed.total, empty_results_ui: true };
		});
		await step('safe rating title and has-chapters controls produce real nonempty narrowed feed', async () => {
			const seeded = await api(`/manga/${manga}`); insist(seeded.status === 200 && seeded.body.content_rating === 'safe', 'Seeded safe title missing'); await page.getByRole('combobox', { name: 'Content Rating', exact: true }).click(); await page.getByRole('option', { name: 'Safe', exact: true }).click(); await page.keyboard.press('Escape'); await page.getByPlaceholder('Search...', { exact: true }).fill(seeded.body.title); await checked('#hasChapters', true);
			const result = page.waitForResponse(response => new URL(response.url()).pathname === '/api/manga' && new URL(response.url()).searchParams.get('contentRating') === 'safe' && new URL(response.url()).searchParams.get('hasChapters') === 'true'); await page.getByRole('button', { name: 'Search', exact: true }).click(); const response = await result, feed = await response.json(); insist(response.status() === 200 && feed.total >= 1 && feed.data.some(row => row.id === manga) && feed.data.every(row => row.content_rating === 'safe'), 'Selected filters did not constrain real feed'); await page.locator(`a[href^="/title/${manga}"]`).first().waitFor({ state: 'visible' });
			return { rating_filter_applied: true, title_filter_applied: true, has_chapters_applied: true, visible_seed_result: true, total: feed.total };
		});
		await step('reset and back-forward use initialized query semantics without double question marks', async () => {
			const filtered = page.url(); await page.getByRole('button', { name: 'Reset', exact: true }).click(); await page.waitForTimeout(800); insist(!page.url().includes('??'), 'Filter query gained duplicate question mark'); await page.goBack({ waitUntil: 'domcontentloaded' }); insist(page.url() === filtered, 'Back did not restore filter route'); await page.goForward({ waitUntil: 'domcontentloaded' }); insist(!page.url().includes('??'), 'Forward query malformed'); return { reset_control: true, back_forward_routes: true, initialized_query_prefix_correct: true };
		});
	} else if (phase === 'editorial') {
		let title;
		await step('staff creates real title draft with author artist and original cover controls', async () => {
			insist(account, 'Staff registration prerequisite failed'); const author = await api(`/author/${ids.author ?? 'SeedAuthr1'}`); insist(author.status === 200 && author.body?.name, 'Real seeded author missing');
			await open('/create/title'); await page.locator('#title').fill(`Browser Draft ${marker}`); await page.locator('#synopsis').fill(`Synthetic editorial draft ${marker}`); await page.getByRole('tab', { name: 'Metadata', exact: true }).click();
			for (let index = 0; index < 2; index++) { await page.getByText('Search...', { exact: true }).nth(index).click(); const dialog = page.getByRole('dialog'); await dialog.getByPlaceholder('Search...', { exact: true }).fill(author.body.name); await dialog.getByRole('button', { name: author.body.name, exact: true }).click(); await page.keyboard.press('Escape'); }
			await page.getByRole('tab', { name: 'Covers', exact: true }).click(); await page.locator('input[type="file"][accept*="image/jpeg"]').setInputFiles({ name: 'cover.png', mimeType: 'image/png', buffer: image });
			const created = page.waitForResponse(response => new URL(response.url()).pathname === '/api/manga' && response.request().method() === 'POST'); await page.getByRole('button', { name: 'Save', exact: true }).click(); const response = await created; insist(response.status() === 200, 'Title creation did not persist'); title = await response.json();
			await page.getByText('Draft successfully created', { exact: true }).waitFor({ state: 'visible' }); const privateDraft = await api(`/manga/draft/${title.id}`); insist(privateDraft.status === 200 && privateDraft.body.state === 'draft', 'Created draft not privately readable'); const covers = await api(`/cover?manga=${title.id}`); insist(covers.status === 200, 'Original cover was not saved');
			await page.getByRole('link', { name: 'Edit draft', exact: true }).click(); await page.locator('#title').waitFor({ state: 'visible' });
			return { create_form_persisted: true, author_and_artist_controls_used: true, actual_png_cover_saved: true, original_editor_opened: true };
		});
		await step('submit reject revise submit preserve actual authoritative transition versions', async () => {
			insist(title, 'Draft prerequisite failed'); const first = await api(`/manga/draft/${title.id}`); await confirm('Submit'); const submitted = await until(() => api(`/manga/draft/${title.id}`), value => value.body?.state === 'submitted' && value.body.version > first.body.version, 'Submit did not advance authoritative version');
			await confirm('Reject'); const rejected = await until(() => api(`/manga/draft/${title.id}`), value => value.body?.state === 'rejected' && value.body.version > submitted.body.version, 'Reject did not advance version');
			await page.locator('#title').fill(`Revised ${marker}`); await page.getByRole('button', { name: 'Save', exact: true }).click(); await until(() => api(`/manga/draft/${title.id}`), value => value.body?.title === `Revised ${marker}` && value.body.version > rejected.body.version, 'Revision did not persist after rejection');
			await confirm('Submit'); const again = await until(() => api(`/manga/draft/${title.id}`), value => value.body?.state === 'submitted', 'Second submit failed');
			return { submitted_version: submitted.body.version, rejected_version: rejected.body.version, revised_and_resubmitted_version: again.body.version };
		});
		await step('publish local scope and repeated lock unlock controls refresh actual versions', async () => {
			insist(title, 'Draft prerequisite failed'); const receiptPromise = page.waitForResponse(response => new URL(response.url()).pathname === `/api/manga/draft/${title.id}/publish` && response.request().method() === 'POST'); await confirm('Publish'); const response = await receiptPromise; insist(response.status() === 200, 'Publish was not acknowledged'); const receipt = await response.json(); insist(receipt.publication_scope === 'instance-local' && receipt.publication_status === 'local', 'Native-only fixture publish scope is dishonest');
			const published = await until(() => api(`/manga/${title.id}`), value => value.body?.state === 'published', 'Published title not genuinely publicly readable'); await confirm('Lock'); const locked = await until(() => api(`/manga/${title.id}`), value => value.body?.locked === true && value.body.version > published.body.version, 'Lock did not persist actual version'); await confirm('Unlock'); const unlocked = await until(() => api(`/manga/${title.id}`), value => value.body?.locked === false && value.body.version > locked.body.version, 'Unlock did not persist after lock');
			return { scope: receipt.publication_scope, publication_publicly_readable: true, lock_version: locked.body.version, unlock_version: unlocked.body.version, real_signed_pending_receipt_requires_original_fixture: true };
		});
	} else if (phase === 'community') {
		let reply, group, claim;
		const thread = ids.thread ?? 'SeedThred1';
		await step('real comment create edit and persisted reload controls', async () => {
			insist(account, 'Moderator registration prerequisite failed'); await open(`/forums/threads/${thread}`); await page.getByPlaceholder('Enter your thoughts...', { exact: true }).fill(`Browser comment ${marker}`); const posted = page.waitForResponse(response => new URL(response.url()).pathname === `/api/thread/${thread}/replies` && response.request().method() === 'POST'); await page.getByRole('button', { name: 'Post', exact: true }).click(); const response = await posted; insist(response.status() === 201, 'Comment POST not accepted'); reply = await response.json(); await page.locator(`#p${reply.id}`).getByText(`Browser comment ${marker}`, { exact: true }).waitFor({ state: 'visible' });
			await page.locator(`#p${reply.id}`).getByRole('button', { name: 'Edit', exact: true }).click(); const dialog = page.getByRole('dialog'); await dialog.getByPlaceholder('Enter your thoughts...', { exact: true }).fill(`Edited ${marker}`); await dialog.getByRole('button', { name: 'Save', exact: true }).click(); const updated = await until(() => api(`/thread/${thread}/replies/${reply.id}`), value => value.body?.body === `Edited ${marker}` && value.body.version > reply.version, 'Comment edit did not advance persisted version'); await page.reload({ waitUntil: 'domcontentloaded' }); await page.locator(`#p${reply.id}`).getByText(`Edited ${marker}`, { exact: true }).waitFor({ state: 'visible' });
			return { reply_created: true, edited_version: updated.body.version, reload_persisted: true };
		});
		await step('real group creation and ownership claim stays pending before moderation', async () => {
			await open('/create/group'); await page.locator('#name').fill(`Browser Group ${marker}`); const created = page.waitForResponse(response => new URL(response.url()).pathname === '/api/group' && response.request().method() === 'POST'); await page.getByRole('button', { name: 'Save', exact: true }).click(); const response = await created; insist(response.status() === 201, 'Group creation failed'); group = await response.json(); await page.waitForURL(url => url.pathname.startsWith(`/group/${group.id}`)); await page.getByRole('button', { name: 'Claim', exact: true }).click(); const dialog = page.getByRole('dialog'); await dialog.getByRole('radio', { name: 'Translated Chapter (Credits Page)', exact: true }).click(); await dialog.locator('#translated-chapter').fill(`${origin}/chapter/${chapter}`); await dialog.locator('#notes').fill(`Synthetic claim proof ${marker}`); await dialog.getByRole('button', { name: 'Save', exact: true }).click(); await page.getByText('Group claim request submitted successfully', { exact: true }).waitFor({ state: 'visible' });
			const feed = await until(() => api(`/report?object=${group.id}&limit=100`), value => value.body?.data?.some(row => row.relationships?.reason?.reason === 'Group claim request'), 'Claim report was not persisted'); claim = feed.body.data.find(row => row.relationships?.reason?.reason === 'Group claim request'); insist(claim.status === 'waiting', 'Claim silently bypassed moderator approval'); const current = await api(`/group/${group.id}`); insist(!(current.body?.relationships?.members ?? []).some(member => member.is_leader), 'Creating/submitting claim granted ownership');
			return { new_group_persisted: true, claim_form_persisted: true, pending_status: claim.status, no_automatic_leadership: true };
		});
		await step('moderator accepts exact claim using report control and actual leadership effect', async () => {
			insist(claim && group, 'Claim prerequisite failed'); await open(`/my/reports?object=${group.id}`); const article = page.locator('article').filter({ has: page.getByText(`#${claim.id}`, { exact: true }) }); await article.getByRole('button', { name: 'Show report information', exact: true }).click(); const information = page.getByRole('dialog'); await information.getByRole('button', { name: 'Accept', exact: true }).click(); const confirmation = page.getByRole('dialog').filter({ has: page.getByText('Accept Report', { exact: true }) }); await confirmation.getByPlaceholder('Reason (Optional)', { exact: true }).fill(`Synthetic reviewed claim ${marker}`); await confirmation.getByRole('button', { name: 'Accept', exact: true }).click();
			const accepted = await until(() => api(`/report?object=${group.id}&limit=100`), value => value.body?.data?.some(row => row.id === claim.id && row.status === 'accepted'), 'Report acceptance did not persist'); const members = await until(() => api(`/group/${group.id}`), value => value.body?.relationships?.members?.some(member => member.is_leader), 'Actual accepted claim did not grant leadership');
			return { accepted_report_confirmed: true, explicit_moderator_control_used: true, persisted_leader_present: members.body.relationships.members.some(member => member.is_leader) };
		});
		await step('comment deletion hides body without pretending hard peer deletion', async () => {
			insist(reply, 'Reply prerequisite failed'); await open(`/forums/threads/${thread}`); await page.locator(`#p${reply.id}`).getByRole('button', { name: 'Delete', exact: true }).click(); await page.getByRole('dialog').getByRole('button', { name: 'Delete', exact: true }).click(); const removed = await until(() => api(`/thread/${thread}/replies/${reply.id}`), value => value.status === 404 || value.body?.state?.startsWith('deleted'), 'Soft comment deletion not persisted'); await page.reload({ waitUntil: 'domcontentloaded' }); insist(await page.getByText(`Edited ${marker}`, { exact: true }).count() === 0, 'Deleted comment body remained visible'); return { removal_status: removed.status, original_body_not_rendered: true };
		});
	} else if (phase === 'local-library') {
		const check = (id, operation) => {
			insist(typeof h.check === 'function', 'The reviewed fixed-check diagnostic recorder is required for this local-library journey');
			return h.check(id, operation);
		};
		const inspect = (suffix, operation) => check('local-library-' + suffix, operation);
		let snapshot, backup;
		await step('anonymous default device follow persists without account or automatic publication', async () => {
			await inspect('device-title-navigation', () => open(`/title/${manga}`)); await selectTitleLibraryStatus(page, 'Reading', { check, prefix: 'local-library-device-follow' });
			snapshot = await inspect('device-status-durable', () => until(device, value => value?.records.some(row => row.family === 'series' && row.id === manga && row.status === 'reading'), 'Anonymous follow was not saved in IndexedDB'));
			await inspect('device-authority-unpublished', async () => {
				insist((await api('/auth/check')).status !== 200, 'Anonymous journey unexpectedly has an account');
				insist(snapshot.scope.origin === origin && snapshot.scope.catalogNamespace, 'Device library is missing instance provenance');
				insist(await outboxCount() === 0, 'A local save automatically queued public transmission');
			});
			await inspect('device-reload-status', async () => { await page.reload({ waitUntil: 'domcontentloaded' }); await titleLibraryTrigger(page, 'Reading').waitFor({ state: 'visible' }); });
			await inspect('device-library-navigation', async () => { await open('/library'); await page.locator(`a[href^="/title/${manga}"]`).first().waitFor({ state: 'visible' }); });
			return { anonymous_saved: true, scoped_device_persistence: true, reload_and_library_control: true, outbox_jobs: 0 };
		});
		await step('real reader progress persists locally and survives password identity changes', async () => {
			await open(`/chapter/${chapter}/1`); await until(() => page.locator('img[src^="blob:"]').evaluateAll(images => images.filter(image => image.complete && image.naturalWidth > 0).length), count => count > 0, 'Reader did not decode actual bytes');
			await page.keyboard.press('ArrowRight'); await page.waitForURL(url => /\/2$/.test(url.pathname));
			const progress = await until(device, value => value?.records.some(row => row.family === 'chapters' && row.id === chapter && row.page >= 1), 'Reader page control did not persist device progress');
			const before = progress.records.map(row => row.family + ':' + row.key).sort();
			await signup('user'); const after = await device(); insist(before.every(key => after.records.some(row => row.family + ':' + row.key === key)), 'Password signup replaced device library');
			await expandPanel(); await panel().getByText('Library source: this browser device.', { exact: false }).waitFor({ state: 'visible' });
			await logout(); const loggedOut = await device(); insist(before.every(key => loggedOut.records.some(row => row.family + ':' + row.key === key)), 'Logout erased the independent device library');
			return { reader_progress_saved: true, signup_did_not_select_account_mode: true, logout_preserved_device_records: true };
		});
		await step('custom list controls and reviewed scoped backup export/import persist actual device rows', async () => {
			const target = panel();
			await inspect('custom-list-navigation', async () => { await open('/library'); await expandPanel(); await target.getByText('Custom device lists', { exact: true }).click(); });
			const list = await inspect('custom-list-durable', async () => {
				await target.getByLabel('List name', { exact: true }).fill(`Local ${marker}`); await target.getByRole('button', { name: 'Create list', exact: true }).click();
				const listed = await until(device, value => value?.records.some(row => row.family === 'lists' && row.name === `Local ${marker}`), 'List control did not persist'); return listed.records.find(row => row.family === 'lists' && row.name === `Local ${marker}`);
			});
			await inspect('custom-member-durable', async () => {
				await target.getByLabel('List', { exact: true }).selectOption(list.id); await target.getByLabel('Series', { exact: true }).selectOption(manga); await target.getByLabel('Position', { exact: true }).fill('3'); await target.getByRole('button', { name: 'Save member', exact: true }).click();
				await until(device, value => value?.records.some(row => row.family === 'members' && row.list === list.id && row.series === manga && row.position === 3), 'Membership control did not persist');
			});
			await inspect('custom-backup-export', async () => {
				const downloading = page.waitForEvent('download'); await target.getByRole('button', { name: 'Export device backup', exact: true }).click(); const download = await downloading; const stream = await download.createReadStream(); const chunks = []; for await (const chunk of stream) chunks.push(chunk); backup = JSON.parse(Buffer.concat(chunks).toString('utf8'));
				insist(backup.format === 'weebdex-device-library' && backup.scope.origin === origin && backup.records.length >= 4, 'UI export was not the scoped device backup');
			});
			await inspect('custom-foreign-backup-fenced', async () => { const foreign = { ...backup, scope: { ...backup.scope, origin: 'https://foreign.example.invalid' } }; await target.getByLabel('Preview device backup', { exact: true }).setInputFiles({ name: 'foreign.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(foreign)) }); await target.getByText('Native IDs belong to a different instance.', { exact: false }).waitFor({ state: 'visible' }); insist(await target.getByRole('button', { name: 'Merge reviewed import', exact: true }).count() === 0, 'Foreign native IDs could be merged silently'); });
			await inspect('custom-backup-merged', async () => { await target.getByRole('button', { name: 'Cancel import', exact: true }).click(); await target.getByLabel('Preview device backup', { exact: true }).setInputFiles({ name: 'same-instance.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(backup)) }); await target.getByRole('button', { name: 'Merge reviewed import', exact: true }).click(); await target.getByText('Backup merged locally.', { exact: true }).waitFor({ state: 'visible' });
				insist(await outboxCount() === 0, 'Backup/list controls automatically published'); });
			return { list_membership_persisted: true, actual_download_reviewed: true, foreign_scope_blocked: true, explicit_merge_succeeded: true, public_outbox_unchanged: true };
		});
		await step('explicit account mode and account import preserve independent device state', async () => {
			const before = await inspect('account-mode-ready', async () => { insist(account, 'Synthetic account prerequisite failed'); await login(account.password); await until(() => api('/auth/check'), value => value.status === 200, 'Login failed'); await open(`/title/${manga}`); await expandPanel(); const saved = await device();
				await panel().getByRole('button', { name: 'Use account library', exact: true }).click(); return saved; });
			await selectTitleLibraryStatus(page, 'Plan to Read', { check, prefix: 'local-library-account-follow' });
			const accountFollow = await inspect('account-status-durable', () => until(() => api(`/user/follows/manga?limit=100`), value => value.status === 200 && value.body?.data?.some(row => row.id === manga && row.reading_status === 'plan_to_read'), 'Explicit account follow did not persist'));
			await inspect('account-device-preserved', async () => { const preserved = await device(); insist(JSON.stringify(preserved.records) === JSON.stringify(before.records), 'Account mode silently overwrote device records'); });
			await inspect('account-import-reviewed', async () => { await panel().getByRole('button', { name: 'Preview account import to device', exact: true }).click(); await panel().getByText('Import preview: account', { exact: false }).waitFor({ state: 'visible' }); await panel().getByRole('button', { name: 'Merge reviewed import', exact: true }).click();
				await panel().getByRole('button', { name: 'Use device library', exact: true }).click(); await until(device, value => value?.records.some(row => row.family === 'series' && row.id === manga), 'Reviewed account import lost local series'); });
			return { account_follow_confirmed: accountFollow.status, explicit_mode_only: true, device_unchanged_before_reviewed_import: true, reviewed_merge_control_used: true };
		});
		await step('optional signer requires explicit selection and dismissal does not queue publication', async () => {
			const target = panel();
			await inspect('signer-section-open', async () => { await expandPanel(); await target.getByText('Optional Nostr publication and verified recovery', { exact: true }).click(); });
			await inspect('signer-binding-installed', () => page.evaluate(() => { window.nostr = { getPublicKey: async () => { throw Error('Synthetic signer prompt dismissed'); }, signEvent: async () => { throw Error('Dismissed signer must never sign'); } }; }));
			await inspect('signer-dismissal-visible', async () => { await target.getByRole('button', { name: 'Connect external browser signer', exact: true }).click(); await target.getByText('Synthetic signer prompt dismissed', { exact: false }).waitFor({ state: 'visible' }); });
			await inspect('signer-outbox-empty', async () => { insist(await outboxCount() === 0, 'Dismissed signer created publication jobs'); await page.evaluate(() => { delete window.nostr; }); });
			return { explicit_signer_control: true, dismissed_prompt_truthful: true, no_job_or_auto_key: true, signed_delivery_recovery_requires_real_original_fixture: true };
		});
	} else if (phase === "pwa") {
		await step("installed service worker purges exact poisoned legacy caches without local database loss", async () => {
			// Start on a same-origin text resource so no app script registers the worker yet.
			await page.goto(origin + "/robots.txt");
			await page.evaluate(async () => {
				localStorage.setItem("browser-acceptance-preserve", "local-device-data");
				for (const name of ["covers", "favicons"]) { const cache = await caches.open(name); await cache.put("/covers/favicons00/never-public.png", new Response("synthetic private stale marker", { headers: { "Content-Type": "text/plain" } })); }
			});
			await open("/"); await page.waitForFunction(() => !!navigator.serviceWorker.controller, undefined, { timeout: 18000 });
			const state = await page.evaluate(async () => ({ stale: await Promise.all(["covers", "favicons"].map(async name => !!(await (await caches.open(name)).match("/covers/favicons00/never-public.png")))), local: localStorage.getItem("browser-acceptance-preserve"), script: navigator.serviceWorker.controller.scriptURL }));
			insist(state.stale.every(value => !value) && state.local === "local-device-data", "Activation retained old private cache or deleted unrelated local data");
			return { actual_worker_controller: true, exact_legacy_entries_removed: true, unrelated_device_storage_retained: true, worker_path: new URL(state.script).pathname };
		});
		await step("authority routes win adversarial favicon IDs/query and never fall back offline", async () => {
			const paths = ["/covers/favicons00/never-public.png", "/covers/SeedManga1/no-such.png?probe=faviconV2", "/api/user/favicons00/lists?probe=faviconV2", "/uploads/never-public.png?probe=/favicons", "/originals/" + "a".repeat(64) + "?probe=faviconV2"];
			await page.evaluate(async paths => { for (const name of ["covers", "favicons"]) { const cache = await caches.open(name); for (const url of paths) await cache.put(url, new Response("synthetic private stale marker", { status: 200 })); } }, paths);
			const live = await page.evaluate(async paths => Promise.all(paths.map(async url => { const response = await fetch(url); return { path: url.split("?")[0], status: response.status, stale: (await response.text()).includes("synthetic private stale marker") }; })), paths);
			insist(live.every(value => value.status !== 200 && !value.stale), "Authority request was answered by a broad persistent cache route");
			await context.setOffline(true);
			try { const leaked = await page.evaluate(async url => { try { return (await (await fetch(url)).text()).includes("synthetic private stale marker"); } catch { return false; } }, paths[0]); insist(!leaked, "Offline request leaked stale authority data"); } finally { await context.setOffline(false); }
			await page.reload({ waitUntil: "domcontentloaded" }); await page.getByRole("link", { name: "Advanced Search", exact: true }).waitFor({ state: "visible" });
			return { warmed_authority_urls: live, offline_stale_fallback_absent: true, ordinary_sw_enabled_reload_bootstrapped: true };
		});
	} else if (!["account", "account-recovery", "upload", "reader", "announcements", "local-library", "filters", "editorial", "community", "pwa"].includes(phase)) throw new Error(`Journey phase still awaiting dedicated control specification: ${phase}`);
}
function png(width, height, seed) {
	const crc = bytes => { let value = 0xffffffff; for (const byte of bytes) { value ^= byte; for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ ((value & 1) ? 0xedb88320 : 0); } return (value ^ 0xffffffff) >>> 0; };
	const chunk = (kind, data) => { const name = Buffer.from(kind), length = Buffer.alloc(4), sum = Buffer.alloc(4); length.writeUInt32BE(data.length); sum.writeUInt32BE(crc(Buffer.concat([name, data]))); return Buffer.concat([length, name, data, sum]); };
	const header = Buffer.alloc(13); header.writeUInt32BE(width); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 2;
	const raw = Buffer.alloc(height * (width * 3 + 1)); const colour = Buffer.from(seed);
	for (let y = 0; y < height; y++) for (let x = 0; x < width * 3; x++) raw[y * (width * 3 + 1) + 1 + x] = (colour[x % colour.length] + x + y) % 256;
	return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]), chunk("IHDR", header), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}
