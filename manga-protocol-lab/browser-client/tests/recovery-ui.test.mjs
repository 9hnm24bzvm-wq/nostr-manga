import test from "node:test";
import assert from "node:assert/strict";
import { domHarness, until } from "./fixtures/dom.mjs";
import { createSigner } from "./fixtures/signer.mjs";
import { recoveryPeer } from "./fixtures/recovery-peer.mjs";
const current = {
  id: "99".repeat(32),
  completed: true,
  page: 9,
  updated_at: 1,
};
function button(h, name) {
  return [...h.$("relays").querySelectorAll("button")].find(
    (b) => b.textContent === name,
  );
}
async function startRecovery(h, peer, confirmations) {
  await h.store.write("chapters", current);
  await h.store.setRelays([peer.relay]);
  window.nostr = peer.user;
  globalThis.fetch = peer.fetch;
  globalThis.confirm = (message) => {
    confirmations.push(message);
    return true;
  };
  await h.start();
  h.$("connect").click();
  await until(() => h.$("identity").textContent.includes(peer.user.pub));
  const recover = button(h, "Recover public snapshot");
  recover.click();
  await peer.waiting;
  return recover;
}

for (const change of [
  "reconnect",
  "forget",
  "deselect",
  "another tab changes preferences",
])
  test(`a deferred recovery cannot preview or replace after ${change}`, async () => {
    const h = await domHarness(),
      peer = await recoveryPeer(),
      confirmations = [];
    let recover;
    try {
      recover = await startRecovery(h, peer, confirmations);
      if (change === "reconnect") {
        const other = createSigner(3);
        window.nostr = other;
        h.$("connect").click();
        await until(() => h.$("identity").textContent.includes(other.pub));
      } else if (change === "forget") {
        button(h, "Forget instance").click();
        await until(() => !h.$("relays").querySelector("button"));
      } else if (change === "deselect") {
        h.$("relays").querySelector("input").click();
        await until(
          async () =>
            (await h.store.relayPreferences()).relays[0].selected === false,
        );
      } else {
        await h.store.setRelays([]);
      }
      peer.release();
      await until(() => !recover.disabled);
      assert.deepEqual(await h.store.list("chapters"), [current]);
      assert.equal(
        confirmations.filter((message) => message.startsWith("Verified "))
          .length,
        0,
      );
      assert.match(h.$("notice").textContent, /identity|selection.*changed/i);
    } finally {
      peer.release();
      if (recover) await until(() => !recover.disabled);
      h.close();
    }
  });

for (const approve of [false, true])
  test(`recovery displays its exact signed source and ${approve ? "applies only after approval" : "decline preserves local data"}`, async () => {
    const h = await domHarness(),
      peer = await recoveryPeer(),
      confirmations = [];
    let recover;
    try {
      recover = await startRecovery(h, peer, confirmations);
      globalThis.confirm = (message) => {
        confirmations.push(message);
        return approve;
      };
      peer.release();
      await until(() => !recover.disabled);
      const preview = confirmations.find((message) =>
        message.startsWith("Verified "),
      );
      assert.ok(preview.includes(peer.user.pub));
      assert.ok(preview.includes(peer.relay.origin));
      assert.ok(preview.includes(peer.relay.serviceKey));
      assert.ok(preview.includes(peer.event.id));
      assert.deepEqual(
        await h.store.list("chapters"),
        approve ? peer.records.chapters : [current],
      );
      assert.deepEqual(await h.store.jobs(), []);
      h.$("connect").click();
      await until(() => !h.$("connect").disabled);
      assert.match(h.$("notice").textContent, /Signer connected/);
    } finally {
      peer.release();
      if (recover) await until(() => !recover.disabled);
      h.close();
    }
  });

test("a preference change racing the confirmed preview is fenced by replacement", async () => {
  const h = await domHarness(),
    peer = await recoveryPeer(),
    confirmations = [];
  let recover, changed;
  try {
    recover = await startRecovery(h, peer, confirmations);
    globalThis.confirm = (message) => {
      confirmations.push(message);
      changed = h.store.setRelays([]);
      return true;
    };
    peer.release();
    await until(() => !recover.disabled);
    await changed;
    assert.deepEqual(await h.store.list("chapters"), [current]);
    assert.match(h.$("notice").textContent, /selection.*changed/i);
    h.$("connect").click();
    await until(() => !h.$("connect").disabled);
    assert.match(h.$("notice").textContent, /Signer connected/);
  } finally {
    peer.release();
    if (recover) await until(() => !recover.disabled);
    h.close();
  }
});

test("a reconnect started by the preview invalidates recovery even before the signer answers", async () => {
  const h = await domHarness(),
    peer = await recoveryPeer(),
    confirmations = [];
  const other = createSigner(3);
  let recover, answer;
  try {
    recover = await startRecovery(h, peer, confirmations);
    globalThis.confirm = (message) => {
      confirmations.push(message);
      window.nostr = {
        signEvent: other.signEvent,
        getPublicKey: () =>
          new Promise((resolve) => {
            answer = resolve;
          }),
      };
      h.$("connect").click();
      return true;
    };
    peer.release();
    await until(() => !recover.disabled);
    assert.deepEqual(await h.store.list("chapters"), [current]);
    assert.match(h.$("notice").textContent, /identity.*changed/i);
    answer(other.pub);
    await until(() => !h.$("connect").disabled);
    assert.ok(h.$("identity").textContent.includes(other.pub));
  } finally {
    answer?.(other.pub);
    peer.release();
    if (recover) await until(() => !recover.disabled);
    h.close();
  }
});

test("editing local data during fetch retains the existing transaction revision fence", async () => {
  const h = await domHarness(),
    peer = await recoveryPeer(),
    confirmations = [];
  let recover;
  try {
    recover = await startRecovery(h, peer, confirmations);
    await h.store.write("chapters", { ...current, page: 19 });
    peer.release();
    await until(() => !recover.disabled);
    assert.deepEqual(await h.store.list("chapters"), [
      { ...current, page: 19 },
    ]);
    assert.match(h.$("notice").textContent, /Local library changed/i);
    h.$("connect").click();
    await until(() => !h.$("connect").disabled);
    assert.match(h.$("notice").textContent, /Signer connected/);
  } finally {
    peer.release();
    if (recover) await until(() => !recover.disabled);
    h.close();
  }
});

test("an aborted recovery commit preserves local data and releases controls for a successful retry", async (t) => {
  const { IDBObjectStore } = await import("fake-indexeddb");
  const h = await domHarness(),
    peer = await recoveryPeer(),
    confirmations = [];
  let recover, fault;
  try {
    recover = await startRecovery(h, peer, confirmations);
    const put = IDBObjectStore.prototype.put;
    let interrupted = false;
    fault = t.mock.method(IDBObjectStore.prototype, "put", function (...args) {
      const query = put.apply(this, args);
      if (!interrupted) {
        interrupted = true;
        query.addEventListener("success", () => this.transaction.abort());
      }
      return query;
    });
    peer.release();
    await until(() => !recover.disabled);
    fault.mock.restore();
    assert.deepEqual(await h.store.list("chapters"), [current]);
    assert.match(h.$("notice").textContent, /abort/i);
    h.$("connect").click();
    await until(() => !h.$("connect").disabled);
    assert.match(h.$("notice").textContent, /Signer connected/);
    recover.click();
    await until(() => !recover.disabled);
    assert.deepEqual(await h.store.list("chapters"), peer.records.chapters);
    assert.match(h.$("notice").textContent, /restored locally/);
  } finally {
    fault?.mock.restore();
    peer.release();
    if (recover) await until(() => !recover.disabled);
    h.close();
  }
});
