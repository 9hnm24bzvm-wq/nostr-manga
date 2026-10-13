import "fake-indexeddb/auto";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";

test("a delayed publisher cannot recreate a cancelled and removed snapshot", async () => {
  const store = await LocalLibrary.open("removed-" + crypto.randomUUID());
  try {
    await store.createJob({
      id: "snapshot",
      revision: 0,
      event: { pubkey: "11".repeat(32), created_at: 1 },
      cancelled: false,
      recipients: [{ state: "pending" }],
      objects: [],
    });
    const delayed = await store.job("snapshot");
    const cancelled = await store.job("snapshot");
    cancelled.cancelled = true;
    await store.saveJob(cancelled);
    await store.deleteJob("snapshot");
    await assert.rejects(store.saveJob(delayed), /removed|missing/i);
    assert.deepEqual(await store.jobs(), []);
  } finally {
    store.close();
  }
});

test("queued immutable bytes survive progress changes and reopen through bounded continuations", async () => {
  const name = "bounded-" + crypto.randomUUID();
  let store = await LocalLibrary.open(name);
  const event = { pubkey: "11".repeat(32), created_at: 1 };
  try {
    for (const size of [100000, 8]) {
      await store.createJob({
        id: "bytes",
        revision: 0,
        event,
        cancelled: false,
        recipients: [
          {
            origin: "https://relay.test",
            serviceKey: "22".repeat(32),
            cursor: 0,
          },
        ],
        objects: Array.from({ length: 70 }, (_, index) => ({
          hash: String(index),
          type: "chunk",
          data: new Uint8Array(size).fill(index),
        })),
      });
      const progress = await store.job("bytes");
      assert.ok(
        !Object.hasOwn(progress, "objects"),
        "listing job state does not return its byte inventory",
      );
      progress.event = { pubkey: "bad" };
      progress.revision = 99;
      progress.objects = [];
      progress.recipients[0].cursor = 5;
      await store.saveJob(progress);
      store.close();
      store = await LocalLibrary.open(name);
      const restored = await store.job("bytes");
      assert.deepEqual(restored.event, event);
      assert.equal(restored.revision, 0);
      assert.equal(restored.recipients[0].cursor, 5);
      let cursor = 0,
        seen = 0;
      for (;;) {
        const batch = await store.jobBatch("bytes", cursor);
        assert.ok(batch.objects.length <= 64);
        assert.ok(
          batch.objects.reduce((n, o) => n + o.data.length, 0) <=
            4 * 1024 * 1024,
        );
        if (!batch.objects.length) break;
        assert.ok(batch.next > cursor);
        for (const object of batch.objects) {
          assert.equal(object.hash, String(seen));
          assert.equal(
            Buffer.compare(Buffer.from(object.data), Buffer.alloc(size, seen)),
            0,
          );
          seen++;
        }
        cursor = batch.next;
      }
      assert.equal(seen, 70);
      await store.deleteJob("bytes");
      await assert.rejects(store.jobBatch("bytes", 0), /missing/);
    }
  } finally {
    store.close();
  }
});

test("version-one reading data and queued bytes remain available after upgrade", async () => {
  const { legacyLibrary } = await import("./fixtures/legacy-library.mjs");
  const name = "legacy-" + crypto.randomUUID();
  const record = {
    id: "11".repeat(32),
    completed: true,
    page: 7,
    updated_at: 1,
  };
  const event = { pubkey: "22".repeat(32), created_at: 1 };
  await legacyLibrary(name, {
    records: [{ family: "chapters", key: record.id, record }],
    metadata: { revision: 1 },
    jobs: [
      {
        id: "old",
        revision: 1,
        event,
        cancelled: false,
        recipients: [],
        objects: [
          { type: "chunk", hash: "fixture", data: new Uint8Array([1, 2, 3]) },
        ],
      },
    ],
  });
  const store = await LocalLibrary.open(name);
  try {
    assert.deepEqual(await store.list("chapters"), [record]);
    assert.deepEqual((await store.job("old")).event, event);
    assert.ok(!Object.hasOwn(await store.job("old"), "objects"));
    assert.deepEqual(
      [...(await store.jobBatch("old", 0)).objects[0].data],
      [1, 2, 3],
    );
  } finally {
    store.close();
  }
});
