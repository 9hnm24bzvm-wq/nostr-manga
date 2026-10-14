/** @typedef {import("./types.js").Job} Job
 * @typedef {import("./types.js").NewJob} NewJob
 * @typedef {import("./types.js").StoredJob} StoredJob
 * @typedef {import("./types.js").JobObjectRow} JobObjectRow
 * @typedef {import("./types.js").GraphObject} GraphObject
 * @typedef {import("./types.js").JobBatch} JobBatch
 * @typedef {import("./types.js").Relay} Relay
 * @typedef {import("./types.js").Recipient} Recipient
 * @typedef {import("./types.js").PublicationClock} PublicationClock
 */
import { request, transaction, cursor } from "./indexeddb.js";
/** @param {string} id */
const inventoryRange = (id) =>
  IDBKeyRange.bound([id, 0], [id, Number.MAX_SAFE_INTEGER]);
/** @param {NewJob | StoredJob} job
 * @returns {Job}
 */
const metadata = (job) => {
  const { objects, ...state } = job;
  return { ...state, objectCount: objects?.length ?? state.objectCount ?? 0 };
};
/** @param {IDBDatabase} db
 * @param {IDBTransaction} tx
 */
export function upgradeOutbox(db, tx) {
  const objects = db.createObjectStore("jobObjects", {
    keyPath: ["job", "index"],
  });
  const query = tx.objectStore("jobs").openCursor();
  query.onsuccess = () => {
    const row = query.result;
    if (!row) return;
    /** @type {StoredJob} */
    const saved = row.value;
    for (const [index, object] of (saved.objects ?? []).entries())
      objects.add({ job: row.key, index, object });
    row.update(metadata(saved));
    row.continue();
  };
}
// Signed identity and bytes are immutable. This module owns state, deletion,
// and bounded delivery reads so callers never reload a graph to update progress.
export class PublicationOutbox {
  /** @param {IDBDatabase} db */
  constructor(db) {
    this.db = db;
  }
  /** @returns {Promise<Job[]>} */
  list() {
    return transaction(this.db, "jobs", "readonly", (tx) =>
      request(tx.objectStore("jobs").getAll()),
    );
  }
  /** @param {string} id
   * @returns {Promise<Job | undefined>}
   */
  get(id) {
    return transaction(this.db, "jobs", "readonly", (tx) =>
      request(tx.objectStore("jobs").get(id)),
    );
  }
  /** @param {Job} job */
  save(job) {
    return transaction(this.db, "jobs", "readwrite", async (tx) => {
      const jobs = tx.objectStore("jobs");
      /** @type {Job | undefined} */
      const previous = await request(jobs.get(job.id));
      if (!previous) throw Error("Snapshot removed or missing");
      const recipients = previous.recipients.map((recipient, index) => {
        const update = job.recipients[index];
        if (
          !update ||
          update.origin !== recipient.origin ||
          update.serviceKey !== recipient.serviceKey
        )
          throw Error("Snapshot recipients are immutable");
        const next = { ...recipient };
        /** @type {(keyof Pick<Recipient, "cursor" | "state" | "message" | "projected">)[]} */
        const keys = ["cursor", "state", "message", "projected"];
        for (const key of keys)
          if (Object.hasOwn(update, key))
            Object.assign(next, { [key]: update[key] });
        return next;
      });
      jobs.put({
        ...previous,
        recipients,
        cancelled: previous.cancelled || job.cancelled,
      });
    });
  }
  /** @param {NewJob} job
   * @returns {Promise<Job>}
   */
  create(job) {
    return transaction(
      this.db,
      ["jobs", "jobObjects", "meta"],
      "readwrite",
      async (tx) => {
        const jobs = tx.objectStore("jobs"),
          meta = tx.objectStore("meta");
        if ((await request(jobs.count())) >= 8)
          throw Error("Outbox limit reached");
        if (((await request(meta.get("revision"))) ?? 0) !== job.revision)
          throw Error("Local library changed while signing; review again");
        if (
          job.relayRevision !== undefined &&
          job.relayRevision !==
            ((await request(meta.get("relayRevision"))) ?? 0)
        )
          throw Error("Instance selection changed while signing; review again");
        /** @type {PublicationClock | undefined} */
        const legacy = await request(meta.get("lastPublication"));
        if (legacy) {
          const key = "publication:" + legacy.owner;
          /** @type {PublicationClock | undefined} */
          const saved = await request(meta.get(key));
          if (!saved || saved.created_at < legacy.created_at)
            meta.put(legacy, key);
        }
        const state = metadata(job);
        jobs.add(state);
        for (const [index, object] of (job.objects ?? []).entries())
          tx.objectStore("jobObjects").add({ job: job.id, index, object });
        meta.put(
          {
            owner: job.event.pubkey,
            created_at: job.event.created_at,
            id: job.id,
          },
          "lastPublication",
        );
        meta.put(
          {
            owner: job.event.pubkey,
            created_at: job.event.created_at,
            id: job.id,
          },
          "publication:" + job.event.pubkey,
        );
        return state;
      },
    );
  }
  /** @param {string} id
   * @param {number} start
   * @returns {Promise<JobBatch>}
   */
  batch(id, start) {
    if (!Number.isSafeInteger(start) || start < 0)
      throw Error("Invalid outbox cursor");
    return transaction(
      this.db,
      ["jobs", "jobObjects"],
      "readonly",
      async (tx) => {
        /** @type {Job | undefined} */
        const job = await request(tx.objectStore("jobs").get(id));
        if (!job || job.cancelled) throw Error("Snapshot cancelled or missing");
        /** @type {GraphObject[]} */
        const objects = [];
        let bytes = 0;
        await cursor(
          tx
            .objectStore("jobObjects")
            .openCursor(
              IDBKeyRange.bound([id, start], [id, Number.MAX_SAFE_INTEGER]),
            ),
          (row) => {
            if (objects.length === 64) return false;
            /** @type {JobObjectRow} */
            const saved = row.value;
            const object = saved.object;
            if (objects.length && bytes + object.data.length > 4 * 1024 * 1024)
              return false;
            if (
              saved.index !== start + objects.length ||
              object.data.length > 4 * 1024 * 1024
            )
              throw Error("Invalid outbox object inventory");
            objects.push(object);
            bytes += object.data.length;
          },
        );
        if (!objects.length && start < job.objectCount)
          throw Error("Missing outbox objects");
        return { objects, next: start + objects.length };
      },
    );
  }
  /** @returns {Promise<{relays: Relay[], revision: number}>} */
  preferences() {
    return transaction(this.db, "meta", "readonly", async (tx) => {
      const meta = tx.objectStore("meta");
      /** @type {[Relay[] | undefined, number | undefined]} */
      const [relays, revision] = await Promise.all([
        request(meta.get("relays")),
        request(meta.get("relayRevision")),
      ]);
      return { relays: relays ?? [], revision: revision ?? 0 };
    });
  }
  /** @param {Relay[]} relays
   * @param {number} [expectedRevision]
   */
  setRelays(relays, expectedRevision = undefined) {
    if (!Array.isArray(relays) || relays.length > 8)
      throw Error("Invalid instance preferences");
    return transaction(this.db, ["meta", "jobs"], "readwrite", async (tx) => {
      const meta = tx.objectStore("meta"),
        jobs = tx.objectStore("jobs");
      const currentRevision = (await request(meta.get("relayRevision"))) ?? 0;
      if (
        expectedRevision !== undefined &&
        expectedRevision !== currentRevision
      )
        throw Error(
          "Instance selection changed in another tab. Review the current selection again.",
        );
      /** @type {Relay[]} */
      const previous = (await request(meta.get("relays"))) ?? [];
      const removed = previous.filter(
        (old) =>
          !relays.some(
            (next) =>
              next.origin === old.origin && next.serviceKey === old.serviceKey,
          ),
      );
      if (removed.length) {
        /** @type {Job[]} */
        const queued = await request(jobs.getAll());
        for (const job of queued)
          if (
            job.recipients.some((recipient) =>
              removed.some((old) => old.origin === recipient.origin),
            )
          )
            jobs.put({ ...job, cancelled: true });
      }
      const revision = currentRevision + 1;
      meta.put(relays, "relays");
      meta.put(revision, "relayRevision");
      return revision;
    });
  }
  /** @param {string} id */
  remove(id) {
    return transaction(this.db, ["jobs", "jobObjects"], "readwrite", (tx) => {
      tx.objectStore("jobs").delete(id);
      tx.objectStore("jobObjects").delete(inventoryRange(id));
    });
  }
}
