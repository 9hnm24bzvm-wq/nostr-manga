import { request, transaction, cursor } from "./indexeddb.js";
const inventoryRange = (id) =>
  IDBKeyRange.bound([id, 0], [id, Number.MAX_SAFE_INTEGER]);
const metadata = (job) => {
  const { objects, ...state } = job;
  return { ...state, objectCount: objects?.length ?? state.objectCount ?? 0 };
};
export function upgradeOutbox(db, tx) {
  const objects = db.createObjectStore("jobObjects", {
    keyPath: ["job", "index"],
  });
  const query = tx.objectStore("jobs").openCursor();
  query.onsuccess = () => {
    const row = query.result;
    if (!row) return;
    for (const [index, object] of (row.value.objects ?? []).entries())
      objects.add({ job: row.key, index, object });
    row.update(metadata(row.value));
    row.continue();
  };
}
// Signed identity and bytes are immutable. This module owns state, deletion,
// and bounded delivery reads so callers never reload a graph to update progress.
export class PublicationOutbox {
  constructor(db) {
    this.db = db;
  }
  list() {
    return transaction(this.db, "jobs", "readonly", (tx) =>
      request(tx.objectStore("jobs").getAll()),
    );
  }
  get(id) {
    return transaction(this.db, "jobs", "readonly", (tx) =>
      request(tx.objectStore("jobs").get(id)),
    );
  }
  save(job) {
    return transaction(this.db, "jobs", "readwrite", async (tx) => {
      const jobs = tx.objectStore("jobs"),
        previous = await request(jobs.get(job.id));
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
        for (const key of ["cursor", "state", "message", "projected"])
          if (Object.hasOwn(update, key)) next[key] = update[key];
        return next;
      });
      jobs.put({
        ...previous,
        recipients,
        cancelled: previous.cancelled || job.cancelled,
      });
    });
  }
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
        const legacy = await request(meta.get("lastPublication"));
        if (legacy) {
          const key = "publication:" + legacy.owner,
            saved = await request(meta.get(key));
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
  batch(id, start) {
    if (!Number.isSafeInteger(start) || start < 0)
      throw Error("Invalid outbox cursor");
    return transaction(
      this.db,
      ["jobs", "jobObjects"],
      "readonly",
      async (tx) => {
        const job = await request(tx.objectStore("jobs").get(id));
        if (!job || job.cancelled) throw Error("Snapshot cancelled or missing");
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
            const object = row.value.object;
            if (objects.length && bytes + object.data.length > 4 * 1024 * 1024)
              return false;
            if (
              row.value.index !== start + objects.length ||
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
  preferences() {
    return transaction(this.db, "meta", "readonly", async (tx) => {
      const meta = tx.objectStore("meta");
      const [relays, revision] = await Promise.all([
        request(meta.get("relays")),
        request(meta.get("relayRevision")),
      ]);
      return { relays: relays ?? [], revision: revision ?? 0 };
    });
  }
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
      const previous = (await request(meta.get("relays"))) ?? [];
      const removed = previous.filter(
        (old) =>
          !relays.some(
            (next) =>
              next.origin === old.origin && next.serviceKey === old.serviceKey,
          ),
      );
      if (removed.length) {
        for (const job of await request(jobs.getAll()))
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
  remove(id) {
    return transaction(this.db, ["jobs", "jobObjects"], "readwrite", (tx) => {
      tx.objectStore("jobs").delete(id);
      tx.objectStore("jobObjects").delete(inventoryRange(id));
    });
  }
}
