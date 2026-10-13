import "fake-indexeddb/auto";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import { schnorr } from "@noble/curves/secp256k1.js";
import { hex, unhex, hash, buildGraph } from "../src/codec.js";
import { LocalLibrary } from "../src/store.js";
import { proveService, signedRequest, signExact } from "../src/nostr.js";
import {
  queueSnapshot,
  deliverJob,
  recoverPublic,
  cancelJob,
} from "../src/publishing.js";
const origin = process.env.BROWSER_TEST_ORIGIN,
  service = process.env.BROWSER_TEST_SERVICE;
assert.ok(origin && service);
if (!navigator.locks)
  Object.defineProperty(navigator, "locks", {
    value: { request: async (_name, options, fn) => (fn ?? options)({}) },
  }); // Node harness only; real browser Web Locks are separately required.
function signer(n) {
  const key = unhex(n.toString(16).padStart(64, "0")),
    pub = hex(schnorr.getPublicKey(key));
  return {
    pub,
    async signEvent(e) {
      e = { ...e, pubkey: pub };
      e.id = hash(
        new TextEncoder().encode(
          JSON.stringify([0, pub, e.created_at, e.kind, e.tags, e.content]),
        ),
      );
      e.sig = hex(schnorr.sign(unhex(e.id), key));
      return e;
    },
  };
}
const user = signer(2),
  admin = signer(1);
const store = await LocalLibrary.open("backend-" + crypto.randomUUID());
try {
  await store.write("chapters", {
    id: "01".repeat(32),
    completed: false,
    page: 7,
    updated_at: 1,
  });
  await store.write("lists", {
    id: "11".repeat(16),
    name: "Public test list",
    updated_at: 1,
  });
  await store.write("series", {
    id: "02".repeat(32),
    status: "reading",
    updated_at: 1,
  });
  await store.write("members", {
    list: "11".repeat(16),
    series: "02".repeat(32),
    position: 0,
    updated_at: 1,
  });
  const snap = await store.snapshot(),
    graph = buildGraph(snap.records),
    relay = await proveService(origin, service);
  await assert.rejects(
    proveService(origin, "22".repeat(32)),
    /identity mismatch/,
  );
  await assert.rejects(
    queueSnapshot(
      store,
      user,
      user.pub,
      { revision: snap.revision, graph },
      [relay],
      false,
    ),
    /consent/,
  );
  const job = await queueSnapshot(
    store,
    user,
    user.pub,
    { revision: snap.revision, graph },
    [relay],
    true,
  );
  const denied = await deliverJob(store, job.id, user, user.pub);
  assert.equal(denied.recipients[0].state, "rejected");
  assert.equal((await store.counts()).chapters, 1);
  const admission = await signedRequest(
    relay,
    admin,
    admin.pub,
    "/admin/accounts/" + user.pub,
    "POST",
    { allowed: true },
  );
  assert.equal(admission.status, 200);
  const reopened = await LocalLibrary.open(store.db.name);
  try {
    const delivered = await deliverJob(reopened, job.id, user, user.pub);
    assert.equal(
      delivered.recipients[0].state,
      "accepted",
      JSON.stringify(delivered.recipients),
    );
    assert.equal(delivered.event.id, job.event.id);
    let recovered;
    for (let i = 0; i < 30; i++) {
      try {
        recovered = await recoverPublic(relay, user, user.pub);
        break;
      } catch (e) {
        if (i === 29) throw e;
        await new Promise((r) => setTimeout(r, 50));
      }
    }
    assert.equal(recovered.event.id, job.id);
    assert.deepEqual(recovered.snapshot.records, snap.records);
    await reopened.write("chapters", {
      id: "01".repeat(32),
      completed: true,
      page: 9,
      updated_at: 2,
    });
    assert.equal((await reopened.jobs())[0].event.id, job.id);
    const old = await recoverPublic(relay, user, user.pub);
    assert.equal(
      old.snapshot.records.chapters[0].page,
      7,
      "local save must not publish",
    );
    let sent = 0;
    const originalFetch = globalThis.fetch;
    globalThis.fetch = async () => {
      sent++;
      throw Error("should not send");
    };
    try {
      await assert.rejects(
        signedRequest(
          relay,
          user,
          user.pub,
          "/libraries",
          "POST",
          { event: job.event },
          65536,
          async () => {
            throw Error("cancelled before send");
          },
        ),
        /cancelled before send/,
      );
      assert.equal(sent, 0);
    } finally {
      globalThis.fetch = originalFetch;
    }
    await fs.writeFile(
      "../results/rebuild-client/browser-client-backend.json",
      JSON.stringify(
        {
          passed: true,
          execution:
            "browser modules under Node with IndexedDB model; not a rendered-browser test",
          checks: [
            "real PostgreSQL/strfry",
            "identity pin rejects mismatch",
            "explicit consent required",
            "denied relay preserves local data",
            "durable job reopens and retries same event after admission",
            "server validates JS canonical chunk/root graph",
            "fresh authenticated response proofs",
            "verified recovery equals local snapshot",
            "subsequent local save does not publish",
            "cancelled request after signing does not transmit",
          ],
        },
        null,
        2,
      ),
    );
    console.log("Browser SDK actual-backend checks passed");
  } finally {
    reopened.close();
  }
} finally {
  store.close();
}
