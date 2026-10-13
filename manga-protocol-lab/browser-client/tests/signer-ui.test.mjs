import test from "node:test";
import assert from "node:assert/strict";
import { domHarness, until } from "./fixtures/dom.mjs";
import { createSigner } from "./fixtures/signer.mjs";
test("a rejected reconnect retains the last complete identity and clears publication consent", async () => {
  const h = await domHarness(),
    signer = createSigner();
  try {
    window.nostr = signer;
    await h.start();
    h.$("connect").click();
    await until(() => h.$("identity").textContent.includes(signer.pub));
    const identity = h.$("identity").textContent;
    h.$("consent").checked = true;
    window.nostr = {
      getPublicKey: async () => {
        throw Error("Reconnect rejected");
      },
      signEvent: signer.signEvent,
    };
    h.$("connect").click();
    await until(() => h.$("notice").textContent === "Reconnect rejected");
    assert.equal(h.$("identity").textContent, identity);
    assert.equal(h.$("consent").checked, false);
    assert.equal(h.$("connect").disabled, false);
  } finally {
    h.close();
  }
});
