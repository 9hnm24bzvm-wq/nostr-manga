import { backupBlob, importBackup } from "./backup.js";
import { LocalLibrary } from "./store.js";
import { recordKey, hex } from "./codec.js";
import { proveService, signedRequest, signerRequest } from "./nostr.js";
import {
  graphInWorker,
  queueSnapshot,
  deliverJob,
  cancelJob,
  recoverPublic,
} from "./publishing.js";
const $ = (id) => document.getElementById(id);
let store,
  signer = null,
  owner = null,
  signerGeneration = 0,
  recoveryApplying = false,
  relays = [],
  relayRevision = 0,
  cursor = null;
function notify(message, error = false) {
  $("notice").textContent = message;
  $("notice").classList.toggle("error", error);
}
async function action(fn, button) {
  if (button) button.disabled = true;
  try {
    await fn();
  } catch (e) {
    notify(e.message, true);
  } finally {
    if (button) button.disabled = false;
  }
}
function button(label, fn) {
  const b = document.createElement("button");
  b.textContent = label;
  b.className = "secondary";
  b.addEventListener("click", () => action(fn, b));
  return b;
}
function p(parent, text, tag = "p") {
  const node = document.createElement(tag);
  node.textContent = text;
  parent.append(node);
  return node;
}
let renderGeneration = 0,
  renderedFamily = null,
  resetPending = false;
async function render(reset = true) {
  const generation = ++renderGeneration,
    family = $("family").value;
  const replace = reset || resetPending || renderedFamily !== family;
  if (replace) resetPending = true;
  const after = replace ? null : cursor;
  let counts, rows;
  try {
    [counts, rows] = await Promise.all([
      store.counts(),
      store.list(family, after, 50),
    ]);
  } catch (error) {
    if (generation === renderGeneration) throw error;
    return;
  }
  if (generation !== renderGeneration || family !== $("family").value) return;
  const fragment = document.createDocumentFragment();
  for (const record of rows) {
    const element = document.createElement("article");
    element.className = "record";
    p(element, recordKey(family, record), "code");
    if (family === "chapters")
      p(
        element,
        `${record.completed ? "Completed" : "Reading"} · page ${record.page ?? "not set"}`,
      );
    if (family === "series") p(element, record.status.replaceAll("_", " "));
    if (family === "lists") p(element, record.name);
    if (family === "members") p(element, `Position ${record.position}`);
    element.append(
      button("Remove locally", async () => {
        if (
          !confirm(
            "Remove this local record? This does not delete anything already published.",
          )
        )
          return;
        await store.remove(family, recordKey(family, record));
        await render();
        notify("Removed locally. No relay contacted.");
      }),
    );
    fragment.append(element);
  }
  if (!rows.length && replace) p(fragment, "No records yet.");
  $("counts").textContent =
    `${counts.chapters} chapters · ${counts.series} series · ${counts.lists} lists`;
  if (replace) $("records").replaceChildren(fragment);
  else $("records").append(fragment);
  cursor = rows.length ? recordKey(family, rows.at(-1)) : after;
  renderedFamily = family;
  resetPending = false;
  $("more").hidden = rows.length < 50;
}
function needSigner() {
  if (!signer || !owner)
    throw Error(
      "Connect a compatible NIP07 browser signer first. Local use needs none.",
    );
}
async function saveRelays(next) {
  try {
    relayRevision = await store.setRelays(next, relayRevision);
    relays = next;
  } catch (error) {
    const current = await store.relayPreferences();
    relays = current.relays;
    relayRevision = current.revision;
    await renderRelays();
    throw error;
  }
}
async function renderRelays() {
  const root = $("relays");
  root.replaceChildren();
  for (const r of relays) {
    const el = document.createElement("div");
    el.className = "relay";
    const label = document.createElement("label");
    label.className = "check";
    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = r.selected === true;
    check.onchange = () =>
      action(async () => {
        await saveRelays(
          relays.map((item) =>
            item.origin === r.origin
              ? { ...item, selected: check.checked }
              : item,
          ),
        );
      });
    label.append(check, document.createTextNode(r.origin));
    el.append(label);
    p(el, r.serviceKey, "code");
    el.append(
      button("Request account admission", async () => {
        needSigner();
        if (
          !confirm(
            `Ask ${r.origin} to admit public key ${owner}? This does not publish your library.`,
          )
        )
          return;
        await proveService(r.origin, r.serviceKey);
        const result = await signedRequest(r, signer, owner, "/enrollment");
        if (![200, 201].includes(result.status))
          throw Error(
            "Instance does not allow self-registration: HTTP" + result.status,
          );
        notify("Account admitted. Library remains local until you publish.");
      }),
      button("Recover public snapshot", async () => {
        needSigner();
        if (recoveryApplying)
          throw Error("Recovery is being applied; try again shortly");
        const reviewedSigner = signer,
          reviewedOwner = owner,
          reviewedGeneration = signerGeneration,
          reviewedRelayRevision = relayRevision;
        const checkIdentity = () => {
          if (
            signer !== reviewedSigner ||
            owner !== reviewedOwner ||
            signerGeneration !== reviewedGeneration
          )
            throw Error("Connected identity changed; review recovery again");
        };
        const checkReview = async () => {
          const preferences = await store.relayPreferences();
          checkIdentity();
          if (preferences.revision !== reviewedRelayRevision)
            throw Error("Instance selection changed; review recovery again");
        };
        if (
          !confirm(
            `Fetch the public library for connected key ${reviewedOwner} from ${r.origin}? No local records will change until you confirm the preview.`,
          )
        )
          return;
        await checkReview();
        const revision = (await store.counts()).revision;
        notify("Verifying the public snapshot…");
        const result = await recoverPublic(r, reviewedSigner, reviewedOwner);
        await checkReview();
        const n = Object.values(result.snapshot.records).reduce(
          (a, b) => a + b.length,
          0,
        );
        if (
          !confirm(
            `Verified ${n} records.\nOwner: ${result.event.pubkey}\nInstance: ${r.origin}\nService key: ${r.serviceKey}\nEvent: ${result.event.id}\nSigned at: ${result.event.created_at}\nReplace this device’s local library? Export a backup first if you want to retain its current contents.`,
          )
        )
          return;
        checkIdentity();
        if (recoveryApplying)
          throw Error("Recovery is being applied; try again shortly");
        recoveryApplying = true;
        try {
          await store.replace(
            result.snapshot,
            revision,
            {
              owner: result.event.pubkey,
              created_at: result.event.created_at,
              id: result.event.id,
            },
            reviewedRelayRevision,
          );
        } finally {
          recoveryApplying = false;
        }
        await render();
        notify(
          "Public snapshot restored locally. Automatic publishing remains off.",
        );
      }),
      button("Forget instance", async () => {
        if (
          !confirm(
            "Forget this instance and cancel unfinished snapshots that include it? Published copies cannot be recalled.",
          )
        )
          return;
        await saveRelays(relays.filter((x) => x.origin !== r.origin));
        await renderRelays();
        await renderJobs();
      }),
    );
    root.append(el);
  }
}
let outboxGeneration = 0;
async function renderJobs() {
  const generation = ++outboxGeneration;
  let jobs;
  try {
    jobs = await store.jobs();
  } catch (error) {
    if (generation === outboxGeneration) throw error;
    return;
  }
  if (generation !== outboxGeneration) return;
  const root = document.createDocumentFragment();
  for (const j of jobs.sort((a, b) => b.createdAt - a.createdAt)) {
    const el = document.createElement("div");
    el.className = "job";
    p(
      el,
      `Snapshot of local revision ${j.revision}${j.cancelled ? " · cancelled" : ""}`,
    );
    p(el, j.id, "code");
    for (const r of j.recipients)
      p(el, `${r.origin}: ${r.state} · ${r.message}`);
    if (
      !j.cancelled &&
      j.recipients.some((r) => !["accepted", "superseded"].includes(r.state))
    )
      el.append(
        button("Retry selected snapshot", async () => {
          needSigner();
          if (
            !confirm(
              "Retry this previously approved PUBLIC snapshot to its original recipients?",
            )
          )
            return;
          await deliverJob(store, j.id, signer, owner, () =>
            action(() => renderJobs()),
          );
          await renderJobs();
          notify(
            "Retry finished; review each recipient above. Local changes were not sent.",
          );
        }),
        button("Cancel further sending", async () => {
          await cancelJob(store, j.id);
          await renderJobs();
          notify(
            "Cancelled further requests. Already-sent requests cannot be recalled.",
          );
        }),
      );
    if (
      j.cancelled ||
      j.recipients.every((r) => ["accepted", "superseded"].includes(r.state))
    )
      el.append(
        button("Remove local outbox record", async () => {
          if (
            !confirm(
              "Remove this saved retry record? This does not delete published data.",
            )
          )
            return;
          await store.deleteJob(j.id);
          await renderJobs();
        }),
      );
    root.append(el);
  }
  $("outbox").replaceChildren(root);
}
function attachForm(id, fn) {
  $(id).addEventListener("submit", (e) => {
    e.preventDefault();
    action(fn, e.submitter);
  });
}
async function boot() {
  store = await LocalLibrary.open();
  const preferences = await store.relayPreferences();
  relays = preferences.relays;
  relayRevision = preferences.revision;
  await render();
  await renderRelays();
  await renderJobs();
  notify("Ready. Saves stay on this device.");
  if (store.channel) store.channel.onmessage = () => action(() => render());
  attachForm("chapter-form", async () => {
    await store.write("chapters", {
      id: $("chapter-id").value.trim(),
      page: Number($("chapter-page").value),
      completed: $("chapter-completed").checked,
    });
    await render();
    notify("Progress saved locally. No relay contacted.");
  });
  attachForm("series-form", async () => {
    await store.write("series", {
      id: $("series-id").value.trim(),
      status: $("series-status").value,
    });
    await render();
    notify("Series saved locally.");
  });
  attachForm("list-form", async () => {
    const id = hex(crypto.getRandomValues(new Uint8Array(16)));
    await store.write("lists", { id, name: $("list-name").value });
    $("member-list").value = id;
    $("family").value = "lists";
    await render();
    notify("List created locally.");
  });
  attachForm("member-form", async () => {
    await store.write("members", {
      list: $("member-list").value.trim(),
      series: $("member-series").value.trim(),
      position: Number($("member-position").value),
    });
    await render();
    notify("List membership saved locally.");
  });
  $("family").onchange = () => action(() => render());
  $("more").onclick = (e) => action(() => render(false), e.currentTarget);
  $("export").onclick = (e) =>
    action(async () => {
      const url = URL.createObjectURL(await backupBlob(store));
      const a = document.createElement("a");
      a.href = url;
      a.download =
        "manga-local-library-" +
        new Date().toISOString().slice(0, 10) +
        ".jsonl";
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 10000);
      notify("Local backup prepared. Keep the file private.");
    }, e.currentTarget);
  $("import").onchange = () =>
    action(async () => {
      const file = $("import").files[0];
      $("import").value = "";
      if (!file) return;
      const revision = (await store.counts()).revision;
      if (
        !confirm(
          "Replace this device’s local library with this backup? Export the current library first if you want to retain it. Nothing will be published.",
        )
      )
        return;
      await importBackup(store, file, revision);
      await render();
      notify("Backup imported locally.");
    });
  $("persistent").onclick = (e) =>
    action(async () => {
      if (!navigator.storage?.persist)
        throw Error(
          "Persistent-storage requests are unavailable. Keep exported backups.",
        );
      const yes = await navigator.storage.persist();
      $("storage-status").textContent = yes
        ? "Persistent storage granted. Keep backups anyway."
        : "Persistence not granted. Storage can be evicted; keep exported backups.";
    }, e.currentTarget);
  $("connect").onclick = (e) =>
    action(async () => {
      if (recoveryApplying)
        throw Error("Recovery is being applied; reconnect afterwards");
      if (!window.nostr?.getPublicKey || !window.nostr?.signEvent)
        throw Error(
          "No NIP07 signer found. Install/use a compatible browser signer to publish; local saving still works.",
        );
      const candidate = window.nostr;
      signerGeneration++;
      $("consent").checked = false;
      const selected = await signerRequest(() => candidate.getPublicKey());
      if (recoveryApplying)
        throw Error("Recovery is being applied; reconnect afterwards");
      if (!/^[0-9a-f]{64}$/.test(selected))
        throw Error("Invalid signer public key");
      signer = candidate;
      owner = selected;
      $("identity").textContent = "Connected public key: " + owner;
      notify("Signer connected. Local saving still does not publish.");
    }, e.currentTarget);
  attachForm("relay-form", async () => {
    const input = $("relay-origin").value.trim(),
      old = relays.find((r) => r.origin === new URL(input).origin);
    if (relays.length >= 8 && !old) throw Error("Up to8 saved instances");
    const expected =
      old?.serviceKey || $("relay-key").value.trim() || undefined;
    const verified = await proveService(input, expected);
    if (
      !old &&
      !confirm(
        `Service key verified for ${verified.origin}:\n${verified.serviceKey}\nRemember this identity? This proves possession, not reputation.`,
      )
    )
      return;
    await saveRelays([
      ...relays.filter((r) => r.origin !== verified.origin),
      { ...verified, selected: true },
    ]);
    await renderRelays();
    notify("Instance remembered. No library data sent.");
  });
  $("publish").onclick = (e) =>
    action(async () => {
      needSigner();
      if (!$("consent").checked)
        throw Error("Confirm that publication is public first");
      if (!navigator.locks)
        throw Error("Web Locks required for multi-tab publishing");
      const recipients = relays.filter((r) => r.selected);
      if (!recipients.length) throw Error("Choose a verified instance");
      const reviewedRelayRevision = relayRevision;
      notify("Preparing a local snapshot…");
      const snapshot = await graphInWorker(store.db.name);
      if (
        !confirm(
          `Publish ${snapshot.graph.records} records from local revision ${snapshot.revision} under key ${owner} to ${recipients.map((r) => r.origin).join(", ")}? Reading history, progress and list names become public. Later local saves remain local.`,
        )
      )
        return;
      const job = await queueSnapshot(
        store,
        signer,
        owner,
        snapshot,
        recipients,
        true,
        reviewedRelayRevision,
      );
      $("consent").checked = false;
      await renderJobs();
      notify("Publishing explicitly approved snapshot…");
      await deliverJob(store, job.id, signer, owner, () =>
        action(() => renderJobs()),
      );
      await renderJobs();
      notify(
        "Publication attempt finished. Review per-instance outcomes; local data is safe.",
      );
    }, e.currentTarget);
  if ("serviceWorker" in navigator)
    navigator.serviceWorker.register("./sw.js").catch(() => {
      $("storage-status").textContent =
        "Offline app caching unavailable. Local data still uses IndexedDB.";
    });
}
boot().catch((e) =>
  notify(
    "Cannot open local storage: " +
      e.message +
      ". This client will not fall back to relay storage.",
    true,
  ),
);
