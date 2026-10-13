const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
(async () => {
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true,
  });
  try {
    const context = await browser.newContext({
      viewport: { width: 1100, height: 850 },
      acceptDownloads: true,
    });
    const external = [],
      errors = [];
    await context.addInitScript(() => {
      window.signerCalls = 0;
      window.nostr = {
        getPublicKey: async () => {
          window.signerCalls++;
          throw Error("Must not connect automatically");
        },
        signEvent: async () => {
          window.signerCalls++;
          throw Error("Must not sign automatically");
        },
      };
    });
    context.on("request", (r) => {
      if (new URL(r.url()).origin !== "http://127.0.0.1:19795")
        external.push(r.url());
    });
    const page = await context.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("dialog", (d) => d.accept());
    await page.goto("http://127.0.0.1:19795");
    await page.getByRole("status").filter({ hasText: "Ready." }).waitFor();
    async function save(p, id, n) {
      await p.locator("#chapter-id").fill(id);
      await p.locator("#chapter-page").fill(String(n));
      await p.getByRole("button", { name: "Save on this device" }).click();
      await p
        .getByRole("status")
        .filter({ hasText: "Progress saved locally" })
        .waitFor();
    }
    await save(page, "01".repeat(32), 3);
    await page.reload();
    await page.getByRole("status").filter({ hasText: "Ready." }).waitFor();
    assert.match(await page.locator("#records").innerText(), /page 3/);
    const tab = await context.newPage();
    await tab.goto("http://127.0.0.1:19795");
    await tab.getByRole("status").filter({ hasText: "Ready." }).waitFor();
    await save(tab, "02".repeat(32), 7);
    await page.locator("#counts").filter({ hasText: "2 chapters" }).waitFor();
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    await page.getByRole("status").filter({ hasText: "Ready." }).waitFor();
    await context.setOffline(true);
    await page.reload();
    await page.getByRole("status").filter({ hasText: "Ready." }).waitFor();
    await save(page, "01".repeat(32), 9);
    await context.setOffline(false);
    const downloadPromise = page.waitForEvent("download");
    await page.locator("#export").click();
    const download = await downloadPromise;
    const lines = (await fs.readFile(await download.path(), "utf8"))
      .trimEnd()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.equal(lines[0].version, 2);
    const backup = {
      format: "manga-local-library",
      version: 1,
      records: { chapters: [], series: [], lists: [], members: [] },
    };
    for (const [family, record] of lines.slice(1, -1))
      backup.records[family].push(record);
    assert.equal(
      lines.at(-1).end,
      Object.values(backup.records).reduce((n, rows) => n + rows.length, 0),
    );
    assert.equal(backup.records.chapters.length, 2);
    assert.equal(
      backup.records.chapters.find((r) => r.id === "01".repeat(32)).page,
      9,
    );
    await page.locator("#import").setInputFiles({
      name: "invalid.json",
      mimeType: "application/json",
      buffer: Buffer.from(
        JSON.stringify({
          ...backup,
          records: {
            ...backup.records,
            chapters: [backup.records.chapters[0], backup.records.chapters[0]],
          },
        }),
      ),
    });
    await page
      .getByRole("status")
      .filter({ hasText: "Duplicate import record" })
      .waitFor();
    assert.match(await page.locator("#counts").innerText(), /2 chapters/);
    await save(page, "01".repeat(32), 12);
    await page.locator("#import").setInputFiles(await download.path());
    await page
      .getByRole("status")
      .filter({ hasText: "Backup imported locally" })
      .waitFor();
    assert.match(await page.locator("#records").innerText(), /page 9/);
    const stale = await page.evaluate(async () => {
      const { LocalLibrary } = await import("./src/store.js");
      const s = await LocalLibrary.open();
      const snapshot = await s.snapshot();
      await s.write("chapters", {
        id: "03".repeat(32),
        completed: false,
        page: 1,
      });
      try {
        await s.replace(snapshot, snapshot.revision);
        return false;
      } catch (e) {
        return e.message.includes("changed");
      } finally {
        s.close();
      }
    });
    assert.ok(stale);
    await page.evaluate(async () => {
      const { LocalLibrary } = await import("./src/store.js");
      const s = await LocalLibrary.open();
      await s.setRelays([
        {
          origin: "https://never-contact.example",
          serviceKey: "11".repeat(32),
          selected: true,
        },
      ]);
      s.close();
    });
    await page.reload();
    await page.getByRole("status").filter({ hasText: "Ready." }).waitFor();
    assert.equal(await page.evaluate(() => window.signerCalls), 0);
    assert.deepEqual(external, []);
    await page.setViewportSize({ width: 390, height: 844 });
    assert.ok(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    );
    await page.screenshot({
      path: path.resolve("../results/rebuild-client/browser-client-mobile.png"),
      fullPage: true,
    });
    await page.setViewportSize({ width: 1100, height: 850 });
    await page.screenshot({
      path: path.resolve(
        "../results/rebuild-client/browser-client-desktop.png",
      ),
      fullPage: true,
    });
    assert.deepEqual(errors, []);
    await fs.writeFile(
      "../results/rebuild-client/browser-client-local.json",
      JSON.stringify(
        {
          passed: true,
          browser: "local cloud Chromium",
          checks: [
            "local save without signer/network",
            "IndexedDB reload persistence",
            "two-tab changes visible",
            "offline reload and local save",
            "backup export contents",
            "duplicate import rejected atomically",
            "stale replacement refuses newer local edits",
            "saved relay does not trigger automatic traffic",
            "no automatic signer invocation",
            "mobile no horizontal overflow",
          ],
          externalRequests: external,
          pageErrors: errors,
        },
        null,
        2,
      ),
    );
    console.log("Browser local-first checks passed");
    await context.close();
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
