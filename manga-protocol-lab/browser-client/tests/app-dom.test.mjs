import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import { JSDOM, VirtualConsole } from "jsdom";
import { IDBFactory, IDBKeyRange } from "fake-indexeddb";
import { LocalLibrary } from "../src/store.js";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn) {
  for (let i = 0; i < 200; i++) {
    if (fn()) return;
    await sleep(5);
  }
  assert.ok(fn(), "UI state did not arrive");
}
test("actual interface local flows work in DOM model without account/network", async () => {
  const html = await fs.readFile(
      new URL("../index.html", import.meta.url),
      "utf8",
    ),
    logs = [],
    checks = [],
    channels = [];
  let fetchCalls = 0,
    signerCalls = 0,
    confirmAnswer = true,
    backupBlob,
    downloadName,
    downloads = 0;
  const saved = {
    window: globalThis.window,
    document: globalThis.document,
    navigator: Object.getOwnPropertyDescriptor(globalThis, "navigator"),
    indexedDB: globalThis.indexedDB,
    IDBKeyRange: globalThis.IDBKeyRange,
    confirm: globalThis.confirm,
    fetch: globalThis.fetch,
    BroadcastChannel: globalThis.BroadcastChannel,
    createObjectURL: URL.createObjectURL,
    revokeObjectURL: URL.revokeObjectURL,
  };
  globalThis.indexedDB = new IDBFactory();
  globalThis.IDBKeyRange = IDBKeyRange;
  globalThis.confirm = () => confirmAnswer;
  globalThis.fetch = async () => {
    fetchCalls++;
    throw Error("Unexpected network request in local UI test");
  };
  globalThis.BroadcastChannel = class extends saved.BroadcastChannel {
    constructor(...args) {
      super(...args);
      channels.push(this);
    }
  };
  const vc = new VirtualConsole();
  vc.on("jsdomError", (e) => logs.push(e.message));
  let dom;
  function setup() {
    dom = new JSDOM(html, {
      url: "https://client.test/",
      runScripts: "outside-only",
      virtualConsole: vc,
    });
    globalThis.window = dom.window;
    globalThis.document = dom.window.document;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: dom.window.navigator,
    });
    Object.defineProperty(dom.window.navigator, "locks", {
      value: { request: async (_name, fn) => fn() },
    });
    dom.window.HTMLAnchorElement.prototype.click = function () {
      downloads++;
      downloadName = this.download;
    };
    URL.createObjectURL = (blob) => {
      backupBlob = blob;
      return "blob:https://client.test/test";
    };
    URL.revokeObjectURL = () => {};
    return (id) => document.getElementById(id);
  }
  let $ = setup();
  let s;
  try {
    await import("../src/app.js?dom=" + crypto.randomUUID());
    await until(() => $("notice").textContent.startsWith("Ready."));
    assert.match($("mode").textContent, /Local saves by default/);
    assert.equal(fetchCalls, 0);
    checks.push("startup has no relay or signer traffic");
    $("chapter-id").value = "01".repeat(32);
    $("chapter-page").value = "7";
    $("chapter-form").requestSubmit();
    await until(() =>
      $("notice").textContent.startsWith("Progress saved locally"),
    );
    assert.match($("counts").textContent, /1 chapters/);
    assert.match($("records").textContent, /page 7/);
    checks.push("progress form writes locally without a key");
    $("chapter-id").value = "invalid";
    $("chapter-form").requestSubmit();
    await sleep(10);
    s = await LocalLibrary.open();
    assert.equal((await s.counts()).chapters, 1);
    checks.push("native form constraints reject malformed IDs");
    $("series-id").value = "02".repeat(32);
    $("series-status").value = "reading";
    $("series-form").requestSubmit();
    await until(() => $("notice").textContent === "Series saved locally.");
    $("list-name").value = "My local list";
    $("list-form").requestSubmit();
    await until(() => $("notice").textContent === "List created locally.");
    assert.match($("records").textContent, /My local list/);
    $("member-series").value = "02".repeat(32);
    $("member-position").value = "0";
    $("member-form").requestSubmit();
    await until(
      () => $("notice").textContent === "List membership saved locally.",
    );
    assert.equal((await s.counts()).members, 1);
    checks.push("series/list/member forms persist all four families");
    $("export").click();
    await until(() => backupBlob && downloads === 1);
    // The native chooser is outside JSDOM. Check its public HTML accept
    // contract using the filename and MIME emitted by the real Export control.
    const acceptedTypes = $("import")
      .accept.toLowerCase()
      .split(",")
      .map((value) => value.trim());
    function acceptsBackup(name, type) {
      return acceptedTypes.some((value) =>
        value.startsWith(".")
          ? name.toLowerCase().endsWith(value)
          : value === type.toLowerCase(),
      );
    }
    assert.ok(
      acceptsBackup(downloadName, backupBlob.type),
      `Import picker excludes its own export: ${downloadName} (${backupBlob.type}); accept=${$("import").accept}`,
    );
    assert.ok(
      acceptsBackup(downloadName, ""),
      "Export filename remains selectable when the OS does not identify its MIME",
    );
    for (const [name, type] of [
      ["backup.jsonl", ""],
      ["backup.ndjson", ""],
      ["backup", "application/x-ndjson"],
      ["legacy.json", ""],
      ["legacy", "application/json"],
    ]) {
      assert.ok(
        acceptsBackup(name, type),
        `Import picker excludes supported backup ${name} (${type})`,
      );
    }
    checks.push(
      "import picker accept filter includes the actual export, JSONL/NDJSON and legacy JSON",
    );
    const exported = (await backupBlob.text())
      .trim()
      .split("\n")
      .map(JSON.parse);
    assert.equal(exported[0].version, 2);
    const backup = {
      format: "manga-local-library",
      version: 1,
      records: { chapters: [], series: [], lists: [], members: [] },
    };
    for (const [family, record] of exported.slice(1, -1))
      backup.records[family].push(record);
    assert.equal(backup.records.chapters[0].page, 7);
    assert.equal(backup.records.lists[0].name, "My local list");
    checks.push("export creates the correct local-only backup payload");
    $("family").value = "lists";
    $("family").dispatchEvent(new dom.window.Event("change"));
    await until(() => $("records").textContent.includes("My local list"));
    confirmAnswer = false;
    $("records").querySelector("button").click();
    await sleep(15);
    assert.equal((await s.counts()).lists, 1);
    confirmAnswer = true;
    $("records").querySelector("button").click();
    await until(() => $("notice").textContent.startsWith("Removed locally"));
    assert.equal((await s.counts()).lists, 0);
    assert.equal((await s.counts()).members, 0);
    assert.equal((await s.counts()).series, 1);
    checks.push(
      "cancelled deletion does nothing; confirmed list removal preserves series",
    );
    async function importData(value) {
      Object.defineProperty($("import"), "files", {
        configurable: true,
        value: [
          value instanceof Blob ? value : new Blob([JSON.stringify(value)]),
        ],
      });
      $("import").dispatchEvent(new dom.window.Event("change"));
    }
    await importData(backupBlob);
    await until(() => $("notice").textContent === "Backup imported locally.");
    assert.equal((await s.counts()).lists, 1);
    const duplicate = structuredClone(backup);
    duplicate.records.chapters.push(duplicate.records.chapters[0]);
    await importData(duplicate);
    await until(() => $("notice").textContent === "Duplicate import record");
    assert.equal((await s.counts()).chapters, 1);
    checks.push(
      "import restores locally; duplicate records cannot partially replace the library",
    );
    $("connect").click();
    await until(() =>
      $("notice").textContent.startsWith("No NIP07 signer found"),
    );
    assert.equal(fetchCalls, 0);
    dom.window.nostr = {
      getPublicKey: async () => {
        signerCalls++;
        return "03".repeat(32);
      },
      signEvent: async () => {
        signerCalls++;
        throw Error("Must not sign local actions");
      },
    };
    $("connect").click();
    await until(() => $("identity").textContent.includes("03".repeat(32)));
    $("publish").click();
    await until(
      () =>
        $("notice").textContent === "Confirm that publication is public first",
    );
    assert.equal(signerCalls, 1);
    assert.equal(fetchCalls, 0);
    checks.push(
      "missing signer is handled; publication needs explicit public consent",
    );
    await s.setRelays([
      {
        origin: "https://never-contact.example",
        serviceKey: "04".repeat(32),
        selected: true,
      },
    ]);
    for (const c of channels) c.close();
    s.close();
    s = null;
    dom.window.close();
    $ = setup();
    await import("../src/app.js?dom=" + crypto.randomUUID());
    await until(() => $("notice").textContent.startsWith("Ready."));
    assert.match($("counts").textContent, /1 chapters/);
    assert.match($("relays").textContent, /never-contact.example/);
    assert.equal(fetchCalls, 0);
    assert.equal(signerCalls, 1);
    checks.push(
      "reopened app restores data/preferences without contacting a saved relay or signer",
    );
    assert.deepEqual(logs, []);
    await fs.writeFile(
      new URL(
        "../../results/rebuild-client/browser-client-dom.json",
        import.meta.url,
      ),
      JSON.stringify(
        {
          passed: true,
          execution:
            "jsdom plus fake-indexeddb; not an actual browser, visual/layout, download, service-worker or offline certification",
          checks,
          fetchCalls,
          signerCalls,
          jsdomErrors: logs,
        },
        null,
        2,
      ),
    );
  } finally {
    s?.close();
    for (const c of channels) c.close();
    dom?.window.close();
    for (const k of [
      "window",
      "document",
      "indexedDB",
      "IDBKeyRange",
      "confirm",
      "fetch",
      "BroadcastChannel",
    ]) {
      if (saved[k] === undefined) delete globalThis[k];
      else globalThis[k] = saved[k];
    }
    if (saved.navigator)
      Object.defineProperty(globalThis, "navigator", saved.navigator);
    else delete globalThis.navigator;
    URL.createObjectURL = saved.createObjectURL;
    URL.revokeObjectURL = saved.revokeObjectURL;
  }
});
