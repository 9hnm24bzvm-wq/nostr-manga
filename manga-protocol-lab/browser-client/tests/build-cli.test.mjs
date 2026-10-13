import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  copyFile,
  readdir,
  rm,
  rename,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import vm from "node:vm";
const run = promisify(execFile);
async function contents(directory, prefix = "") {
  const result = {};
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const name = prefix + entry.name,
      filename = path.join(directory, entry.name);
    if (entry.isDirectory())
      Object.assign(result, await contents(filename, name + "/"));
    else result[name] = await readFile(filename, "utf8");
  }
  return result;
}
test("build CLI preserves the last good output on failure and promotes only a clean deterministic build", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "manga-build-"));
  const inputs = {
    "package.json": '{"type":"module"}',
    "src/app.js":
      "import { value } from '@noble/hashes/sha2.js'; export const answer = value;",
    "index.html": "<title>Current client</title>",
    "style.css": "body{color:navy}",
    "node_modules/@noble/curves/LICENSE": "Curves license",
    "node_modules/@noble/curves/package.json": '{"type":"module"}',
    "node_modules/@noble/curves/secp256k1.js": "export const curve=true;",
    "node_modules/@noble/hashes/LICENSE": "Hashes license",
    "node_modules/@noble/hashes/package.json": '{"type":"module"}',
    "node_modules/@noble/hashes/sha2.js": "export const value='current';",
    "keep.txt": "outside output",
  };
  try {
    for (const [name, body] of Object.entries(inputs)) {
      await mkdir(path.dirname(path.join(root, name)), { recursive: true });
      await writeFile(path.join(root, name), body);
    }
    await copyFile(
      new URL("../build.mjs", import.meta.url),
      path.join(root, "build.mjs"),
    );
    const build = () =>
      run(process.execPath, [path.join(root, "build.mjs")], { cwd: tmpdir() });
    await build();
    const output = path.join(root, "public"),
      previous = await contents(output);
    await writeFile(
      path.join(root, "src/app.js"),
      'export const answer="changed";',
    );
    await rm(path.join(root, "style.css"));
    await assert.rejects(build());
    assert.deepEqual(
      await contents(output),
      previous,
      "a failed build must not modify the previously published tree",
    );
    // Documented recovery input: an interrupted promotion retained the old tree
    // under the generated previous-build name, with public temporarily absent.
    await rename(
      output,
      path.join(root, ".public-previous-00000000-0000-4000-8000-000000000001"),
    );
    await assert.rejects(build());
    assert.deepEqual(
      await contents(output),
      previous,
      "the next invocation restores the prior build before attempting new inputs",
    );
    await writeFile(path.join(root, "style.css"), inputs["style.css"]);
    await writeFile(path.join(root, "src/app.js"), inputs["src/app.js"]);
    await writeFile(path.join(output, "src/removed.js"), "obsolete");
    await writeFile(path.join(output, "vendor/hashes/removed.js"), "obsolete");
    await writeFile(path.join(output, "removed.txt"), "obsolete");
    const first = JSON.parse((await build()).stdout),
      clean = await contents(output);
    const expected = [
      "index.html",
      "style.css",
      "sw.js",
      "src/app.js",
      "vendor/curves/LICENSE",
      "vendor/curves/package.json",
      "vendor/curves/secp256k1.js",
      "vendor/hashes/LICENSE",
      "vendor/hashes/package.json",
      "vendor/hashes/sha2.js",
    ].sort();
    assert.deepEqual(Object.keys(clean).sort(), expected);
    assert.equal(
      await readFile(path.join(root, "keep.txt"), "utf8"),
      "outside output",
    );
    const loaded = await run(
      process.execPath,
      [
        "--input-type=module",
        "--eval",
        "import {answer} from './public/src/app.js'; console.log(answer)",
      ],
      { cwd: root },
    );
    assert.equal(loaded.stdout.trim(), "current");
    const handlers = {};
    let cached, installed;
    vm.runInNewContext(clean["sw.js"], {
      self: {
        addEventListener: (kind, fn) => {
          handlers[kind] = fn;
        },
        skipWaiting: async () => {},
      },
      caches: {
        open: async () => ({
          addAll: async (assets) => {
            cached = [...assets];
          },
        }),
      },
    });
    handlers.install({
      waitUntil: (promise) => {
        installed = promise;
      },
    });
    await installed;
    assert.deepEqual(
      cached.sort(),
      [
        "./",
        "./style.css",
        "./src/app.js",
        "./vendor/curves/LICENSE",
        "./vendor/curves/package.json",
        "./vendor/curves/secp256k1.js",
        "./vendor/hashes/LICENSE",
        "./vendor/hashes/package.json",
        "./vendor/hashes/sha2.js",
      ].sort(),
    );
    const second = JSON.parse((await build()).stdout);
    assert.equal(second.version, first.version);
    assert.deepEqual(await contents(output), clean);
    await writeFile(
      path.join(root, "src/extra-one.js"),
      "export const unused = 1;",
    );
    const beforeRename = JSON.parse((await build()).stdout);
    await rename(
      path.join(root, "src/extra-one.js"),
      path.join(root, "src/extra-two.js"),
    );
    const afterRename = JSON.parse((await build()).stdout);
    assert.notEqual(
      afterRename.version,
      beforeRename.version,
      "asset names are part of the published cache identity",
    );
    const renamed = await contents(output);
    assert.equal(renamed["src/extra-one.js"], undefined);
    assert.equal(renamed["src/extra-two.js"], "export const unused = 1;");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
