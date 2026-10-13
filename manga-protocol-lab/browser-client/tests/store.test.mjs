import "fake-indexeddb/auto";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
const id = (n) => n.toString(16).padStart(64, "0");
test("local storage persists across connections and serializes concurrent writes", async () => {
  const name = "test-" + crypto.randomUUID(),
    a = await LocalLibrary.open(name),
    b = await LocalLibrary.open(name);
  try {
    await Promise.all([
      a.write("chapters", { id: id(1), completed: false, page: 1 }),
      b.write("chapters", { id: id(2), completed: true, page: 9 }),
    ]);
    assert.equal((await a.counts()).revision, 2);
    assert.equal((await b.list("chapters")).length, 2);
    const backup = await a.snapshot();
    await b.write("chapters", { id: id(3), completed: false, page: 4 });
    await assert.rejects(a.replace(backup, backup.revision), /changed/);
    assert.equal((await a.counts()).chapters, 3);
  } finally {
    a.close();
    b.close();
  }
});
test("import validates before mutation and strips unrecognized local fields", async () => {
  const s = await LocalLibrary.open("test-" + crypto.randomUUID());
  try {
    await s.write("chapters", {
      id: id(1),
      completed: false,
      page: 1,
      secretNote: "no",
    });
    const snap = await s.snapshot();
    assert.equal(snap.records.chapters[0].secretNote, undefined);
    snap.records.chapters.push(snap.records.chapters[0]);
    await assert.rejects(s.replace(snap, snap.revision), /Duplicate/);
    assert.equal((await s.counts()).chapters, 1);
  } finally {
    s.close();
  }
});
test("cancellation is sticky against stale worker saves", async () => {
  const s = await LocalLibrary.open("test-" + crypto.randomUUID());
  try {
    const job = {
      id: "fixture",
      revision: 0,
      event: { pubkey: id(2), created_at: 3 },
      cancelled: false,
      recipients: [],
    };
    await s.createJob(job);
    const cancelled = await s.job("fixture");
    cancelled.cancelled = true;
    await s.saveJob(cancelled);
    await s.saveJob(job);
    assert.equal((await s.job("fixture")).cancelled, true);
  } finally {
    s.close();
  }
});
test("deleting a local list removes only its membership records", async () => {
  const s = await LocalLibrary.open("test-" + crypto.randomUUID());
  try {
    await s.write("lists", { id: "11".repeat(16), name: "Local" });
    await s.write("members", {
      list: "11".repeat(16),
      series: id(1),
      position: 0,
    });
    await s.write("series", { id: id(1), status: "reading" });
    await s.remove("lists", "11".repeat(16));
    const c = await s.counts();
    assert.equal(c.lists, 0);
    assert.equal(c.members, 0);
    assert.equal(c.series, 1);
  } finally {
    s.close();
  }
});
test("outbox creation atomically fences local edits and records publication identity", async () => {
  const s = await LocalLibrary.open("test-" + crypto.randomUUID());
  try {
    await s.write("chapters", { id: id(1), completed: false, page: 1 });
    const job = {
      id: "job",
      revision: 1,
      event: { pubkey: id(2), created_at: 3 },
      cancelled: false,
      recipients: [],
    };
    await s.createJob(job);
    assert.deepEqual((await s.job("job")).event, job.event);
    await s.write("chapters", { id: id(1), completed: false, page: 2 });
    await assert.rejects(s.createJob({ ...job, id: "stale" }), /changed/);
    assert.equal((await s.jobs()).length, 1);
    assert.deepEqual((await s.job("job")).event, job.event);
  } finally {
    s.close();
  }
});

test("a synchronous storage failure during replacement cannot commit an empty library", async (t) => {
  const { IDBObjectStore } = await import("fake-indexeddb");
  const name = "quota-" + crypto.randomUUID();
  let store = await LocalLibrary.open(name);
  try {
    await store.write("chapters", {
      id: id(1),
      completed: false,
      page: 7,
      updated_at: 1,
    });
    const prior = await store.snapshot();
    const next = structuredClone(prior);
    next.records.chapters[0].page = 99;
    const fault = t.mock.method(IDBObjectStore.prototype, "put", () => {
      throw new DOMException("Storage exhausted", "QuotaExceededError");
    });
    await assert.rejects(store.replace(next, prior.revision), {
      name: "QuotaExceededError",
    });
    fault.mock.restore();
    store.close();
    store = await LocalLibrary.open(name);
    assert.deepEqual((await store.snapshot()).records, prior.records);
    await store.replace(next, prior.revision);
    assert.equal((await store.list("chapters"))[0].page, 99);
  } finally {
    store.close();
  }
});

test("a snapshot read aborted at completion rejects and a later read still returns the saved library", async (t) => {
  const { IDBObjectStore } = await import("fake-indexeddb");
  const store = await LocalLibrary.open("read-abort-" + crypto.randomUUID());
  try {
    const record = { id: id(1), completed: false, page: 7, updated_at: 1 };
    await store.write("chapters", record);
    const open = IDBObjectStore.prototype.openCursor;
    const fault = t.mock.method(
      IDBObjectStore.prototype,
      "openCursor",
      function (...args) {
        const query = open.apply(this, args);
        query.addEventListener("success", () => {
          if (!query.result) this.transaction.abort();
        });
        return query;
      },
    );
    await assert.rejects(store.snapshot(), /abort/i);
    fault.mock.restore();
    assert.deepEqual((await store.snapshot()).records.chapters, [record]);
  } finally {
    store.close();
  }
});
