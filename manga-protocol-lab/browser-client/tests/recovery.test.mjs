import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { buildGraph, decode, encode } from "../src/codec.js";
import { recoverPublic } from "../src/publishing.js";
import { createSigner } from "./fixtures/signer.mjs";

const bytes = (value) => new TextEncoder().encode(JSON.stringify(value));
const digest = (value) => createHash("sha256").update(value).digest("hex");
const now = () => Math.floor(Date.now() / 1000);
const records = {
  chapters: [
    { id: "07".repeat(32), completed: false, page: 3, updated_at: 42 },
  ],
  series: [],
  lists: [{ id: "08".repeat(16), name: "Recovery fixture", updated_at: 41 }],
  members: [],
};

// The transport is an external peer adapter. All signatures and response scope
// tags are real, so a graph/head error cannot be masked by a broken receipt.
async function peer(
  t,
  { head = {}, author, root, rootReply, objects, responseStatus = 200 } = {},
) {
  const user = createSigner(2),
    service = createSigner(11);
  const graph = buildGraph(records);
  root ??= graph.root.data;
  objects ??= new Map(graph.objects.map((item) => [item.hash, item.data]));
  const rootHash = digest(root);
  const content = JSON.stringify({ manifest: rootHash, version: 1 });
  const event = await (author ?? user).signEvent({
    kind: 11999,
    created_at: now() - 1,
    tags: [],
    content,
    ...head,
  });
  const relay = { origin: "https://recovery.test", serviceKey: service.pub };
  const requested = [];
  t.mock.method(globalThis, "fetch", async (url, init) => {
    const location = new URL(url);
    requested.push(location.pathname);
    if (location.pathname === "/identity") {
      const identity = await service.signEvent({
        kind: 20005,
        created_at: now(),
        content: "",
        tags: [
          ["nonce", location.searchParams.get("nonce")],
          ["endpoint", relay.origin],
        ],
      });
      return new Response(bytes(identity));
    }
    if (location.pathname === "/libraries/" + user.pub) {
      const request = JSON.parse(
        Buffer.from(init.headers.authorization.slice(6), "base64"),
      );
      const body = bytes({ event, projected: true });
      const proof = await service.signEvent({
        kind: 20006,
        created_at: now(),
        content: "",
        tags: [
          ["protocol", "manga-federation-response-v1"],
          ["u", url],
          ["method", "GET"],
          ["request", request.id],
          ["status", String(responseStatus)],
          ["payload", digest(body)],
        ],
      });
      return new Response(body, {
        status: responseStatus,
        headers: {
          "x-manga-response-proof":
            "Nostr " + Buffer.from(bytes(proof)).toString("base64"),
        },
      });
    }
    const hash = location.pathname.slice("/library/blobs/".length);
    if (hash === rootHash)
      return rootReply === null
        ? new Response("missing", { status: 404 })
        : new Response(rootReply ?? root);
    const body = objects.get(hash);
    return body ? new Response(body) : new Response("missing", { status: 404 });
  });
  return {
    run: () => recoverPublic(relay, user, user.pub),
    requested,
    event,
    graph,
  };
}

test("public recovery accepts the exact signed complete graph", async (t) => {
  const fixture = await peer(t);
  const result = await fixture.run();
  assert.deepEqual(result.event, fixture.event);
  assert.deepEqual(result.snapshot.records, records);
});

for (const [label, options] of [
  ["missing root", { rootReply: null }],
  ["corrupt root", { rootReply: new Uint8Array([1, 2, 3]) }],
  [
    "hash-valid malformed root",
    { root: encode({ type: "library-root", version: 1, families: [] }) },
  ],
])
  test("public recovery refuses " + label, async (t) => {
    const fixture = await peer(t, options);
    await assert.rejects(fixture.run());
    assert.equal(
      fixture.requested.filter((path) => path.startsWith("/library/blobs/"))
        .length,
      1,
    );
  });

for (const [label, options] of [
  ["different signed owner", { author: createSigner(3) }],
  ["different signed kind", { head: { kind: 1 } }],
  ["future signed head", { head: { created_at: now() + 120 } }],
  ["expiration tag", { head: { tags: [["expiration", "1"]] } }],
  ["malformed pointer JSON", { head: { content: "{" } }],
  [
    "unsupported pointer version",
    {
      head: {
        content: JSON.stringify({ manifest: "07".repeat(32), version: 2 }),
      },
    },
  ],
  [
    "additional pointer field",
    {
      head: {
        content: JSON.stringify({
          manifest: "07".repeat(32),
          version: 1,
          extra: true,
        }),
      },
    },
  ],
  [
    "invalid pointer hash",
    {
      head: { content: JSON.stringify({ manifest: "not-a-hash", version: 1 }) },
    },
  ],
])
  test(
    "public recovery refuses " + label + " before fetching objects",
    async (t) => {
      const fixture = await peer(t, options);
      await assert.rejects(fixture.run());
      assert.ok(
        fixture.requested.every((path) => !path.startsWith("/library/blobs/")),
      );
    },
  );

for (const missing of [false, true])
  test(
    "public recovery refuses " +
      (missing ? "missing" : "corrupt") +
      " referenced chunk",
    async (t) => {
      const graph = buildGraph(records);
      const objects = new Map(
        graph.objects.map((item) => [item.hash, item.data]),
      );
      if (missing) objects.delete(graph.objects[0].hash);
      else objects.set(graph.objects[0].hash, new Uint8Array([1, 2, 3]));
      const fixture = await peer(t, { objects });
      await assert.rejects(fixture.run(), /unavailable or corrupt/);
    },
  );

for (const size of ["oversized", "short"])
  test(
    "public recovery stops at the first " +
      size +
      " descriptor without fetching another chunk",
    async (t) => {
      const graph = buildGraph(records);
      const root = decode(graph.root.data);
      const descriptor = root.families[0].chunks.find(Boolean);
      descriptor[1] = size === "oversized" ? 1 : descriptor[1] + 1;
      const fixture = await peer(t, { root: encode(root) });
      await assert.rejects(fixture.run());
      const fetchedChunks = fixture.requested.filter((path) =>
        path.startsWith("/library/blobs/"),
      );
      assert.equal(
        fetchedChunks.length,
        2,
        "only the root and first malformed-size chunk may be requested",
      );
    },
  );
