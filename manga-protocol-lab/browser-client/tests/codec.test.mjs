import test from "node:test";
import assert from "node:assert/strict";
import {
  encode,
  decode,
  buildGraph,
  readGraph,
  hash,
  normalize,
  unhex,
  hex,
  families,
} from "../src/codec.js";
const id = (n) => n.toString(16).padStart(64, "0");
test("canonical encoding and strict rejection", () => {
  assert.equal(hex(encode({ long: 2, a: 1 })), "a2616101646c6f6e6702");
  assert.deepEqual([...decode(encode(new Uint8Array([1, 2])))], [1, 2]);
  for (const b of [
    [0x18, 1],
    [0x9f, 0xff],
    [0xf9, 0, 0],
    [0xa2, 0x61, 0x61, 1, 0x61, 0x61, 2],
    [1, 2],
  ])
    assert.throws(() => decode(Uint8Array.from(b)));
});
test("round-trip all four families and hash/bucket integrity", () => {
  const records = {
    chapters: [{ id: id(1), completed: false, page: 3, updated_at: 1 }],
    series: [{ id: id(2), status: "reading", updated_at: 2 }],
    lists: [{ id: "11".repeat(16), name: "My public list", updated_at: 3 }],
    members: [
      { list: "11".repeat(16), series: id(2), position: 0, updated_at: 4 },
    ],
  };
  const g = buildGraph(records),
    objects = new Map(g.objects.map((o) => [o.hash, o.data]));
  assert.deepEqual(readGraph(g.root.data, objects), records);
  objects.set(g.objects[0].hash, encode({ bad: true }));
  assert.throws(() => readGraph(g.root.data, objects));
});
test("unknown local fields never reach public wire and invalid records fail", () => {
  const r = normalize("chapters", {
    id: id(1),
    completed: true,
    page: null,
    updated_at: 1,
    privateNote: "never publish",
  });
  assert.equal(r.privateNote, undefined);
  assert.throws(() =>
    normalize("lists", { id: "11".repeat(16), name: "bad\nname" }),
  );
  assert.throws(() => normalize("chapters", { id: id(1), completed: "yes" }));
  assert.throws(() => buildGraph({ chapters: [r, r] }));
});
test("30k and500k chapter libraries are complete and bounded", () => {
  for (const n of [30000, 500000]) {
    const chapters = Array.from({ length: n }, (_, i) => ({
      id: id(i + 1),
      completed: true,
      page: null,
      updated_at: 10,
    }));
    const g = buildGraph({ chapters });
    assert.equal(g.records, n);
    const decoded = readGraph(
      g.root.data,
      new Map(g.objects.map((o) => [o.hash, o.data])),
    );
    assert.equal(decoded.chapters.length, n);
    assert.equal(new Set(decoded.chapters.map((x) => x.id)).size, n);
    assert.deepEqual(
      decoded.chapters.sort((a, b) => a.id.localeCompare(b.id)),
      chapters,
    );
    assert.ok(g.root.data.length <= 262144);
    assert.ok(g.objects.every((x) => x.data.length <= 1000000));
  }
});
test("ill-formed Unicode names cannot change during UTF8 encoding", () => {
  assert.throws(() =>
    normalize("lists", { id: "11".repeat(16), name: "\ud800" }),
  );
  assert.equal(
    normalize("lists", { id: "11".repeat(16), name: "📚 Manga" }).name,
    "📚 Manga",
  );
});

test("wire tuples require byte-string identifiers and explicit integer timestamps", () => {
  const bytes = new Uint8Array(32).fill(1);
  // Assemble wire records independently of buildGraph/normalization. A one-bucket
  // graph admits this ID so only the tuple contract can reject these cases.
  const read = (tuple) => {
    const chunk = encode({
      type: "library-chunk",
      version: 1,
      family: "chapters",
      entries: [tuple],
    });
    const key = hash(chunk);
    const root = encode({
      type: "library-root",
      version: 1,
      families: families.map((family) => ({
        family,
        chunks:
          family === "chapters" ? [[unhex(key), chunk.length, 1]] : [null],
      })),
    });
    return readGraph(root, new Map([[key, chunk]])).chapters;
  };
  assert.throws(() => read([[...bytes], true, null, 1]));
  assert.throws(() => read([bytes, true, null, null]));
  assert.deepEqual(read([bytes, true, null, 0]), [
    { id: "01".repeat(32), completed: true, page: null, updated_at: 0 },
  ]);
});

test("valid maximum-width list names are partitioned into legal wire objects", () => {
  const lists = Array.from({ length: 2300 }, (_, n) => ({
    id: n.toString(16).padStart(32, "0"),
    name: "🐈".repeat(128),
    updated_at: 1,
  }));
  const graph = buildGraph({ lists });
  assert.ok(graph.objects.every((object) => object.data.length <= 1000000));
  const recovered = readGraph(
    graph.root.data,
    new Map(graph.objects.map((object) => [object.hash, object.data])),
  );
  assert.equal(recovered.lists.length, 2300);
  const sorted = recovered.lists.sort((a, b) => a.id.localeCompare(b.id));
  assert.deepEqual(sorted, lists);
});
