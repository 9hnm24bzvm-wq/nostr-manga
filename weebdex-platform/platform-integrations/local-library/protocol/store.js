import { families, normalize, recordKey, integer, unhex } from "./codec.js";
export const DB_NAME = "manga-local-library-v1";
import { request, transaction, cursor } from "./indexeddb.js";
import { PublicationOutbox, upgradeOutbox } from "./outbox.js";
import { MAX_RECORDS } from "./limits.js";
import { reclaimBackupStage } from "./backup.js";
export class LocalLibrary {
  static async open(name = DB_NAME) {
    const q = indexedDB.open(name, 5);
    q.onupgradeneeded = (event) => {
      const db = q.result;
      if (event.oldVersion < 1) {
        const rows = db.createObjectStore("records", {
          keyPath: ["family", "key"],
        });
        rows.createIndex("family", "family");
        db.createObjectStore("meta");
        db.createObjectStore("jobs", { keyPath: "id" });
      }
      if (event.oldVersion < 2) upgradeOutbox(db, q.transaction);
      if (event.oldVersion < 3)
        db.createObjectStore("backupStage", { keyPath: ["family", "key"] });
      if (event.oldVersion < 5) {
        // Count existing keys once during upgrade, including over-budget legacy
        // libraries. Normal reads and mutations never recount the record ranges.
        const statistics = { total: 0 };
        let pending = families.length;
        for (const family of families) {
          const count = q.transaction
            .objectStore("records")
            .index("family")
            .count(IDBKeyRange.only(family));
          count.onsuccess = () => {
            statistics[family] = count.result;
            statistics.total += count.result;
            if (--pending === 0) {
              const meta = q.transaction.objectStore("meta");
              meta.put(statistics, "recordStatistics");
              meta.delete("recordCount");
            }
          };
        }
      }
    };
    const db = await new Promise((resolve, reject) => {
      let blocked = false;
      q.onblocked = () => {
        blocked = true;
        reject(
          Error(
            "Local database upgrade blocked by another tab; close other tabs and reload",
          ),
        );
      };
      q.onerror = () => reject(q.error);
      q.onsuccess = () => {
        // IndexedDB cannot cancel an issued open request. If an old tab closes
        // after the rejection, release the eventual connection rather than leak it.
        if (blocked) q.result.close();
        else resolve(q.result);
      };
    });
    db.onversionchange = () => db.close();
    await reclaimBackupStage(db);
    return new LocalLibrary(db);
  }
  constructor(db) {
    this.db = db;
    this.outbox = new PublicationOutbox(db);
    this.channel =
      typeof BroadcastChannel === "function"
        ? new BroadcastChannel(db.name)
        : null;
  }
  close() {
    this.channel?.close();
    this.db.close();
  }
  async write(family, value) {
    const record = normalize(family, value);
    const revision = await transaction(
      this.db,
      ["records", "meta"],
      "readwrite",
      async (tx) => {
        const rows = tx.objectStore("records"),
          meta = tx.objectStore("meta"),
          key = recordKey(family, record);
        const [existing, statistics] = await Promise.all([
          request(rows.getKey([family, key])),
          request(meta.get("recordStatistics")),
        ]);
        if (existing === undefined && statistics.total >= MAX_RECORDS)
          throw Error(
            "Local library record limit reached; remove records before adding new ones",
          );
        rows.put({
          family,
          key,
          record,
        });
        if (existing === undefined) {
          statistics[family]++;
          statistics.total++;
          meta.put(statistics, "recordStatistics");
        }
        const revision = ((await request(meta.get("revision"))) ?? 0) + 1;
        meta.put(revision, "revision");
        return revision;
      },
    );
    this.channel?.postMessage({ revision });
    return revision;
  }
  async remove(family, key) {
    if (!families.includes(family)) throw Error("Invalid family");
    const revision = await transaction(
      this.db,
      ["records", "meta"],
      "readwrite",
      async (tx) => {
        const rows = tx.objectStore("records"),
          meta = tx.objectStore("meta");
        const [existing, statistics] = await Promise.all([
          request(rows.getKey([family, key])),
          request(meta.get("recordStatistics")),
        ]);
        if (existing !== undefined) {
          statistics[family]--;
          statistics.total--;
        }
        rows.delete([family, key]);
        if (family === "lists")
          await cursor(
            rows.index("family").openCursor(IDBKeyRange.only("members")),
            (row) => {
              if (row.value.record.list === key) {
                row.delete();
                statistics.members--;
                statistics.total--;
              }
            },
          );
        meta.put(statistics, "recordStatistics");
        const revision = ((await request(meta.get("revision"))) ?? 0) + 1;
        meta.put(revision, "revision");
        return revision;
      },
    );
    this.channel?.postMessage({ revision });
  }
  async list(family, after = null, limit = 100) {
    if (!families.includes(family)) throw Error("Invalid family");
    integer(limit, 500);
    if (!limit) return [];
    return transaction(this.db, "records", "readonly", async (tx) => {
      const range = IDBKeyRange.bound(
        [family, after ?? ""],
        [family, "\uffff"],
        after !== null,
      );
      const rows = await request(
        tx.objectStore("records").getAll(range, limit),
      );
      return rows.map((row) => row.record);
    });
  }

  counts() {
    return transaction(this.db, "meta", "readonly", async (tx) => {
      const meta = tx.objectStore("meta");
      const [statistics, revision] = await Promise.all([
        request(meta.get("recordStatistics")),
        request(meta.get("revision")),
      ]);
      return {
        ...statistics,
        revision: revision ?? 0,
        recordLimit: MAX_RECORDS,
        overBudget: statistics.total > MAX_RECORDS,
      };
    });
  }
  snapshot() {
    return transaction(this.db, ["records", "meta"], "readonly", async (tx) => {
      const revision = await request(tx.objectStore("meta").get("revision"));
      const records = Object.fromEntries(
        families.map((family) => [family, []]),
      );
      await cursor(tx.objectStore("records").openCursor(), (row) => {
        records[row.value.family].push(row.value.record);
      });
      return {
        format: "manga-local-library",
        version: 1,
        revision: revision ?? 0,
        exported_at: new Date().toISOString(),
        records,
      };
    });
  }
  async replace(
    snapshot,
    expectedRevision,
    observedClock = null,
    expectedRelayRevision = null,
  ) {
    if (expectedRelayRevision !== null) integer(expectedRelayRevision);
    if (observedClock !== null) {
      unhex(observedClock.owner);
      unhex(observedClock.id);
      integer(observedClock.created_at);
      observedClock = {
        owner: observedClock.owner,
        id: observedClock.id,
        created_at: observedClock.created_at,
      };
    }
    if (
      snapshot?.format !== "manga-local-library" ||
      snapshot.version !== 1 ||
      !snapshot.records ||
      Object.keys(snapshot.records).sort().join("|") !==
        [...families].sort().join("|")
    )
      throw Error("Unsupported backup");
    const statistics = { total: 0 };
    const rows = [];
    for (const family of families) {
      const records = snapshot.records[family];
      if (
        !Array.isArray(records) ||
        (statistics.total += records.length) > MAX_RECORDS
      )
        throw Error("Import record limit");
      statistics[family] = records.length;
      const seen = new Set();
      for (const r of records) {
        const record = normalize(family, r),
          key = recordKey(family, record);
        if (seen.has(key)) throw Error("Duplicate import record");
        seen.add(key);
        rows.push({ family, key, record });
      }
    }
    const revision = await transaction(
      this.db,
      ["records", "meta"],
      "readwrite",
      async (tx) => {
        const m = tx.objectStore("meta");
        const old = (await request(m.get("revision"))) ?? 0;
        if (old !== expectedRevision)
          throw Error("Local library changed. Review the import again.");
        if (
          expectedRelayRevision !== null &&
          ((await request(m.get("relayRevision"))) ?? 0) !==
            expectedRelayRevision
        )
          throw Error("Instance selection changed; review recovery again");
        if (observedClock) {
          const key = "publication:" + observedClock.owner;
          const [saved, legacy] = await Promise.all([
            request(m.get(key)),
            request(m.get("lastPublication")),
          ]);
          let newest = observedClock;
          for (const candidate of [saved, legacy])
            if (
              candidate?.owner === observedClock.owner &&
              candidate.created_at > newest.created_at
            )
              newest = candidate;
          m.put(newest, key);
        }
        tx.objectStore("records").clear();
        for (const row of rows) tx.objectStore("records").put(row);
        m.put(statistics, "recordStatistics");
        m.put(old + 1, "revision");
        return old + 1;
      },
    );
    this.channel?.postMessage({ revision });
    return revision;
  }
  getMeta(key) {
    return transaction(this.db, "meta", "readonly", (tx) =>
      request(tx.objectStore("meta").get(key)),
    );
  }
  relayPreferences() {
    return this.outbox.preferences();
  }
  setRelays(relays, expectedRevision) {
    return this.outbox.setRelays(relays, expectedRevision);
  }
  publicationClock(owner, jobs = []) {
    return transaction(this.db, "meta", "readonly", async (tx) => {
      const meta = tx.objectStore("meta");
      const [saved, legacy] = await Promise.all([
        request(meta.get("publication:" + owner)),
        request(meta.get("lastPublication")),
      ]);
      let timestamp = null;
      for (const candidate of [
        saved,
        legacy,
        ...jobs.map((job) => ({
          owner: job.event.pubkey,
          created_at: job.event.created_at,
        })),
      ]) {
        if (candidate?.owner !== owner) continue;
        integer(candidate.created_at);
        timestamp = Math.max(timestamp ?? 0, candidate.created_at);
      }
      return timestamp;
    });
  }
  jobs() {
    return this.outbox.list();
  }
  job(id) {
    return this.outbox.get(id);
  }
  saveJob(job) {
    return this.outbox.save(job);
  }
  createJob(job) {
    return this.outbox.create(job);
  }
  jobBatch(id, start) {
    return this.outbox.batch(id, start);
  }
  deleteJob(id) {
    return this.outbox.remove(id);
  }
}
