import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import vm from "node:vm";
test("Cache/Fetch adapter model serves canonical offline root and bypasses private or unrelated requests", async () => {
  const source = await fs.readFile(
      new URL("../public/sw.js", import.meta.url),
      "utf8",
    ),
    handlers = {},
    stored = new Map();
  let fetches = 0;
  const base = "https://client.example/";
  const context = {
    URL,
    self: {
      location: { href: base + "sw.js", origin: new URL(base).origin },
      addEventListener: (kind, fn) => {
        handlers[kind] = fn;
      },
      skipWaiting: async () => {},
      clients: {
        claim: async () => {},
      },
    },
    caches: {
      open: async () => ({
        addAll: async (assets) => {
          for (const asset of assets)
            stored.set(new URL(asset, base).href, {
              url: new URL(asset, base).href,
              redirected: asset.endsWith("index.html"),
            });
        },
        match: async (url) => stored.get(String(url)),
      }),
      keys: async () => [],
      delete: async () => true,
    },
    fetch: async () => {
      fetches++;
      throw Error("Offline fixture");
    },
  };
  vm.runInNewContext(source, context);
  let done;
  handlers.install({
    waitUntil: (p) => {
      done = p;
    },
  });
  await done;
  handlers.activate({
    waitUntil: (p) => {
      done = p;
    },
  });
  await done;
  for (const url of [base, base + "index.html"]) {
    let response;
    handlers.fetch({
      request: { url, method: "GET", headers: { has: () => false } },
      respondWith: (p) => {
        response = p;
      },
    });
    const r = await response;
    assert.equal(r.url, base);
    assert.equal(r.redirected, false);
  }
  assert.equal(fetches, 0);
  for (const url of [
    base + "libraries",
    base + "?qa=network",
    "https://relay.example/",
  ]) {
    let intercepted = false;
    handlers.fetch({
      request: { url, method: "GET", headers: { has: () => false } },
      respondWith: () => {
        intercepted = true;
      },
    });
    assert.equal(intercepted, false);
  }
  let intercepted = false;
  handlers.fetch({
    request: { url: base, method: "GET", headers: { has: () => true } },
    respondWith: () => {
      intercepted = true;
    },
  });
  assert.equal(intercepted, false, "authorized data must not use shell cache");
});
