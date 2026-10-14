import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { buildGraph } from "../src/codec.js";
import { deliverJob, graphInWorker, recoverPublic } from "../src/publishing.js";
import { signerRequest, verifyEvent } from "../src/nostr.js";
import { createSigner } from "./fixtures/signer.mjs";

const bytes = (value) => new TextEncoder().encode(JSON.stringify(value));
const digest = (value) => createHash("sha256").update(value).digest("hex");
const now = () => Math.floor(Date.now() / 1000);
const emptyRecords = { chapters: [], series: [], lists: [], members: [] };
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    locks: { request: async (_name, options, fn) => (fn ?? options)({}) },
  },
});

// Persistence is an external dependency of delivery; this adapter returns
// detached metadata just like IndexedDB. All protocol functions run unchanged.
function repository(event, relay, objects = []) {
  let saved = {
    id: event.id,
    event,
    revision: 0,
    createdAt: Date.now(),
    cancelled: false,
    objectCount: objects.length,
    recipients: [
      { ...relay, cursor: 0, state: "pending", message: "Not sent" },
    ],
  };
  return {
    async job() {
      return structuredClone(saved);
    },
    async saveJob(job) {
      saved = structuredClone(job);
    },
    async jobBatch(_id, start) {
      return { objects: objects.slice(start), next: objects.length };
    },
  };
}

async function publicationFixture(t, options = {}) {
  const user = createSigner(2),
    service = createSigner(11);
  const relay = { origin: "https://boundaries.test", serviceKey: service.pub };
  const graph = buildGraph(emptyRecords);
  const pointer = Object.hasOwn(options, "pointer")
    ? options.pointer
    : { manifest: graph.root.hash, version: 1 };
  const event = await user.signEvent({
    kind: 11999,
    created_at: now() - 1,
    tags: [],
    content: JSON.stringify(pointer),
  });
  const paths = [];
  t.mock.method(globalThis, "fetch", async (url, init) => {
    const location = new URL(url);
    paths.push(location.pathname);
    if (location.pathname === "/identity") {
      return new Response(
        bytes(
          await service.signEvent({
            kind: 20005,
            created_at: now(),
            content: "",
            tags: [
              ["nonce", location.searchParams.get("nonce")],
              ["endpoint", relay.origin],
            ],
          }),
        ),
      );
    }
    if (location.pathname.startsWith("/library/blobs/")) {
      assert.equal(location.pathname, "/library/blobs/" + graph.root.hash);
      return new Response(graph.root.data);
    }
    const request = JSON.parse(
      Buffer.from(init.headers.authorization.slice(6), "base64"),
    );
    let payload, status;
    if (location.pathname === "/library/batches") {
      payload = Object.hasOwn(options, "stage")
        ? options.stage
        : { staged: true, hashes: graph.objects.map((o) => o.hash) };
      status = options.stageStatus ?? 200;
    } else if (location.pathname === "/libraries") {
      payload = Object.hasOwn(options, "receipt")
        ? options.receipt
        : { id: event.id, projected: true };
      if (typeof payload === "function") payload = payload(event);
      status = options.status ?? 200;
    } else {
      assert.equal(location.pathname, "/libraries/" + user.pub);
      payload = Object.hasOwn(options, "headBody")
        ? options.headBody
        : { event, projected: true };
      status = 200;
    }
    const body = bytes(payload);
    const proof = await service.signEvent({
      kind: 20006,
      created_at: now(),
      content: "",
      tags: [
        ["protocol", "manga-federation-response-v1"],
        ["u", url],
        ["method", init.method],
        ["request", request.id],
        ["status", String(status)],
        ["payload", digest(body)],
      ],
    });
    return new Response(body, {
      status,
      headers: {
        "x-manga-response-proof":
          "Nostr " + Buffer.from(bytes(proof)).toString("base64"),
      },
    });
  });
  const store = repository(
    event,
    relay,
    options.withObjects ? graph.objects : [],
  );
  return {
    user,
    relay,
    graph,
    event,
    paths,
    store,
    run: () => deliverJob(store, event.id, user, user.pub),
  };
}

test("generic signer requests preserve synchronous and asynchronous values", async () => {
  assert.equal(await signerRequest(() => "owner"), "owner");
  const value = { selected: true };
  assert.equal(await signerRequest(async () => value), value);
});

test("event verification accepts valid events and refuses unknown input", async () => {
  const signer = createSigner();
  const event = await signer.signEvent({
    kind: 11999,
    created_at: now(),
    tags: [],
    content: "{}",
  });
  assert.equal(verifyEvent(event), true);
  for (const value of [
    null,
    undefined,
    "private response",
    5,
    {},
    { ...event, tags: [[1]] },
  ]) {
    assert.equal(verifyEvent(value), false);
  }
});

for (const [label, tags] of [
  ["sparse outer tags", new Array(1)],
  ["sparse inner tag", [new Array(1)]],
]) {
  test(
    "event verification rejects signed " + label + " in process and on wire",
    async () => {
      const signer = createSigner();
      const event = await signer.signEvent({
        kind: 11999,
        created_at: now(),
        tags,
        content: "{}",
      });
      assert.equal(verifyEvent(event), false);
      assert.equal(verifyEvent(JSON.parse(JSON.stringify(event))), false);
    },
  );
}

test("event verification preserves dense empty tag lists and empty tags", async () => {
  const signer = createSigner();
  for (const tags of [[], [[]]]) {
    const event = await signer.signEvent({
      kind: 11999,
      created_at: now(),
      tags,
      content: "{}",
    });
    assert.equal(verifyEvent(event), true);
    assert.equal(verifyEvent(JSON.parse(JSON.stringify(event))), true);
  }
});

for (const [label, failure, state, message] of [
  ["null", null, "retryable", "Publication request failed"],
  ["string", "private response", "retryable", "Publication request failed"],
  ["Error", Error("Transport failed"), "retryable", "Transport failed"],
  [
    "Error with forbidden status",
    Object.assign(Error("Forbidden"), { status: 403 }),
    "rejected",
    "Forbidden",
  ],
  [
    "object with numeric status",
    { status: 401, message: "private response" },
    "rejected",
    "Publication request failed",
  ],
  [
    "object with retry status",
    { status: 500 },
    "retryable",
    "Publication request failed",
  ],
  [
    "object with string status",
    { status: "403" },
    "retryable",
    "Publication request failed",
  ],
]) {
  test("delivery safely classifies a " + label + " rejection", async (t) => {
    const fixture = await publicationFixture(t);
    t.mock.method(globalThis, "fetch", async () => {
      throw failure;
    });
    const result = await fixture.run();
    assert.equal(result.recipients[0].state, state);
    assert.equal(result.recipients[0].message, message);
    assert.deepEqual(await fixture.store.job(), result);
  });
}

for (const payload of [
  null,
  "private response",
  3,
  [],
  { staged: "true", hashes: [] },
]) {
  test(
    "delivery refuses malformed signed staging payload " +
      JSON.stringify(payload),
    async (t) => {
      const fixture = await publicationFixture(t, {
        stage: payload,
        withObjects: true,
      });
      const result = await fixture.run();
      assert.equal(result.recipients[0].state, "retryable");
      assert.equal(result.recipients[0].message, "Invalid staging receipt");
      assert.equal(result.recipients[0].cursor, 0);
      assert.ok(!fixture.paths.includes("/libraries"));
    },
  );
}

for (const payload of [null, "private response", 3, []]) {
  test(
    "delivery refuses malformed signed publication payload " +
      JSON.stringify(payload),
    async (t) => {
      const fixture = await publicationFixture(t, { receipt: payload });
      const result = await fixture.run();
      assert.equal(result.recipients[0].state, "retryable");
      assert.match(
        result.recipients[0].message,
        /Invalid publication receipt|Wrong publication receipt/,
      );
      assert.ok(!result.recipients[0].message.includes("private response"));
    },
  );
}

for (const [status, receipt, state, projected] of [
  [200, (event) => ({ id: event.id, projected: true }), "accepted", true],
  [202, (event) => ({ id: event.id, projected: false }), "accepted", false],
  [
    409,
    (event) => ({ id: event.id, superseded: true }),
    "superseded",
    undefined,
  ],
  [403, "private response", "rejected", undefined],
  [500, null, "retryable", undefined],
]) {
  test(
    "delivery preserves signed HTTP " + status + " receipt outcomes",
    async (t) => {
      const fixture = await publicationFixture(t, {
        status,
        receipt,
        withObjects: true,
      });
      const result = await fixture.run();
      assert.equal(result.recipients[0].state, state);
      assert.equal(result.recipients[0].projected, projected);
      assert.equal(result.recipients[0].cursor, fixture.graph.objects.length);
      assert.ok(fixture.paths.includes("/libraries"));
      assert.ok(!result.recipients[0].message.includes("private response"));
    },
  );
}

for (const pointer of [
  null,
  "private response",
  3,
  [],
  { manifest: 4, version: 1 },
]) {
  test(
    "recovery refuses malformed signed pointer " + JSON.stringify(pointer),
    async (t) => {
      const fixture = await publicationFixture(t, { pointer });
      await assert.rejects(
        recoverPublic(fixture.relay, fixture.user, fixture.user.pub),
        /Invalid library pointer/,
      );
      assert.ok(
        fixture.paths.every((path) => !path.startsWith("/library/blobs/")),
      );
    },
  );
}
for (const headBody of [null, "private response", 3, []]) {
  test(
    "recovery refuses malformed signed head payload " +
      JSON.stringify(headBody),
    async (t) => {
      const fixture = await publicationFixture(t, { headBody });
      await assert.rejects(
        recoverPublic(fixture.relay, fixture.user, fixture.user.pub),
        /Invalid public library head/,
      );
      assert.ok(
        fixture.paths.every((path) => !path.startsWith("/library/blobs/")),
      );
    },
  );
}

test("recovery accepts a legitimate signed empty-library graph", async (t) => {
  const fixture = await publicationFixture(t);
  const recovered = await recoverPublic(
    fixture.relay,
    fixture.user,
    fixture.user.pub,
  );
  assert.deepEqual(recovered.event, fixture.event);
  assert.deepEqual(recovered.snapshot.records, emptyRecords);
});

function workerFixture(t, payload) {
  let worker;
  class Adapter {
    constructor() {
      worker = this;
      this.terminated = false;
    }
    postMessage(value) {
      this.request = value;
      queueMicrotask(() => this.onmessage({ data: payload }));
    }
    terminate() {
      this.terminated = true;
    }
  }
  const original = Object.getOwnPropertyDescriptor(globalThis, "Worker");
  Object.defineProperty(globalThis, "Worker", {
    configurable: true,
    writable: true,
    value: Adapter,
  });
  t.after(() => {
    if (original) Object.defineProperty(globalThis, "Worker", original);
    else delete globalThis.Worker;
  });
  return () => worker;
}

for (const [label, payload, message] of [
  ["null", null, /Invalid snapshot preparation response/],
  ["string", "private response", /Invalid snapshot preparation response/],
  ["empty error", { error: "" }, /Snapshot preparation failed/],
  [
    "nonstring error",
    { error: { private: "response" } },
    /Snapshot preparation failed/,
  ],
  ["missing graph", { revision: 0 }, /Invalid snapshot preparation response/],
  [
    "mixed response",
    { revision: 0, graph: buildGraph(emptyRecords), error: "failed" },
    /Invalid snapshot preparation response/,
  ],
  [
    "invalid revision",
    { revision: "0", graph: buildGraph(emptyRecords) },
    /Invalid snapshot preparation response/,
  ],
  [
    "shadowed inventory array method",
    structuredClone({
      revision: 0,
      graph: {
        ...buildGraph(emptyRecords),
        objects: Object.assign(buildGraph(emptyRecords).objects, {
          every: null,
        }),
      },
    }),
    /Invalid snapshot preparation response/,
  ],
  [
    "sparse object inventory",
    structuredClone({
      revision: 0,
      graph: { ...buildGraph(emptyRecords), objects: new Array(1) },
    }),
    /Invalid snapshot preparation response/,
  ],
  [
    "malformed graph",
    { revision: 0, graph: { root: null, objects: [], records: 0 } },
    /Invalid snapshot preparation response/,
  ],
]) {
  test(
    "snapshot worker rejects " + label + " without waiting for timeout",
    async (t) => {
      const current = workerFixture(t, payload);
      await assert.rejects(graphInWorker("fixture-database"), message);
      assert.equal(current().terminated, true);
      assert.deepEqual(current().request, { database: "fixture-database" });
    },
  );
}

test("snapshot worker accepts only its valid success shape", async (t) => {
  const payload = { revision: 3, graph: buildGraph(emptyRecords) };
  const current = workerFixture(t, payload);
  assert.equal(await graphInWorker("fixture-database"), payload);
  assert.equal(current().terminated, true);
});
