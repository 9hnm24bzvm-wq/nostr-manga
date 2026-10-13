import "fake-indexeddb/auto";
import { IDBObjectStore } from "fake-indexeddb";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
import { importBackup } from "../src/backup.js";

const id = (n) => n.toString(16).padStart(64, "0");
const chapter = (n) => ({
  id: id(n),
  completed: false,
  page: 1,
  updated_at: 0,
});
const deferred = () => {
  let resolve;
  const promise = new Promise((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
function fileFixture() {
  return new Blob([
    JSON.stringify({ format: "manga-local-library", version: 2 }) + "\n",
    ...Array.from(
      { length: 1500 },
      (_, n) => JSON.stringify(["chapters", chapter(n)]) + "\n",
    ),
    '{"end":1500}\n',
  ]);
}

// External Web Locks adapter with exclusion and ifAvailable; this is not native
// browser scheduling or actual process termination evidence.
function lockAdapter() {
  const held = new Set();
  return {
    async request(name, options, callback) {
      if (typeof options === "function") {
        callback = options;
        options = {};
      }
      if (held.has(name)) {
        if (options.ifAvailable) return callback(null);
        throw Error("Test fixture does not request queued locks");
      }
      held.add(name);
      try {
        return await callback({ name });
      } finally {
        held.delete(name);
      }
    },
  };
}
function navigatorFixture(locks) {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { locks },
  });
  return () => {
    if (saved) Object.defineProperty(globalThis, "navigator", saved);
    else delete globalThis.navigator;
  };
}

test("normal reopen frees abandoned import occupancy while preserving current records", async (t) => {
  const restoreNavigator = navigatorFixture(lockAdapter());
  const name = "orphan-" + crypto.randomUUID();
  let store = await LocalLibrary.open(name);
  const file = fileFixture();
  let disconnected = false;
  // External storage model: add requests consume free capacity on commit;
  // successful clear releases that store's occupancy. The old library predates
  // this free-space budget. No private stored values are outcome assertions.
  const occupancy = new Map();
  const add = IDBObjectStore.prototype.add,
    clear = IDBObjectStore.prototype.clear,
    put = IDBObjectStore.prototype.put;
  const addFault = t.mock.method(
    IDBObjectStore.prototype,
    "add",
    function (...args) {
      const query = add.apply(this, args),
        name = this.name;
      this.transaction.addEventListener(
        "complete",
        () => occupancy.set(name, (occupancy.get(name) ?? 0) + 1),
        { once: true },
      );
      return query;
    },
  );
  const clearFault = t.mock.method(
    IDBObjectStore.prototype,
    "clear",
    function (...args) {
      if (disconnected)
        throw new DOMException(
          "Storage temporarily unavailable",
          "UnknownError",
        );
      const query = clear.apply(this, args),
        name = this.name;
      this.transaction.addEventListener(
        "complete",
        () => occupancy.delete(name),
        { once: true },
      );
      return query;
    },
  );
  const putFault = t.mock.method(
    IDBObjectStore.prototype,
    "put",
    function (...args) {
      if ([...occupancy.values()].reduce((a, b) => a + b, 0) >= 500)
        throw new DOMException("Origin quota exceeded", "QuotaExceededError");
      return put.apply(this, args);
    },
  );
  const interrupted = {
    size: file.size,
    slice(start, end) {
      const part = file.slice(start, end);
      return {
        text: () => part.text(),
        arrayBuffer() {
          if (start >= 131072) {
            disconnected = true;
            throw new DOMException("File disconnected", "NotReadableError");
          }
          return part.arrayBuffer();
        },
      };
    },
  };
  try {
    await store.write("chapters", chapter(9999));
    await assert.rejects(importBackup(store, interrupted, 1), {
      name: "NotReadableError",
    });
    disconnected = false;
    await assert.rejects(store.write("chapters", chapter(9998)), {
      name: "QuotaExceededError",
    });
    store.close();
    const restoreNoLocks = navigatorFixture(undefined);
    store = await LocalLibrary.open(name);
    try {
      assert.deepEqual(await store.list("chapters"), [chapter(9999)]);
      await assert.rejects(store.write("chapters", chapter(9998)), {
        name: "QuotaExceededError",
      });
    } finally {
      store.close();
      restoreNoLocks();
    }
    store = await LocalLibrary.open(name);
    assert.deepEqual(await store.list("chapters"), [chapter(9999)]);
    await store.write("chapters", chapter(9998));
    assert.deepEqual(await store.list("chapters"), [
      chapter(9998),
      chapter(9999),
    ]);
    assert.equal((await store.counts()).total, 2);
  } finally {
    addFault.mock.restore();
    clearFault.mock.restore();
    putFault.mock.restore();
    store.close();
    restoreNavigator();
  }
});

test("another tab reopening while an import holds its lock cannot erase active staged rows", async () => {
  const restoreNavigator = navigatorFixture(lockAdapter());
  const name = "active-import-" + crypto.randomUUID();
  const importer = await LocalLibrary.open(name);
  const file = fileFixture(),
    waiting = deferred(),
    resume = deferred();
  let observer, importPromise;
  const slow = {
    size: file.size,
    slice(start, end) {
      const part = file.slice(start, end);
      return {
        text: () => part.text(),
        async arrayBuffer() {
          if (start >= 131072) {
            waiting.resolve();
            await resume.promise;
          }
          return part.arrayBuffer();
        },
      };
    },
  };
  try {
    await importer.write("chapters", chapter(9999));
    importPromise = importBackup(importer, slow, 1);
    await waiting.promise;
    observer = await LocalLibrary.open(name);
    assert.deepEqual(await observer.list("chapters"), [chapter(9999)]);
    resume.resolve();
    await importPromise;
    const restored = [];
    for (let after = null; ; ) {
      const rows = await observer.list("chapters", after, 500);
      restored.push(...rows);
      if (rows.length < 500) break;
      after = rows.at(-1).id;
    }
    assert.deepEqual(
      restored,
      Array.from({ length: 1500 }, (_, n) => chapter(n)),
    );
    assert.equal((await observer.counts()).total, 1500);
  } finally {
    resume.resolve();
    await importPromise?.catch(() => {});
    observer?.close();
    importer.close();
    restoreNavigator();
  }
});
