import "fake-indexeddb/auto";
import { IDBIndex, IDBObjectStore } from "fake-indexeddb";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
import { backupBlob, importBackup } from "../src/backup.js";
import { versionThreeLibrary } from "./fixtures/version-three-library.mjs";
import { versionFourLibrary } from "./fixtures/version-four-library.mjs";

Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    locks: { request: async (_name, options, fn) => (fn ?? options)({}) },
  },
});

const id = (n) => n.toString(16).padStart(64, "0");
const chapter = (n, page = 1) => ({
  id: id(n),
  completed: false,
  page,
  updated_at: 0,
});

const listA = "11".repeat(16),
  listB = "22".repeat(16);
const initial = {
  chapters: [chapter(1), chapter(2)],
  series: [{ id: id(3), status: "reading", updated_at: 0 }],
  lists: [
    { id: listA, name: "A", updated_at: 0 },
    { id: listB, name: "B", updated_at: 0 },
  ],
  members: [
    { list: listA, series: id(3), position: 0, updated_at: 0 },
    { list: listA, series: id(4), position: 1, updated_at: 0 },
    { list: listB, series: id(3), position: 0, updated_at: 0 },
  ],
};
const snapshot = (records) => ({
  format: "manga-local-library",
  version: 1,
  records,
});
const initialCounts = {
  chapters: 2,
  series: 1,
  lists: 2,
  members: 3,
  total: 8,
  recordLimit: 2000000,
  overBudget: false,
};
const changed = {
  chapters: [chapter(9)],
  series: [],
  lists: [{ id: listB, name: "Replacement", updated_at: 3 }],
  members: [],
};
const changedCounts = {
  chapters: 1,
  series: 0,
  lists: 1,
  members: 0,
  total: 2,
  recordLimit: 2000000,
  overBudget: false,
};
const jsonl = (records) => {
  const rows = Object.entries(records).flatMap(([family, values]) =>
    values.map((record) => [family, record]),
  );
  return new Blob([
    ...[
      { format: "manga-local-library", version: 2 },
      ...rows,
      { end: rows.length },
    ].map((value) => JSON.stringify(value) + "\n"),
  ]);
};

for (const [version, fixture] of [
  [3, versionThreeLibrary],
  [4, versionFourLibrary],
]) {
  test(`schema ${version} migration preserves every family, revision and pending publication bytes`, async () => {
    const name = "statistics-migrate-" + crypto.randomUUID();
    const event = { pubkey: id(8), created_at: 9 };
    const old = await fixture(name, {
      records: Object.entries(initial).flatMap(([family, rows]) =>
        rows.map((record) => ({
          family,
          key: family === "members" ? record.list + record.series : record.id,
          record,
        })),
      ),
      revision: 7,
      jobs: [
        {
          id: "pending",
          revision: 0,
          event,
          cancelled: false,
          recipients: [],
          objectCount: 1,
        },
      ],
      objects: [
        {
          job: "pending",
          index: 0,
          object: {
            type: "chunk",
            hash: "fixture",
            data: new Uint8Array([0, 127, 255]),
          },
        },
      ],
    });
    old.close();
    let store = await LocalLibrary.open(name);
    const revision = version === 4 ? 7 : 0;
    try {
      assert.deepEqual(await store.counts(), { ...initialCounts, revision });
      assert.deepEqual((await store.snapshot()).records, initial);
      assert.deepEqual((await store.job("pending")).event, event);
      assert.deepEqual(
        [...(await store.jobBatch("pending", 0)).objects[0].data],
        [0, 127, 255],
      );
      store.close();
      store = await LocalLibrary.open(name);
      assert.deepEqual(await store.counts(), { ...initialCounts, revision });
      await store.write("chapters", chapter(1, 99));
      assert.deepEqual(await store.counts(), {
        ...initialCounts,
        revision: revision + 1,
      });
      assert.deepEqual(await store.list("chapters"), [
        chapter(1, 99),
        chapter(2),
      ]);
    } finally {
      store.close();
    }
  });
}

test("interrupted statistics upgrade can reopen the unchanged legacy library", async (t) => {
  const name = "statistics-upgrade-abort-" + crypto.randomUUID();
  const old = await versionFourLibrary(name, {
    records: [{ family: "chapters", key: id(1), record: chapter(1) }],
    revision: 7,
  });
  old.close();
  const count = IDBIndex.prototype.count;
  let interrupted = false;
  const fault = t.mock.method(IDBIndex.prototype, "count", function (...args) {
    const query = count.apply(this, args);
    if (!interrupted) {
      interrupted = true;
      query.addEventListener("success", () =>
        this.objectStore.transaction.abort(),
      );
    }
    return query;
  });
  await assert.rejects(LocalLibrary.open(name));
  fault.mock.restore();
  const store = await LocalLibrary.open(name);
  try {
    assert.deepEqual(await store.list("chapters"), [chapter(1)]);
    assert.deepEqual(await store.counts(), {
      chapters: 1,
      series: 0,
      lists: 0,
      members: 0,
      total: 1,
      revision: 7,
      recordLimit: 2000000,
      overBudget: false,
    });
  } finally {
    store.close();
  }
});

test("streamed mixed-family import replaces all family totals together", async () => {
  const store = await LocalLibrary.open(
    "statistics-import-" + crypto.randomUUID(),
  );
  try {
    await store.write("chapters", chapter(99));
    await importBackup(store, jsonl(initial), 1);
    assert.deepEqual(await store.counts(), { ...initialCounts, revision: 2 });
    assert.deepEqual((await store.snapshot()).records, initial);
  } finally {
    store.close();
  }
});

test("family totals follow updates, absent deletes and list cascades, including orphan memberships", async () => {
  const store = await LocalLibrary.open(
    "statistics-mutations-" + crypto.randomUUID(),
  );
  try {
    await store.replace(snapshot(initial), 0);
    for (const [family, rows] of Object.entries(initial))
      await store.write(family, { ...rows[0], updated_at: 5 });
    assert.deepEqual(await store.counts(), { ...initialCounts, revision: 5 });
    await store.remove("chapters", id(999));
    assert.deepEqual(await store.counts(), { ...initialCounts, revision: 6 });
    await store.remove("lists", listA);
    assert.deepEqual(await store.counts(), {
      ...initialCounts,
      lists: 1,
      members: 1,
      total: 5,
      revision: 7,
    });
    assert.deepEqual(await store.list("members"), [initial.members[2]]);
    await store.remove("lists", listA);
    await store.write("members", initial.members[0]);
    await store.remove("lists", listA);
    assert.deepEqual(await store.counts(), {
      ...initialCounts,
      lists: 1,
      members: 1,
      total: 5,
      revision: 10,
    });
    await store.remove("members", listB + id(3));
    await store.remove("series", id(3));
    await store.remove("chapters", id(1));
    assert.deepEqual(await store.counts(), {
      ...changedCounts,
      revision: 13,
    });
    assert.deepEqual(await store.list("chapters"), [chapter(2)]);
    assert.deepEqual(await store.list("series"), []);
    assert.deepEqual(await store.list("members"), []);
  } finally {
    store.close();
  }
});

test("two connections conserve family totals across concurrent inserts, updates and deletion", async () => {
  const name = "statistics-tabs-" + crypto.randomUUID();
  const a = await LocalLibrary.open(name),
    b = await LocalLibrary.open(name);
  try {
    await Promise.all([
      a.write("chapters", chapter(1)),
      b.write("chapters", chapter(1, 9)),
      a.write("series", initial.series[0]),
      b.write("lists", initial.lists[0]),
    ]);
    const expected = {
      ...initialCounts,
      chapters: 1,
      lists: 1,
      members: 0,
      total: 3,
      revision: 4,
    };
    assert.deepEqual(await a.counts(), expected);
    assert.deepEqual(await b.counts(), expected);
    assert.equal((await a.list("chapters")).length, 1);
    await Promise.all([
      a.remove("chapters", id(1)),
      b.remove("chapters", id(1)),
      a.write("members", initial.members[0]),
    ]);
    assert.deepEqual(await b.counts(), {
      ...expected,
      chapters: 0,
      members: 1,
      total: 3,
      revision: 7,
    });
    assert.deepEqual(await a.list("chapters"), []);
    assert.deepEqual(await b.list("members"), [initial.members[0]]);
  } finally {
    a.close();
    b.close();
  }
});

test("aborted list deletion preserves its rows and family totals before a successful retry", async (t) => {
  const store = await LocalLibrary.open(
    "statistics-delete-abort-" + crypto.randomUUID(),
  );
  try {
    await store.replace(snapshot(initial), 0);
    const remove = IDBObjectStore.prototype.delete;
    const fault = t.mock.method(
      IDBObjectStore.prototype,
      "delete",
      function (...args) {
        const query = remove.apply(this, args);
        query.addEventListener("success", () => this.transaction.abort());
        return query;
      },
    );
    await assert.rejects(store.remove("lists", listA));
    fault.mock.restore();
    assert.deepEqual(await store.counts(), { ...initialCounts, revision: 1 });
    assert.deepEqual((await store.snapshot()).records, initial);
    await store.remove("lists", listA);
    assert.deepEqual(await store.counts(), {
      ...initialCounts,
      lists: 1,
      members: 1,
      total: 5,
      revision: 2,
    });
    assert.deepEqual(await store.list("members"), [initial.members[2]]);
  } finally {
    store.close();
  }
});

for (const format of [1, 2]) {
  test(`version-${format} replacement rolls family statistics back on abort and permits a clean retry`, async (t) => {
    const name = "statistics-replace-" + crypto.randomUUID();
    let store = await LocalLibrary.open(name);
    const replace = (revision) =>
      format === 1
        ? store.replace(snapshot(changed), revision)
        : importBackup(store, jsonl(changed), revision);
    try {
      await store.replace(snapshot(initial), 0);
      const duplicate = { ...changed, chapters: [chapter(9), chapter(9)] };
      await assert.rejects(
        format === 1
          ? store.replace(snapshot(duplicate), 1)
          : importBackup(store, jsonl(duplicate), 1),
      );
      await assert.rejects(replace(0), /changed/);
      assert.deepEqual(await store.counts(), { ...initialCounts, revision: 1 });
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
      await assert.rejects(replace(1));
      fault.mock.restore();
      store.close();
      store = await LocalLibrary.open(name);
      assert.deepEqual(await store.counts(), { ...initialCounts, revision: 1 });
      assert.deepEqual((await store.snapshot()).records, initial);
      await replace(1);
      assert.deepEqual(await store.counts(), { ...changedCounts, revision: 2 });
      assert.deepEqual((await store.snapshot()).records, changed);
      const target = await LocalLibrary.open(
        "statistics-roundtrip-" + crypto.randomUUID(),
      );
      try {
        await importBackup(target, await backupBlob(store), 0);
        assert.deepEqual(await target.counts(), {
          ...changedCounts,
          revision: 1,
        });
        assert.deepEqual((await target.snapshot()).records, changed);
      } finally {
        target.close();
      }
      const empty = { chapters: [], series: [], lists: [], members: [] };
      if (format === 1) await store.replace(snapshot(empty), 2);
      else await importBackup(store, jsonl(empty), 2);
      assert.deepEqual(await store.counts(), {
        chapters: 0,
        series: 0,
        lists: 0,
        members: 0,
        total: 0,
        revision: 3,
        recordLimit: 2000000,
        overBudget: false,
      });
      assert.deepEqual((await store.snapshot()).records, empty);
    } finally {
      store.close();
    }
  });
}

test("refresh counts remain available when the external database refuses repeated range counting", async (t) => {
  const name = "statistics-refresh-" + crypto.randomUUID();
  let store = await LocalLibrary.open(name);
  try {
    await store.write("chapters", chapter(1));
    // Model an unavailable expensive dependency after initialization. This
    // asserts the public result, never private metadata or an exact call count.
    t.mock.method(IDBIndex.prototype, "count", () => {
      throw Error("Repeated IndexedDB range counting unavailable");
    });
    t.mock.method(IDBObjectStore.prototype, "count", () => {
      throw Error("Repeated IndexedDB object-store counting unavailable");
    });
    const expected = {
      chapters: 1,
      series: 0,
      lists: 0,
      members: 0,
      total: 1,
      revision: 1,
      recordLimit: 2000000,
      overBudget: false,
    };
    assert.deepEqual(await store.counts(), expected);
    store.close();
    store = await LocalLibrary.open(name);
    assert.deepEqual(await store.counts(), expected);
    assert.deepEqual(await store.list("chapters", null, 50), [chapter(1)]);
  } finally {
    store.close();
  }
});
