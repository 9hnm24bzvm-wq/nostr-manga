import "fake-indexeddb/auto";
import { IDBIndex, IDBObjectStore } from "fake-indexeddb";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
import { legacyLibrary } from "./fixtures/legacy-library.mjs";

const id = (n) => n.toString(16).padStart(64, "0");
const chapter = (n, page = 1) => ({
  id: id(n),
  completed: false,
  page,
  updated_at: 0,
});

// External IndexedDB capacity arrangement: real legacy rows and transactions,
// but the upgrade count reports a modeled population. This is not a 2M-row run.
async function capacityFixture(t, total) {
  const name = "capacity-" + crypto.randomUUID();
  await legacyLibrary(name, {
    records: [{ family: "chapters", key: id(1), record: chapter(1) }],
  });
  const count = IDBIndex.prototype.count;
  const fault = t.mock.method(IDBIndex.prototype, "count", function (...args) {
    const query = count.apply(this, args);
    if (
      this.objectStore.transaction.mode === "versionchange" &&
      args[0]?.lower === "chapters"
    )
      query.addEventListener("success", () => {
        Object.defineProperty(query, "result", { value: total });
      });
    return query;
  });
  try {
    return await LocalLibrary.open(name);
  } finally {
    fault.mock.restore();
  }
}

test("modeled at-cap legacy library preserves updates and deletes while rejecting additions across reopen", async (t) => {
  let store = await capacityFixture(t, 2000000);
  const name = store.db.name;
  try {
    await assert.rejects(store.write("chapters", chapter(2)), /record limit/i);
    assert.deepEqual(await store.list("chapters"), [chapter(1)]);
    assert.equal((await store.counts()).revision, 0);
    await store.write("chapters", chapter(1, 9));
    await store.remove("chapters", id(999));
    await assert.rejects(store.write("chapters", chapter(2)), /record limit/i);
    await store.remove("chapters", id(1));
    await store.write("chapters", chapter(2));
    store.close();
    store = await LocalLibrary.open(name);
    assert.deepEqual(await store.list("chapters"), [chapter(2)]);
    await assert.rejects(store.write("chapters", chapter(3)), /record limit/i);
  } finally {
    store.close();
  }
});

test("legacy over-budget data is preserved and diagnosed without denying updates or deletes", async (t) => {
  const store = await capacityFixture(t, 2000001);
  try {
    const status = await store.counts();
    assert.equal(status.total, 2000001);
    assert.equal(status.recordLimit, 2000000);
    assert.equal(status.overBudget, true);
    assert.deepEqual(await store.list("chapters"), [chapter(1)]);
    await store.write("chapters", chapter(1, 9));
    await assert.rejects(store.write("chapters", chapter(2)), /record limit/i);
    await store.remove("chapters", id(1));
    assert.equal((await store.counts()).overBudget, false);
    await assert.rejects(store.write("chapters", chapter(2)), /record limit/i);
  } finally {
    store.close();
  }
});

test("two connections cannot both consume the last modeled record slot", async (t) => {
  const a = await capacityFixture(t, 1999999);
  const b = await LocalLibrary.open(a.db.name);
  try {
    const results = await Promise.allSettled([
      a.write("chapters", chapter(2)),
      b.write("chapters", chapter(3)),
    ]);
    assert.equal(results.filter((r) => r.status === "fulfilled").length, 1);
    assert.match(
      results.find((r) => r.status === "rejected").reason.message,
      /record limit/i,
    );
    const rows = await a.list("chapters");
    assert.equal(rows.length, 2);
    assert.deepEqual(rows[0], chapter(1));
    assert.equal((await b.counts()).revision, 1);
    assert.equal((await b.counts()).total, 2000000);
  } finally {
    a.close();
    b.close();
  }
});

test("actual legacy rows initialize capacity and list cascades release exactly their records", async () => {
  const name = "actual-migration-" + crypto.randomUUID();
  await legacyLibrary(name, {
    records: [{ family: "chapters", key: id(1), record: chapter(1) }],
  });
  let store = await LocalLibrary.open(name);
  try {
    assert.equal((await store.counts()).total, 1);
    await store.write("chapters", chapter(1, 3));
    await store.write("lists", { id: "11".repeat(16), name: "Keep exact" });
    await store.write("members", {
      list: "11".repeat(16),
      series: id(8),
      position: 0,
    });
    await store.write("members", {
      list: "22".repeat(16),
      series: id(8),
      position: 1,
    });
    assert.equal((await store.counts()).total, 4);
    await store.remove("lists", "11".repeat(16));
    store.close();
    store = await LocalLibrary.open(name);
    assert.deepEqual(await store.counts(), {
      chapters: 1,
      series: 0,
      lists: 0,
      members: 1,
      revision: 5,
      total: 2,
      recordLimit: 2000000,
      overBudget: false,
    });
    assert.deepEqual(await store.list("chapters"), [chapter(1, 3)]);
    assert.equal((await store.list("members"))[0].list, "22".repeat(16));
  } finally {
    store.close();
  }
});

test("reviewed replacement resets capacity using the accepted records", async (t) => {
  const store = await capacityFixture(t, 2000001);
  try {
    await store.replace(
      {
        format: "manga-local-library",
        version: 1,
        records: { chapters: [chapter(3)], series: [], lists: [], members: [] },
      },
      0,
    );
    await store.write("chapters", chapter(4));
    assert.deepEqual(await store.list("chapters"), [chapter(3), chapter(4)]);
    assert.equal((await store.counts()).total, 2);
    assert.equal((await store.counts()).overBudget, false);
  } finally {
    store.close();
  }
});

test("streamed import atomically replaces the capacity diagnosis as well as records", async (t) => {
  const { importBackup } = await import("../src/backup.js");
  const saved = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: {
      locks: {
        request: async (_name, options, callback) => (callback ?? options)({}),
      },
    },
  });
  const store = await capacityFixture(t, 2000001);
  try {
    const file = new Blob([
      JSON.stringify({ format: "manga-local-library", version: 2 }) + "\n",
      JSON.stringify(["chapters", chapter(3)]) + "\n",
      JSON.stringify({ end: 1 }) + "\n",
    ]);
    await importBackup(store, file, 0);
    await store.write("chapters", chapter(4));
    assert.deepEqual(await store.list("chapters"), [chapter(3), chapter(4)]);
    assert.equal((await store.counts()).total, 2);
    assert.equal((await store.counts()).overBudget, false);
  } finally {
    store.close();
    if (saved) Object.defineProperty(globalThis, "navigator", saved);
    else delete globalThis.navigator;
  }
});

test("an over-budget legacy library has a bounded rescue stream distinct from an importable backup", async (t) => {
  const backup = await import("../src/backup.js");
  const store = await capacityFixture(t, 2000001);
  try {
    await assert.rejects(backup.backupBlob(store), /record limit/i);
    const lines = [];
    for await (const line of backup.exportRescue(store)) {
      assert.ok(new TextEncoder().encode(line).length <= 65536);
      lines.push(JSON.parse(line));
    }
    assert.equal(lines[0].format, "manga-local-library-rescue");
    assert.equal(lines[0].version, 1);
    assert.deepEqual(lines.slice(1), [["chapters", chapter(1)], { end: 1 }]);
    const file = new Blob(lines.map((value) => JSON.stringify(value) + "\n"));
    await assert.rejects(
      backup.importBackup(store, file, 0),
      /Unsupported backup/,
    );
    assert.deepEqual(await store.list("chapters"), [chapter(1)]);
    const iterator = backup.exportRescue(store);
    await iterator.next();
    await store.write("chapters", chapter(1, 5));
    await assert.rejects(iterator.next(), /changed/);
  } finally {
    store.close();
  }
});

test("an aborted admission does not consume the last slot and same-key concurrent updates consume only one", async (t) => {
  const a = await capacityFixture(t, 1999999);
  const b = await LocalLibrary.open(a.db.name);
  const put = IDBObjectStore.prototype.put;
  let interrupted = false;
  const fault = t.mock.method(
    IDBObjectStore.prototype,
    "put",
    function (...args) {
      const query = put.apply(this, args);
      if (!interrupted) {
        interrupted = true;
        query.addEventListener("success", () => this.transaction.abort());
      }
      return query;
    },
  );
  try {
    await assert.rejects(a.write("chapters", chapter(2)));
    fault.mock.restore();
    assert.deepEqual(await a.list("chapters"), [chapter(1)]);
    assert.equal((await a.counts()).total, 1999999);
    await Promise.all([
      a.write("chapters", chapter(2)),
      b.write("chapters", chapter(2, 3)),
    ]);
    assert.equal((await a.counts()).total, 2000000);
    assert.equal((await a.list("chapters")).length, 2);
    await assert.rejects(a.write("chapters", chapter(3)), /record limit/i);
  } finally {
    fault.mock.restore();
    a.close();
    b.close();
  }
});
