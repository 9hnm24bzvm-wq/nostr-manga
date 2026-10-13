import test from "node:test";
import assert from "node:assert/strict";
import { IDBObjectStore } from "fake-indexeddb";
import { domHarness, until } from "./fixtures/dom.mjs";
test("a late chapter read cannot append chapter controls into the selected lists view", async (t) => {
  const h = await domHarness();
  let release;
  try {
    await h.store.write("chapters", {
      id: "11".repeat(32),
      completed: false,
      page: 7,
    });
    await h.store.write("lists", { id: "22".repeat(16), name: "Current list" });
    await h.start();
    const getAll = IDBObjectStore.prototype.getAll;
    let first = true;
    t.mock.method(IDBObjectStore.prototype, "getAll", function (...args) {
      const query = getAll.apply(this, args);
      if (first) {
        first = false;
        query.addEventListener("success", (event) => {
          const handler = query.onsuccess;
          event.stopImmediatePropagation();
          release = () => handler.call(query, event);
        });
      }
      return query;
    });
    h.$("family").dispatchEvent(new h.dom.window.Event("change"));
    await until(() => release);
    h.$("family").value = "lists";
    h.$("family").dispatchEvent(new h.dom.window.Event("change"));
    await until(() => h.$("records").textContent.includes("Current list"));
    release();
    await new Promise((resolve) => setTimeout(resolve, 15));
    assert.equal(h.$("records").querySelectorAll("article").length, 1);
    assert.ok(!h.$("records").textContent.includes("page 7"));
  } finally {
    release?.();
    h.close();
  }
});

test("late outbox refresh cannot duplicate snapshots or revive cancelled retry controls", async (t) => {
  const h = await domHarness();
  let release;
  try {
    for (const n of [1, 2])
      await h.store.createJob({
        id: String(n),
        revision: 0,
        event: { pubkey: "11".repeat(32), created_at: n },
        createdAt: n,
        cancelled: false,
        recipients: [
          {
            origin: "https://relay.test",
            state: "pending",
            message: "Not sent",
          },
        ],
        objects: [],
      });
    await h.start();
    const getAll = IDBObjectStore.prototype.getAll;
    let first = true;
    t.mock.method(IDBObjectStore.prototype, "getAll", function (...args) {
      const query = getAll.apply(this, args);
      if (first) {
        first = false;
        query.addEventListener("success", (event) => {
          const handler = query.onsuccess;
          event.stopImmediatePropagation();
          release = () => handler.call(query, event);
        });
      }
      return query;
    });
    const cancels = [...h.$("outbox").querySelectorAll("button")].filter(
      (b) => b.textContent === "Cancel further sending",
    );
    cancels[0].click();
    cancels[1].click();
    await until(
      () =>
        release &&
        [...h.$("outbox").querySelectorAll(".job")].length === 2 &&
        [...h.$("outbox").querySelectorAll(".job")].every((row) =>
          row.textContent.includes("cancelled"),
        ),
    );
    release();
    await new Promise((resolve) => setTimeout(resolve, 15));
    assert.equal(h.$("outbox").querySelectorAll(".job").length, 2);
    assert.equal(
      [...h.$("outbox").querySelectorAll("button")].filter(
        (b) => b.textContent === "Retry selected snapshot",
      ).length,
      0,
    );
  } finally {
    release?.();
    h.close();
  }
});

test("Load more during a pending reset replaces the first page instead of duplicating records", async (t) => {
  const h = await domHarness();
  let release;
  try {
    for (let n = 0; n < 51; n++)
      await h.store.write("chapters", {
        id: n.toString(16).padStart(64, "0"),
        completed: false,
        page: n,
        updated_at: 1,
      });
    await h.start();
    const firstPage = [...h.$("records").querySelectorAll("code")].map(
      (node) => node.textContent,
    );
    assert.equal(firstPage.length, 50);
    const getAll = IDBObjectStore.prototype.getAll;
    let first = true;
    t.mock.method(IDBObjectStore.prototype, "getAll", function (...args) {
      const query = getAll.apply(this, args);
      if (first) {
        first = false;
        query.addEventListener("success", (event) => {
          const handler = query.onsuccess;
          event.stopImmediatePropagation();
          release = () => handler.call(query, event);
        });
      }
      return query;
    });
    h.$("family").dispatchEvent(new h.dom.window.Event("change"));
    await until(() => release);
    h.$("more").click();
    await until(() => !h.$("more").disabled);
    release();
    await new Promise(setImmediate);
    assert.deepEqual(
      [...h.$("records").querySelectorAll("code")].map(
        (node) => node.textContent,
      ),
      firstPage,
    );
  } finally {
    release?.();
    h.close();
  }
});
