import "fake-indexeddb/auto";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
import { versionThreeLibrary } from "./fixtures/version-three-library.mjs";
const record = { id: "11".repeat(32), completed: true, page: 7, updated_at: 1 };
const event = { pubkey: "22".repeat(32), created_at: 9 };
const fixture = () => ({
  records: [{ family: "chapters", key: record.id, record }],
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

test("a blocked upgrade rejects with close/reload guidance and succeeds after the old tab closes", async () => {
  const name = "blocked-" + crypto.randomUUID();
  const old = await versionThreeLibrary(name, fixture());
  const opening = LocalLibrary.open(name);
  opening.then(
    (store) => store.close(),
    () => {},
  );
  let timer;
  try {
    await assert.rejects(
      Promise.race([
        opening,
        new Promise((_, reject) => {
          timer = setTimeout(
            () => reject(Error("Open remained pending without guidance")),
            200,
          );
        }),
      ]),
      /blocked.*close.*reload/i,
    );
  } finally {
    clearTimeout(timer);
    old.close();
  }
  const store = await LocalLibrary.open(name);
  try {
    assert.deepEqual(await store.list("chapters"), [record]);
    assert.deepEqual((await store.job("pending")).event, event);
    assert.deepEqual(
      [...(await store.jobBatch("pending", 0)).objects[0].data],
      [0, 127, 255],
    );
    assert.equal((await store.counts()).total, 1);
  } finally {
    store.close();
  }
});

test("a cooperating older connection finishes admitted writes before capacity upgrade and keeps queued bytes", async () => {
  const name = "upgrade-" + crypto.randomUUID();
  const old = await versionThreeLibrary(name, fixture());
  old.onversionchange = () => old.close();
  const tx = old.transaction("records", "readwrite");
  const newer = { ...record, id: "33".repeat(32), page: 17 };
  tx.objectStore("records").put({
    family: "chapters",
    key: newer.id,
    record: newer,
  });
  const store = await LocalLibrary.open(name);
  try {
    assert.deepEqual(await store.list("chapters"), [record, newer]);
    assert.deepEqual((await store.job("pending")).event, event);
    assert.deepEqual(
      [...(await store.jobBatch("pending", 0)).objects[0].data],
      [0, 127, 255],
    );
    assert.equal((await store.counts()).total, 2);
    await store.write("chapters", { ...newer, page: 18 });
    assert.equal((await store.counts()).total, 2);
  } finally {
    store.close();
    old.close();
  }
});
