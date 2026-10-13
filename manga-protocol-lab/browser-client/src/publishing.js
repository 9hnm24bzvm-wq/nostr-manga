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
export function graphInWorker(database) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./worker.js", import.meta.url), {
      type: "module",
    });
    const timer = setTimeout(() => {
      worker.terminate();
      reject(Error("Snapshot preparation timed out"));
    }, 120000);
    worker.onmessage = ({ data }) => {
      clearTimeout(timer);
      worker.terminate();
      if (data.error) reject(Error(data.error));
      else resolve(data);
    };
    worker.onerror = (e) => {
      clearTimeout(timer);
      worker.terminate();
      reject(Error(e.message));
    };
    worker.postMessage({ database });
  });
}
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
export async function deliverJob(
  store,
  id,
  signer,
  owner,
  onUpdate = () => {},
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
              const error = Error(
                "Relay refused staging: HTTP" + response.status,
              );
              error.status = response.status;
              throw error;
            }
            if (
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
            response.data.id !== job.event.id &&
            [200, 202, 409].includes(response.status)
          )
            throw Error("Wrong publication receipt");
          if (
            [200, 202].includes(response.status) &&
            typeof response.data.projected === "boolean"
          ) {
            r.state = "accepted";
            r.projected = response.data.projected;
            r.message = response.data.projected
              ? "Accepted and projected"
              : "Accepted; projection pending";
          } else if (
            response.status === 409 &&
            response.data.superseded === true
          ) {
            r.state = "superseded";
            r.message = "A newer public head already exists";
          } else {
            const error = Error(
              "Relay refused publication: HTTP" + response.status,
            );
            error.status = response.status;
            throw error;
          }
        } catch (error) {
          r.state =
            error.status && [400, 401, 403, 404, 413].includes(error.status)
              ? "rejected"
              : "retryable";
          r.message = error.message;
        }
        const current = await store.job(id);
        if (!current) return job;
        if (current.cancelled) {
          job.cancelled = true;
          r.message += " (publishing cancelled)";
        }
        await store.saveJob(job);
        onUpdate(job);
        if (job.cancelled) break;
      }
      return job;
    },
  );
}
export async function cancelJob(store, id) {
  const job = await store.job(id);
  if (job) {
    job.cancelled = true;
    await store.saveJob(job);
  }
}
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
  const event = response.data.event;
  if (
    !verifyEvent(event) ||
    event.pubkey !== owner ||
    event.kind !== 11999 ||
    event.created_at > now() + 60 ||
    event.tags.some((t) => t[0] === "expiration")
  )
    throw Error("Invalid public library head");
  const pointer = JSON.parse(event.content);
  if (
    Object.keys(pointer).sort().join("|") !== "manifest|version" ||
    pointer.version !== 1
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
  const descriptors = rootDescriptors(root),
    objects = new Map();
  for (const d of descriptors) {
    if (!objects.has(d.hash)) {
      const bytes = await publicObject(relay, signer, owner, d.hash, d.size);
      if (bytes.length !== d.size) throw Error("Library object size mismatch");
      objects.set(d.hash, bytes);
    }
    if (objects.get(d.hash).length !== d.size)
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
