import fs from "node:fs/promises";
import { JSDOM } from "jsdom";
import { IDBFactory, IDBKeyRange } from "fake-indexeddb";
import { LocalLibrary } from "../../src/store.js";
export async function until(predicate, message = "UI did not settle") {
  for (let i = 0; i < 200; i++) {
    if (await predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw Error(message);
}
export async function domHarness() {
  const names = [
    "window",
    "document",
    "navigator",
    "indexedDB",
    "IDBKeyRange",
    "BroadcastChannel",
    "confirm",
    "fetch",
    "Worker",
  ];
  const saved = new Map(
    names.map((name) => [
      name,
      Object.getOwnPropertyDescriptor(globalThis, name),
    ]),
  );
  globalThis.indexedDB = new IDBFactory();
  globalThis.IDBKeyRange = IDBKeyRange;
  globalThis.BroadcastChannel = undefined;
  const store = await LocalLibrary.open();
  const dom = new JSDOM(
    await fs.readFile(new URL("../../index.html", import.meta.url), "utf8"),
    { url: "https://client.test/", runScripts: "outside-only" },
  );
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { locks: { request: async (_name, opts, fn) => (fn ?? opts)({}) } },
  });
  globalThis.confirm = () => true;
  globalThis.fetch = async () => {
    throw Error("Unexpected request in local UI test");
  };
  const $ = (id) => document.getElementById(id);
  return {
    store,
    dom,
    $,
    async start() {
      await import("../../src/app.js?test=" + crypto.randomUUID());
      await until(() => $("notice").textContent.startsWith("Ready."));
    },
    close() {
      store.close();
      dom.window.close();
      for (const name of names) {
        const descriptor = saved.get(name);
        if (descriptor) Object.defineProperty(globalThis, name, descriptor);
        else delete globalThis[name];
      }
    },
  };
}
