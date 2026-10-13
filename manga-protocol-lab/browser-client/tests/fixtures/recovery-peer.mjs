import { createHash } from "node:crypto";
import { buildGraph } from "../../src/codec.js";
import { createSigner } from "./signer.mjs";
const bytes = (value) => new TextEncoder().encode(JSON.stringify(value));
const digest = (value) => createHash("sha256").update(value).digest("hex");
const now = () => Math.floor(Date.now() / 1000);
const deferred = () => {
  let resolve;
  const promise = new Promise((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
// A signed external HTTP peer with one deferred chunk. No application method is
// replaced: actual service, HTTP, head, hash and graph validation all execute.
export async function recoveryPeer() {
  const user = createSigner(2),
    service = createSigner(11);
  const relay = {
    origin: "https://recovery.test",
    serviceKey: service.pub,
    selected: true,
  };
  const records = {
    chapters: [
      { id: "07".repeat(32), completed: false, page: 3, updated_at: 42 },
    ],
    series: [],
    lists: [],
    members: [],
  };
  const graph = buildGraph(records),
    rootHash = digest(graph.root.data);
  const event = await user.signEvent({
    kind: 11999,
    created_at: now() - 1,
    tags: [],
    content: JSON.stringify({ manifest: rootHash, version: 1 }),
  });
  const waiting = deferred(),
    resume = deferred();
  return {
    user,
    relay,
    records,
    event,
    waiting: waiting.promise,
    release: resume.resolve,
    async fetch(url, init) {
      const location = new URL(url);
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
            ["status", "200"],
            ["payload", digest(body)],
          ],
        });
        return new Response(body, {
          headers: {
            "x-manga-response-proof":
              "Nostr " + Buffer.from(bytes(proof)).toString("base64"),
          },
        });
      }
      const hash = location.pathname.slice("/library/blobs/".length);
      if (hash === rootHash) return new Response(graph.root.data);
      const object = graph.objects.find((object) => object.hash === hash);
      if (!object) return new Response("missing", { status: 404 });
      waiting.resolve();
      await resume.promise;
      return new Response(object.data);
    },
  };
}
