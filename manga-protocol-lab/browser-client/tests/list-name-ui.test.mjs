import test from "node:test";
import assert from "node:assert/strict";
import { domHarness, until } from "./fixtures/dom.mjs";

test("the actual list control admits 128 Unicode scalars and the save operation rejects 129", async () => {
  const h = await domHarness();
  try {
    await h.start();
    const input = h.$("list-name"),
      valid = "🐈".repeat(128);
    // DOM contract check only: native typing enforcement is separate browser QA.
    assert.ok(
      input.maxLength >= valid.length,
      "native UTF-16 limit must not exclude 128 supplementary scalars",
    );
    assert.match(
      input.getAttribute("aria-describedby") ?? "",
      /list-name-help/,
    );
    assert.match(h.$("list-name-help").textContent, /128.*512/);
    input.value = valid;
    h.$("list-form").dispatchEvent(
      new h.dom.window.Event("submit", { bubbles: true, cancelable: true }),
    );
    await until(() => h.$("notice").textContent === "List created locally.");
    assert.deepEqual(
      (await h.store.list("lists")).map((r) => r.name),
      [valid],
    );
    input.value = "a".repeat(129);
    h.$("list-form").dispatchEvent(
      new h.dom.window.Event("submit", { bubbles: true, cancelable: true }),
    );
    await until(() => h.$("notice").textContent.includes("Invalid list name"));
    assert.deepEqual(
      (await h.store.list("lists")).map((r) => r.name),
      [valid],
    );
  } finally {
    h.close();
  }
});
