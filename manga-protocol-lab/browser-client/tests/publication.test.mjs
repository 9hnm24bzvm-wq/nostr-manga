import "fake-indexeddb/auto";
import test from "node:test";
import assert from "node:assert/strict";
import { LocalLibrary } from "../src/store.js";
import { buildGraph } from "../src/codec.js";
import { queueSnapshot } from "../src/publishing.js";
import { createSigner } from "./fixtures/signer.mjs";
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: { locks: { request: async (_name, opts, fn) => (fn ?? opts)({}) } },
});
const relay = {
  origin: "https://relay.test",
  serviceKey: "22".repeat(32),
  selected: true,
};
const prepare = async (store) => {
  const snapshot = await store.snapshot();
  return { revision: snapshot.revision, graph: buildGraph(snapshot.records) };
};
test("forgetting an instance while signing prevents the late signature from creating a job", async () => {
  const store = await LocalLibrary.open("forget-" + crypto.randomUUID()),
    signer = createSigner();
  let release, event;
  try {
    await store.setRelays([relay]);
    const pending = queueSnapshot(
      store,
      {
        signEvent: (e) => {
          event = e;
          return new Promise((resolve) => {
            release = resolve;
          });
        },
      },
      signer.pub,
      await prepare(store),
      [relay],
      true,
    );
    for (let n = 0; !release && n < 100; n++) await new Promise(setImmediate);
    assert.ok(release);
    await store.setRelays([]);
    const rejected = assert.rejects(pending, /changed.*signing/i);
    release(await signer.signEvent(event));
    await rejected;
    assert.deepEqual(await store.jobs(), []);
  } finally {
    store.close();
  }
});

test("switching identities and deleting retry records cannot reset the next signed publication", async (t) => {
  t.mock.method(Date, "now", () => 1800000000000);
  const name = "clock-" + crypto.randomUUID(),
    a = createSigner(1),
    b = createSigner(2);
  let store = await LocalLibrary.open(name);
  try {
    const snapshot = await prepare(store);
    const first = await queueSnapshot(store, a, a.pub, snapshot, [relay], true);
    await store.deleteJob(first.id);
    const other = await queueSnapshot(store, b, b.pub, snapshot, [relay], true);
    await store.deleteJob(other.id);
    store.close();
    store = await LocalLibrary.open(name);
    const next = await queueSnapshot(store, a, a.pub, snapshot, [relay], true);
    assert.equal(next.event.created_at, 1800000001);
    assert.notEqual(next.id, first.id);
  } finally {
    store.close();
  }
});

test("verified recovery advances later events while stale recovery and backup fields cannot alter that clock", async (t) => {
  t.mock.method(Date, "now", () => 1800000000000);
  const store = await LocalLibrary.open("recovery-" + crypto.randomUUID()),
    signer = createSigner();
  try {
    const original = await store.snapshot();
    await store.replace(original, 0, {
      owner: signer.pub,
      created_at: 1800000020,
      id: "33".repeat(32),
    });
    const next = await queueSnapshot(
      store,
      signer,
      signer.pub,
      await prepare(store),
      [relay],
      true,
    );
    assert.equal(next.event.created_at, 1800000021);
    await assert.rejects(
      store.replace(original, 0, {
        owner: signer.pub,
        created_at: 1800000030,
        id: "44".repeat(32),
      }),
      /changed/,
    );
    const snapshot = await store.snapshot();
    snapshot.publicationClock = { owner: signer.pub, created_at: 9999999999 };
    await store.replace(snapshot, snapshot.revision);
    const later = await queueSnapshot(
      store,
      signer,
      signer.pub,
      await prepare(store),
      [relay],
      true,
    );
    assert.equal(later.event.created_at, 1800000022);
  } finally {
    store.close();
  }
});

test("forget and key rotation cancel prior snapshots across connections despite stale progress saves", async () => {
  const name = "preferences-" + crypto.randomUUID();
  const a = await LocalLibrary.open(name),
    b = await LocalLibrary.open(name),
    signer = createSigner();
  try {
    for (const next of [[], [{ ...relay, serviceKey: "33".repeat(32) }]]) {
      await a.setRelays([relay]);
      const job = await queueSnapshot(
        a,
        signer,
        signer.pub,
        await prepare(a),
        [relay],
        true,
      );
      const stale = await a.job(job.id);
      await b.setRelays(next);
      await a.saveJob(stale);
      assert.equal((await a.job(job.id)).cancelled, true);
      await assert.rejects(a.jobBatch(job.id, 0), /cancelled/);
      await a.deleteJob(job.id);
    }
  } finally {
    a.close();
    b.close();
  }
});

test("version-one publication history still orders later events after switching identity and removing retries", async (t) => {
  t.mock.method(Date, "now", () => 1800000000000);
  const { legacyLibrary } = await import("./fixtures/legacy-library.mjs");
  const a = createSigner(1),
    b = createSigner(2),
    name = "clock-migration-" + crypto.randomUUID();
  await legacyLibrary(name, {
    metadata: {
      lastPublication: {
        owner: a.pub,
        created_at: 1800000020,
        id: "aa".repeat(32),
      },
    },
  });
  const store = await LocalLibrary.open(name);
  try {
    const other = await queueSnapshot(
      store,
      b,
      b.pub,
      await prepare(store),
      [relay],
      true,
    );
    await store.deleteJob(other.id);
    const next = await queueSnapshot(
      store,
      a,
      a.pub,
      await prepare(store),
      [relay],
      true,
    );
    assert.equal(next.event.created_at, 1800000021);
  } finally {
    store.close();
  }
});

test("a publication approved after the signer timeout never enters the durable outbox", async (t) => {
  const signer = createSigner(),
    store = await LocalLibrary.open("late-publication-" + crypto.randomUUID());
  let release, template;
  t.mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const pending = queueSnapshot(
      store,
      {
        signEvent: (event) => {
          template = event;
          return new Promise((resolve) => {
            release = resolve;
          });
        },
      },
      signer.pub,
      await prepare(store),
      [relay],
      true,
    );
    for (let n = 0; !release && n < 100; n++) await new Promise(setImmediate);
    assert.ok(release);
    const rejected = assert.rejects(pending, /timed out/i);
    t.mock.timers.tick(60000);
    await rejected;
    release(await signer.signEvent(template));
    await new Promise(setImmediate);
    assert.deepEqual(await store.jobs(), []);
  } finally {
    store.close();
  }
});
