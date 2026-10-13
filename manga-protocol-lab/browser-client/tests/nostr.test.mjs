import test from "node:test";
import assert from "node:assert/strict";
import { schnorr } from "@noble/curves/secp256k1.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { hex, unhex } from "../src/codec.js";
import { verifyEvent, signExact, origin } from "../src/nostr.js";
const key = unhex("1".padStart(64, "0")),
  pub = hex(schnorr.getPublicKey(key));
function sign(e) {
  e = { ...e, pubkey: pub };
  e.id = hex(
    sha256(
      new TextEncoder().encode(
        JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content]),
      ),
    ),
  );
  e.sig = hex(schnorr.sign(unhex(e.id), key));
  return e;
}
test("exact Nostr signatures and signer-request binding", async () => {
  const template = { kind: 11999, created_at: 1, tags: [], content: "{}" };
  const good = await signExact(
    { signEvent: async (x) => sign(x) },
    pub,
    template,
  );
  assert.ok(verifyEvent(good));
  assert.ok(!verifyEvent({ ...good, content: "changed" }));
  await assert.rejects(
    signExact(
      { signEvent: async (x) => sign({ ...x, content: "changed" }) },
      pub,
      template,
    ),
  );
});
test("endpoint normalization refuses credentials and insecure remote origins", () => {
  assert.equal(origin("http://127.0.0.1:8000/"), "http://127.0.0.1:8000");
  for (const s of [
    "http://example.com",
    "https://user:pass@example.com",
    "https://example.com/path",
    "https://example.com/?x=1",
  ])
    assert.throws(() => origin(s));
});

test("a supplied event cannot override the selected signing identity", async () => {
  const { createSigner } = await import("./fixtures/signer.mjs");
  const a = createSigner(1),
    b = createSigner(2);
  let prompted = false;
  await assert.rejects(
    signExact(
      {
        signEvent: async (event) => {
          prompted = true;
          return a.signEvent(event);
        },
      },
      b.pub,
      { pubkey: a.pub, kind: 11999, created_at: 1, tags: [], content: "{}" },
    ),
    /identity|owner/i,
  );
  assert.equal(prompted, false);
});

test("timed-out HTTP signing ignores late approval and never transmits it", async (t) => {
  const { signedRequest } = await import("../src/nostr.js");
  const { createSigner } = await import("./fixtures/signer.mjs");
  const signer = createSigner();
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let release,
    template,
    failure,
    sent = 0;
  t.mock.method(globalThis, "fetch", async () => {
    sent++;
    throw Error("Unexpected transmission");
  });
  const pending = signedRequest(
    { origin: "https://relay.test", serviceKey: signer.pub },
    {
      signEvent: (event) => {
        template = event;
        return new Promise((resolve) => {
          release = resolve;
        });
      },
    },
    signer.pub,
    "/libraries",
  ).catch((error) => {
    failure = error;
  });
  try {
    await Promise.resolve();
    t.mock.timers.tick(60000);
    await new Promise(setImmediate);
    assert.match(failure?.message ?? "still pending", /timed out/i);
  } finally {
    release(await signer.signEvent(template));
    await pending;
  }
  assert.equal(sent, 0);
});

test("approval after the monotonic deadline is rejected even before a delayed timer fires", async (t) => {
  const { createSigner } = await import("./fixtures/signer.mjs");
  const signer = createSigner();
  let clock = 0,
    release,
    template;
  t.mock.method(performance, "now", () => clock);
  const pending = signExact(
    {
      signEvent: (event) => {
        template = event;
        return new Promise((resolve) => {
          release = resolve;
        });
      },
    },
    signer.pub,
    { kind: 11999, created_at: 1, tags: [], content: "{}" },
  );
  await new Promise(setImmediate);
  clock = 60001;
  const rejected = assert.rejects(pending, /timed out/i);
  release(await signer.signEvent(template));
  await rejected;
});
