// Opt-in public backup round trip. This is a resource-heavy model test, not a
// real-browser storage/quota guarantee. Keep it separate from the fast gate.
import "fake-indexeddb/auto";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { LocalLibrary } from "../src/store.js";
import { backupBlob, importBackup } from "../src/backup.js";
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    locks: {
      request: async (_name, options, operation) => (operation ?? options)({}),
    },
  },
});
const count = 500000,
  partLimit = 65536,
  started = Date.now(),
  NativeBlob = Blob;
const report = (phase, details = {}) =>
  console.log(
    JSON.stringify({
      phase,
      seconds: Math.round((Date.now() - started) / 100) / 10,
      rssMiB: Math.round(process.memoryUsage().rss / 1024 ** 2),
      ...details,
    }),
  );
const sourceHashes = {};
for (const filename of (
  await readdir(new URL("../src/", import.meta.url))
).sort()) {
  if (filename.endsWith(".js"))
    sourceHashes[filename] = createHash("sha256")
      .update(await readFile(new URL("../src/" + filename, import.meta.url)))
      .digest("hex");
}
sourceHashes["tests/backup-large.mjs"] = createHash("sha256")
  .update(await readFile(new URL(import.meta.url)))
  .digest("hex");
report("sources", sourceHashes);
const expected = (index) => ({
  id: index.toString(16).padStart(32, "0"),
  name: "🐈".repeat(128),
  updated_at: 1,
});
const parts = [];
let buffer = "";
const append = (value) => {
  buffer += JSON.stringify(value) + "\n";
  if (buffer.length >= 12000) {
    parts.push(new NativeBlob([buffer]));
    buffer = "";
  }
};
append({ format: "manga-local-library", version: 2 });
for (let index = 0; index < count; index++) append(["lists", expected(index)]);
append({ end: count });
if (buffer) parts.push(new NativeBlob([buffer]));
const file = new NativeBlob(parts);
parts.length = 0;
buffer = "";
assert.ok(file.size > 256 * 1024 ** 2);
report("input-ready", { records: count, bytes: file.size });
let nextProgress = 50 * 1024 ** 2;
const boundedFile = {
  size: file.size,
  slice(start, end) {
    assert.ok(end - start <= partLimit, "import reads bounded File slices");
    if (start >= nextProgress) {
      report("import-read", { bytes: start });
      nextProgress += 50 * 1024 ** 2;
    }
    return file.slice(start, end);
  },
  text() {
    throw Error("Version-two import must not read a whole file as text");
  },
};
let exportedBytes = 0,
  nextExportProgress = 50 * 1024 ** 2;
globalThis.Blob = class extends NativeBlob {
  constructor(parts = [], options) {
    for (const part of parts) {
      const size =
        part instanceof NativeBlob
          ? part.size
          : typeof part === "string"
            ? Buffer.byteLength(part)
            : part.byteLength;
      assert.ok(size <= partLimit, "export constructs bounded Blob parts");
      if (!(part instanceof NativeBlob)) exportedBytes += size;
      if (exportedBytes >= nextExportProgress) {
        report("export-built", { bytes: exportedBytes });
        nextExportProgress += 50 * 1024 ** 2;
      }
    }
    super(parts, options);
  }
};
const store = await LocalLibrary.open("large-backup-" + crypto.randomUUID());
try {
  await store.write("chapters", {
    id: "11".repeat(32),
    completed: false,
    page: 7,
  });
  await importBackup(store, boundedFile, 1);
  assert.deepEqual(await store.counts(), {
    chapters: 0,
    series: 0,
    lists: count,
    members: 0,
    revision: 2,
    total: count,
    recordLimit: 2000000,
    overBudget: false,
  });
  report("import-complete");
  const exported = await backupBlob(store);
  assert.ok(exported.size > 256 * 1024 ** 2);
  report("export-complete", { bytes: exported.size });
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let pending = "",
    index = -1,
    footer = false;
  for await (const bytes of exported.stream()) {
    pending += decoder.decode(bytes, { stream: true });
    for (;;) {
      const end = pending.indexOf("\n");
      if (end < 0) break;
      const row = pending.slice(0, end);
      pending = pending.slice(end + 1);
      assert.ok(Buffer.byteLength(row) + 1 <= partLimit);
      const value = JSON.parse(row);
      if (index === -1) {
        assert.equal(value.format, "manga-local-library");
        assert.equal(value.version, 2);
        index = 0;
      } else if (index < count) {
        assert.deepEqual(value, ["lists", expected(index)]);
        index++;
      } else {
        assert.deepEqual(value, { end: count });
        assert.equal(footer, false);
        footer = true;
      }
    }
  }
  assert.equal(pending + decoder.decode(), "");
  assert.equal(index, count);
  assert.equal(footer, true);
  report("passed", { records: count, boundedPartBytes: partLimit });
} finally {
  store.close();
  globalThis.Blob = NativeBlob;
}
