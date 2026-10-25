import { hash, b64, unhex, rootDescriptors, readGraph } from "./codec.js";
import {
  now,
  signExact,
  proveService,
  signedRequest,
  publicObject,
  verifyEvent,
  origin,
} from "./nostr.js";
/** @typedef {import("./store.js").LocalLibrary} LocalLibrary */
/** @typedef {import("./types.js").Signer} Signer */
/** @typedef {import("./types.js").Relay} Relay */
/** @typedef {import("./types.js").Job} Job */
/** @typedef {import("./types.js").NewJob} NewJob */
/** @typedef {import("./types.js").PreparedSnapshot} PreparedSnapshot */
/** @typedef {import("./types.js").GraphObject} GraphObject */
/** @typedef {import("./types.js").JsonObject} JsonObject */

/** @param {unknown} value @returns {value is JsonObject} */
function isObject(value) {
  return (
    value !== null && (typeof value === "object" || typeof value === "function")
  );
}
/** @param {unknown} value @returns {value is GraphObject} */
function isGraphObject(value) {
  return (
    isObject(value) &&
    (value.type === "root" || value.type === "chunk") &&
    typeof value.hash === "string" &&
    /^[0-9a-f]{64}$/.test(value.hash) &&
    value.data instanceof Uint8Array &&
    value.data.buffer instanceof ArrayBuffer
  );
}
/** @param {unknown} value @returns {value is PreparedSnapshot} */
function isPreparedSnapshot(value) {
  if (
    !isObject(value) ||
    "error" in value ||
    typeof value.revision !== "number" ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0 ||
    !isObject(value.graph)
  )
    return false;
  const graph = value.graph;
  if (
    !isGraphObject(graph.root) ||
    graph.root.type !== "root" ||
    !Array.isArray(graph.objects) ||
    Object.keys(graph.objects).length !== graph.objects.length
  )
    return false;
  // Iteration visits sparse-array holes, which must never become outbox objects.
  for (const object of graph.objects) if (!isGraphObject(object)) return false;
  return (
    typeof graph.records === "number" &&
    Number.isSafeInteger(graph.records) &&
    graph.records >= 0
  );
}
/** @param {string} database @returns {Promise<PreparedSnapshot>} */
export function graphInWorker(database) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./worker.js", import.meta.url), {
      type: "module",
    });
    const timer = setTimeout(() => {
      worker.terminate();
      reject(Error("Snapshot preparation timed out"));
    }, 120000);
    worker.onmessage = /** @param {MessageEvent<unknown>} message */ ({
      data,
    }) => {
      clearTimeout(timer);
      worker.terminate();
      if (
        isObject(data) &&
        "error" in data &&
        !("revision" in data) &&
        !("graph" in data)
      )
        reject(
          Error(
            typeof data.error === "string" && data.error
              ? data.error
              : "Snapshot preparation failed",
          ),
        );
      else if (isPreparedSnapshot(data)) resolve(data);
      else reject(Error("Invalid snapshot preparation response"));
    };
    worker.onerror = (e) => {
      clearTimeout(timer);
      worker.terminate();
      reject(Error(e.message));
    };
    worker.postMessage({ database });
  });
}
/**
 * @param {LocalLibrary} store
 * @param {Signer} signer
 * @param {string} owner
 * @param {PreparedSnapshot} snapshot
 * @param {Relay[]} recipients
 * @param {boolean} publicConsent
 * @param {number} [reviewedRelayRevision]
 * @returns {Promise<Job>}
 */
export async function queueSnapshot(
  store,
  signer,
  owner,
  snapshot,
  recipients,
  publicConsent,
  reviewedRelayRevision = undefined,
) {
  if (!navigator.locks)
    throw Error("Web Locks required for safe snapshot signing");
  const relayRevision =
    reviewedRelayRevision ?? (await store.getMeta("relayRevision")) ?? 0;
  return navigator.locks.request("manga-new-snapshot", () =>
    queueSnapshotInner(
      store,
      signer,
      owner,
      snapshot,
      recipients,
      publicConsent,
      relayRevision,
    ),
  );
}
/**
 * @param {LocalLibrary} store
 * @param {Signer} signer
 * @param {string} owner
 * @param {PreparedSnapshot} snapshot
 * @param {Relay[]} recipients
 * @param {boolean} publicConsent
 * @param {number} relayRevision
 * @returns {Promise<Job>}
 */
async function queueSnapshotInner(
  store,
  signer,
  owner,
  snapshot,
  recipients,
  publicConsent,
  relayRevision,
) {
  if (publicConsent !== true)
    throw Error("Explicit public publication consent required");
  unhex(owner);
  if (!recipients.length || recipients.length > 8)
    throw Error("Choose between1 and8 relays");
  const jobs = await store.jobs();
  if (jobs.length >= 8)
    throw Error("Outbox limit: remove completed snapshots first");
  const seen = new Set();
  for (const r of recipients) {
    if (origin(r.origin) !== r.origin)
      throw Error("Use canonical relay origins");
    unhex(r.serviceKey);
    if (seen.has(r.origin)) throw Error("Duplicate relay");
    seen.add(r.origin);
  }
  if ((await store.counts()).revision !== snapshot.revision)
    throw Error(
      "Local library changed; preview the new snapshot before publishing",
    );
  let timestamp = now();
  const previous = await store.publicationClock(owner, jobs);
  if (previous !== null) timestamp = Math.max(timestamp, previous + 1);
  if (timestamp > now() + 60)
    throw Error("Publication clock is ahead; wait before signing");
  if (((await store.getMeta("relayRevision")) ?? 0) !== relayRevision)
    throw Error("Instance selection changed while preparing; review again");
  const event = await signExact(signer, owner, {
    kind: 11999,
    created_at: timestamp,
    tags: [],
    content: JSON.stringify({ manifest: snapshot.graph.root.hash, version: 1 }),
  });
  /** @type {NewJob} */
  const job = {
    id: event.id,
    event,
    revision: snapshot.revision,
    relayRevision,
    createdAt: Date.now(),
    cancelled: false,
    objects: snapshot.graph.objects,
    recipients: recipients.map((r) => ({
      ...r,
      cursor: 0,
      state: "pending",
      message: "Not sent",
    })),
  };
  return store.createJob(job);
}
/**
 * @param {LocalLibrary} store
 * @param {string} id
 * @param {Signer} signer
 * @param {string} owner
 * @param {(job: Job) => void} [onUpdate]
 * @param {() => void} [assertCurrent]
 * @param {Pick<LocalLibrary, "job" | "saveJob">} [completionStore]
 * @returns {Promise<Job>}
 */
export async function deliverJob(
  store,
  id,
  signer,
  owner,
  onUpdate = () => {},
  assertCurrent = () => {},
  completionStore = store,
) {
  if (!navigator.locks)
    throw Error("This browser needs Web Locks for safe multi-tab publishing");
  return navigator.locks.request(
    "manga-publish-" + id,
    { ifAvailable: true },
    async (lock) => {
      if (!lock)
        throw Error("This snapshot is already being published in another tab");
      let job = await store.job(id);
      if (!job || job.cancelled) throw Error("Snapshot cancelled or missing");
      if (job.event.pubkey !== owner || !verifyEvent(job.event))
        throw Error("Connect the snapshot signing identity");
      async function active() {
        const current = await store.job(id);
        // This is also signedRequest's final beforeSend boundary. A view may
        // have closed while the cancellation read was awaiting IndexedDB.
        assertCurrent();
        if (!current || current.cancelled)
          throw Error("Cancelled; an already-sent request cannot be recalled");
      }
      for (let index = 0; index < job.recipients.length; index++) {
        const r = job.recipients[index];
        if (["accepted", "superseded"].includes(r.state)) continue;
        try {
          await active();
          await proveService(r.origin, r.serviceKey);
          r.state = "sending";
          r.message = "Verifying and staging";
          await store.saveJob(job);
          onUpdate(job);
          // Restart staging on every manual retry: a persisted cursor can outlive a relay's staging lease.
          r.cursor = 0;
          while (r.cursor < job.objectCount) {
            await active();
            const selected = await store.jobBatch(id, r.cursor);
            const b = {
              body: {
                objects: selected.objects.map((o) => ({
                  type: o.type,
                  data: b64(o.data),
                })),
              },
              hashes: selected.objects.map((o) => o.hash),
              next: selected.next,
            };
            const response = await signedRequest(
              r,
              signer,
              owner,
              "/library/batches",
              "POST",
              b.body,
              65536,
              active,
            );
            if (response.status !== 200) {
              const error = Object.assign(
                Error("Relay refused staging: HTTP" + response.status),
                { status: response.status },
              );
              throw error;
            }
            if (
              !isObject(response.data) ||
              response.data.staged !== true ||
              JSON.stringify(response.data.hashes) !== JSON.stringify(b.hashes)
            )
              throw Error("Invalid staging receipt");
            r.cursor = b.next;
            await store.saveJob(job);
            onUpdate(job);
          }
          await active();
          const response = await signedRequest(
            r,
            signer,
            owner,
            "/libraries",
            "POST",
            { event: job.event },
            65536,
            active,
          );
          if (
            !isObject(response.data) &&
            [200, 202, 409].includes(response.status)
          )
            throw Error("Invalid publication receipt");
          /** @type {JsonObject} */
          const receipt = isObject(response.data) ? response.data : {};
          if (
            receipt.id !== job.event.id &&
            [200, 202, 409].includes(response.status)
          )
            throw Error("Wrong publication receipt");
          if (
            [200, 202].includes(response.status) &&
            typeof receipt.projected === "boolean"
          ) {
            r.state = "accepted";
            r.projected = receipt.projected;
            r.message = receipt.projected
              ? "Accepted and projected"
              : "Accepted; projection pending";
          } else if (response.status === 409 && receipt.superseded === true) {
            r.state = "superseded";
            r.message = "A newer public head already exists";
          } else {
            const error = Object.assign(
              Error("Relay refused publication: HTTP" + response.status),
              { status: response.status },
            );
            throw error;
          }
        } catch (error) {
          const status =
            isObject(error) && typeof error.status === "number"
              ? error.status
              : undefined;
          r.state =
            status && [400, 401, 403, 404, 413].includes(status)
              ? "rejected"
              : "retryable";
          r.message =
            error instanceof Error
              ? error.message
              : "Publication request failed";
        }
        // Preserve an exact response even if the selected key changes after
        // HTTP completed. New requests still go through active() above.
        const current = await completionStore.job(id);
        if (!current) return job;
        if (current.cancelled) {
          job.cancelled = true;
          r.message += " (publishing cancelled)";
        }
        await completionStore.saveJob(job);
        onUpdate(job);
        if (job.cancelled) break;
      }
      return job;
    },
  );
}
/** @param {LocalLibrary} store @param {string} id @returns {Promise<void>} */
export async function cancelJob(store, id) {
  const job = await store.job(id);
  if (job) {
    job.cancelled = true;
    await store.saveJob(job);
  }
}
/**
 * @param {Relay} relay
 * @param {Signer} signer
 * @param {string} owner
 * @returns {Promise<{event: import("./types.js").SignedEvent, snapshot: import("./types.js").LibrarySnapshot}>}
 */
export async function recoverPublic(relay, signer, owner) {
  await proveService(relay.origin, relay.serviceKey);
  const response = await signedRequest(
    relay,
    signer,
    owner,
    "/libraries/" + owner,
    "GET",
  );
  if (response.status !== 200)
    throw Error("No readable library head: HTTP" + response.status);
  if (!isObject(response.data)) throw Error("Invalid public library head");
  const event = response.data.event;
  if (
    !verifyEvent(event) ||
    event.pubkey !== owner ||
    event.kind !== 11999 ||
    event.created_at > now() + 60 ||
    event.tags.some((t) => t[0] === "expiration")
  )
    throw Error("Invalid public library head");
  /** @type {unknown} */
  const pointer = JSON.parse(event.content);
  if (
    !isObject(pointer) ||
    Object.keys(pointer).sort().join("|") !== "manifest|version" ||
    pointer.version !== 1 ||
    typeof pointer.manifest !== "string"
  )
    throw Error("Invalid library pointer");
  unhex(pointer.manifest);
  const root = await publicObject(
    relay,
    signer,
    owner,
    pointer.manifest,
    262144,
  );
  const descriptors = rootDescriptors(root);
  /** @type {Map<string, Uint8Array<ArrayBuffer>>} */
  const objects = new Map();
  for (const d of descriptors) {
    if (!objects.has(d.hash)) {
      const bytes = await publicObject(relay, signer, owner, d.hash, d.size);
      if (bytes.length !== d.size) throw Error("Library object size mismatch");
      objects.set(d.hash, bytes);
    }
    if (objects.get(d.hash)?.length !== d.size)
      throw Error("Library object size mismatch");
  }
  return {
    event,
    snapshot: {
      format: "manga-local-library",
      version: 1,
      records: readGraph(root, objects),
    },
  };
}
