import test from "node:test";
import assert from "node:assert/strict";
import { domHarness, until } from "./fixtures/dom.mjs";
import { createSigner } from "./fixtures/signer.mjs";
import { buildGraph } from "../src/codec.js";

test("forgetting an instance during snapshot preparation prevents later signing from publishing to it", async () => {
  const h = await domHarness(),
    signer = createSigner();
  let release,
    transmissions = 0;
  try {
    await h.store.setRelays([
      { origin: "https://relay.test", serviceKey: signer.pub, selected: true },
    ]);
    const snapshot = await h.store.snapshot();
    // Worker is an external browser transport. Hold its valid response to expose
    // the user's intervening action; no application operation is replaced.
    globalThis.Worker = class {
      postMessage() {
        release = () =>
          this.onmessage({
            data: {
              revision: snapshot.revision,
              graph: buildGraph(snapshot.records),
            },
          });
      }
      terminate() {}
    };
    globalThis.fetch = async () => {
      transmissions++;
      throw Error("Unexpected transmission");
    };
    window.nostr = signer;
    await h.start();
    h.$("connect").click();
    await until(() => h.$("identity").textContent.includes(signer.pub));
    h.$("consent").checked = true;
    h.$("publish").click();
    await until(() => release);
    [...h.$("relays").querySelectorAll("button")]
      .find((button) => button.textContent === "Forget instance")
      .click();
    await until(() => !h.$("relays").querySelector("button"));
    release();
    await until(() => !h.$("publish").disabled);
    assert.deepEqual(await h.store.jobs(), []);
    assert.equal(transmissions, 0);
    assert.match(h.$("notice").textContent, /selection changed/i);
  } finally {
    h.close();
  }
});

test("a tab with an older instance selection cannot publish after another tab forgets it", async () => {
  const h = await domHarness(),
    signer = createSigner();
  let transmissions = 0;
  try {
    await h.store.setRelays([
      { origin: "https://relay.test", serviceKey: signer.pub, selected: true },
    ]);
    const snapshot = await h.store.snapshot();
    globalThis.Worker = class {
      postMessage() {
        queueMicrotask(() =>
          this.onmessage({
            data: {
              revision: snapshot.revision,
              graph: buildGraph(snapshot.records),
            },
          }),
        );
      }
      terminate() {}
    };
    globalThis.fetch = async () => {
      transmissions++;
      throw Error("Unexpected transmission");
    };
    window.nostr = signer;
    await h.start();
    h.$("connect").click();
    await until(() => h.$("identity").textContent.includes(signer.pub));
    await h.store.setRelays([]);
    h.$("consent").checked = true;
    h.$("publish").click();
    await until(() => !h.$("publish").disabled);
    assert.deepEqual(await h.store.jobs(), []);
    assert.equal(transmissions, 0);
    assert.match(h.$("notice").textContent, /selection changed/i);
  } finally {
    h.close();
  }
});

test("changing a stale instance checkbox cannot recreate an instance forgotten by another tab", async () => {
  const h = await domHarness(),
    signer = createSigner();
  try {
    await h.store.setRelays([
      { origin: "https://relay.test", serviceKey: signer.pub, selected: true },
    ]);
    await h.start();
    await h.store.setRelays([]);
    const checkbox = h.$("relays").querySelector("input");
    checkbox.click();
    await until(
      () =>
        h.$("notice").textContent.includes("changed") ||
        h.$("relays").querySelector("input")?.checked === false,
    );
    assert.deepEqual((await h.store.relayPreferences()).relays, []);
    await until(() => h.$("notice").textContent.includes("changed"));
    assert.equal(h.$("relays").querySelector("input"), null);
  } finally {
    h.close();
  }
});
