import "fake-indexeddb/auto";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
import { backupBlob, importBackup } from "../src/backup.js";
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    locks: { request: async (_name, options, fn) => (fn ?? options)({}) },
  },
});
const id = (number) => number.toString(16).padStart(64, "0");
test("version-two local backup restores all families without changing saved instances", async () => {
  const source = await LocalLibrary.open("source-" + crypto.randomUUID()),
    target = await LocalLibrary.open("target-" + crypto.randomUUID());
  const records = {
    chapters: [{ id: id(1), completed: false, page: 7, updated_at: 1 }],
    series: [{ id: id(2), status: "reading", updated_at: 2 }],
    lists: [{ id: "11".repeat(16), name: "Reading 🐈", updated_at: 3 }],
    members: [
      { list: "11".repeat(16), series: id(2), position: 0, updated_at: 4 },
    ],
  };
  try {
    for (const [family, rows] of Object.entries(records))
      for (const record of rows) await source.write(family, record);
    await target.setRelays([
      { origin: "https://relay.test", serviceKey: id(2), selected: true },
    ]);
    const file = await backupBlob(source);
    assert.equal(JSON.parse((await file.text()).split("\n")[0]).version, 2);
    await importBackup(target, file, 0);
    assert.deepEqual((await target.snapshot()).records, records);
    assert.equal(
      (await target.relayPreferences()).relays[0].origin,
      "https://relay.test",
    );
  } finally {
    source.close();
    target.close();
  }
});

const encode = (rows) =>
  new Blob(rows.map((row) => JSON.stringify(row) + "\n"));
const header = { format: "manga-local-library", version: 2 };
test("invalid, duplicate, oversized and stale backups never replace current local records", async () => {
  const store = await LocalLibrary.open("invalid-" + crypto.randomUUID());
  const current = { id: id(9), completed: true, page: 7, updated_at: 1 };
  const row = ["chapters", { id: id(1), completed: false }];
  try {
    await store.write("chapters", current);
    for (const file of [
      encode([header, row]),
      encode([header, row, row, { end: 2 }]),
      encode([header, row, { end: 1 }, row]),
      encode([header, row, { end: 3 }]),
      encode([
        header,
        ["chapters", { ...row[1], padding: "é".repeat(33000) }],
        { end: 1 },
      ]),
      new Blob([JSON.stringify(header) + "\n", new Uint8Array([0xff, 10])]),
    ]) {
      await assert.rejects(importBackup(store, file, 1));
      assert.deepEqual(await store.list("chapters"), [current]);
    }
    await assert.rejects(
      importBackup(store, encode([header, row, { end: 1 }]), 0),
      /changed/,
    );
    await assert.rejects(
      importBackup(store, { size: 4 * 1024 ** 3 + 1 }, 1),
      /file limit/,
    );
    const tooLargeLegacy = {
      size: 256 * 1024 * 1024 + 1,
      slice: () => new Blob(['{"format":"manga-local-library","version":1}']),
    };
    await assert.rejects(importBackup(store, tooLargeLegacy, 1), /Legacy/);
    const backup = await store.snapshot();
    await importBackup(store, new Blob([JSON.stringify(backup)]), 1);
    assert.deepEqual(await store.list("chapters"), [current]);
  } finally {
    store.close();
  }
});

test("database abort during replacement preserves old data after reopen and a clean retry succeeds", async (t) => {
  const { IDBObjectStore } = await import("fake-indexeddb");
  const name = "abort-" + crypto.randomUUID();
  let store = await LocalLibrary.open(name);
  const current = { id: id(9), completed: true, page: 7, updated_at: 1 };
  const incoming = { id: id(1), completed: false, page: 2, updated_at: 2 };
  const file = encode([header, ["chapters", incoming], { end: 1 }]);
  try {
    await store.write("chapters", current);
    const put = IDBObjectStore.prototype.put;
    let interrupted = false;
    const fault = t.mock.method(
      IDBObjectStore.prototype,
      "put",
      function (...args) {
        const query = put.apply(this, args);
        if (!interrupted) {
          interrupted = true;
          queueMicrotask(() => this.transaction.abort());
        }
        return query;
      },
    );
    await assert.rejects(importBackup(store, file, 1));
    fault.mock.restore();
    assert.ok(interrupted);
    await new Promise(setImmediate);
    store.close();
    store = await LocalLibrary.open(name);
    assert.deepEqual(await store.list("chapters"), [current]);
    await importBackup(store, file, 1);
    assert.deepEqual(await store.list("chapters"), [incoming]);
  } finally {
    store.close();
  }
});

test("interrupted file reads preserve the prior library and failed exports reject concurrent edits", async () => {
  const { exportBackup } = await import("../src/backup.js");
  const store = await LocalLibrary.open("read-failure-" + crypto.randomUUID());
  const current = { id: id(9000), completed: true, page: 7, updated_at: 1 };
  const file = encode([
    header,
    ...Array.from({ length: 1500 }, (_, i) => [
      "chapters",
      { id: id(i), completed: false, page: 2, updated_at: 2 },
    ]),
    { end: 1500 },
  ]);
  const unreadable = {
    size: file.size,
    slice(start, end) {
      const part = file.slice(start, end);
      return {
        text: () => part.text(),
        arrayBuffer() {
          if (start >= 131072)
            throw new DOMException("Device disconnected", "NotReadableError");
          return part.arrayBuffer();
        },
      };
    },
  };
  try {
    await store.write("chapters", current);
    await assert.rejects(importBackup(store, unreadable, 1), {
      name: "NotReadableError",
    });
    assert.deepEqual(await store.list("chapters"), [current]);
    const iterator = exportBackup(store);
    await iterator.next();
    await store.write("chapters", { ...current, page: 8 });
    await assert.rejects(iterator.next(), /changed/);
    await importBackup(store, file, 2);
    assert.equal((await store.counts()).chapters, 1500);
  } finally {
    store.close();
  }
});
