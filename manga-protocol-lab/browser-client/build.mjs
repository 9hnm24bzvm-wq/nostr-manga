import {
  mkdir,
  readFile,
  writeFile,
  readdir,
  copyFile,
  mkdtemp,
  rename,
  rm,
  lstat,
} from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
const root = path.dirname(fileURLToPath(import.meta.url));
const published = path.join(root, "public");
// Recover the previous complete output after an interrupted promotion. Refuse
// ambiguous retained outputs rather than guessing which one belongs in public.
try {
  await lstat(published);
} catch (error) {
  if (error.code !== "ENOENT") throw error;
  const previous = (await readdir(root, { withFileTypes: true })).filter(
    (entry) =>
      entry.isDirectory() &&
      /^\.public-previous-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
        entry.name,
      ),
  );
  if (previous.length > 1)
    throw new Error(
      "Multiple previous builds retained; restore the intended directory to public before building",
    );
  if (previous.length === 1)
    await rename(path.join(root, previous[0].name), published);
}
const out = await mkdtemp(path.join(root, ".public-build-"));
try {
  async function copyJS(source, dest) {
    await mkdir(dest, { recursive: true });
    for (const entry of await readdir(source, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (entry.name === "abstract")
          await copyJS(
            path.join(source, entry.name),
            path.join(dest, entry.name),
          );
        continue;
      }
      if (
        !entry.name.endsWith(".js") &&
        !["LICENSE", "package.json"].includes(entry.name)
      )
        continue;
      let content = await readFile(path.join(source, entry.name), "utf8");
      if (entry.name.endsWith(".js"))
        content = content.replace(
          /(['"])@noble\/(curves|hashes)\/([^'"]+)\1/g,
          (_, q, p, f) => {
            let rel = path
              .relative(dest, path.join(out, "vendor", p, f))
              .split(path.sep)
              .join("/");
            if (!rel.startsWith(".")) rel = "./" + rel;
            return q + rel + q;
          },
        );
      await writeFile(path.join(dest, entry.name), content);
    }
  }
  await copyJS(
    path.join(root, "node_modules/@noble/curves"),
    path.join(out, "vendor/curves"),
  );
  await copyJS(
    path.join(root, "node_modules/@noble/hashes"),
    path.join(out, "vendor/hashes"),
  );
  await copyJS(path.join(root, "src"), path.join(out, "src"));
  for (const f of ["index.html", "style.css"])
    await copyFile(path.join(root, f), path.join(out, f));
  async function files(dir) {
    const result = [];
    for (const e of await readdir(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) result.push(...(await files(p)));
      else if (e.name !== "sw.js") result.push(p);
    }
    return result;
  }
  const all = await files(out);
  const checksum = crypto.createHash("sha256");
  for (const f of all.sort()) {
    const content = await readFile(f);
    const name = path.relative(out, f).split(path.sep).join("/");
    checksum.update(JSON.stringify([name, content.length]));
    checksum.update(content);
  }
  checksum.update(await readFile(new URL(import.meta.url)));
  const version = checksum.digest("hex").slice(0, 16),
    assets = all.map((f) => {
      const name = path.relative(out, f).split(path.sep).join("/");
      return name === "index.html" ? "./" : "./" + name;
    });
  await writeFile(
    path.join(out, "sw.js"),
    `const CACHE='manga-local-${version}',ASSETS=${JSON.stringify(assets)};self.addEventListener('install',e=>e.waitUntil(caches.open(CACHE).then(c=>c.addAll(ASSETS)).then(()=>self.skipWaiting())));self.addEventListener('activate',e=>e.waitUntil(Promise.all([caches.keys().then(names=>Promise.all(names.filter(n=>n.startsWith('manga-local-')&&n!==CACHE).map(n=>caches.delete(n)))),self.clients.claim()])));self.addEventListener('fetch',e=>{const u=new URL(e.request.url);if(e.request.method!=='GET'||u.origin!==self.location.origin||e.request.headers.has('authorization'))return;const base=new URL('./',self.location.href);if(!u.pathname.startsWith(base.pathname))return;const name=(u.pathname===base.pathname||u.pathname===base.pathname+'index.html')?'./':'./'+u.pathname.slice(base.pathname.length);if(u.search||!ASSETS.includes(name))return;e.respondWith(caches.open(CACHE).then(c=>c.match(new URL(name,base))).then(r=>r||fetch(e.request)));});`,
  );
  // Each promotion uses a same-filesystem rename. Preserve a rollback copy until
  // promotion succeeds. This portable two-rename protocol is not power-loss atomic.
  const previous = path.join(root, ".public-previous-" + crypto.randomUUID());
  let savedPrevious = false;
  try {
    await rename(published, previous);
    savedPrevious = true;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  try {
    await rename(out, published);
  } catch (error) {
    if (savedPrevious) {
      try {
        await rename(previous, published);
      } catch (restoreError) {
        throw new AggregateError(
          [error, restoreError],
          "Build promotion failed; previous output retained at " + previous,
        );
      }
    }
    throw error;
  }
  if (savedPrevious) await rm(previous, { recursive: true, force: true });
  console.log(
    JSON.stringify({ version, assets: assets.length, output: published }),
  );
} finally {
  await rm(out, { recursive: true, force: true });
}
