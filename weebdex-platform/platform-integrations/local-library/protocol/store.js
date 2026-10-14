/** @typedef {import("./types.js").Family} Family
 * @typedef {import("./types.js").LibraryRecord} LibraryRecord
 * @typedef {import("./types.js").RecordRow} RecordRow
 * @typedef {import("./types.js").Records} Records
 * @typedef {import("./types.js").Statistics} Statistics
 * @typedef {import("./types.js").Counts} Counts
 * @typedef {import("./types.js").LocalSnapshot} LocalSnapshot
 * @typedef {import("./types.js").PublicationClock} PublicationClock
 * @typedef {import("./types.js").Relay} Relay
 * @typedef {import("./types.js").Job} Job
 * @typedef {import("./types.js").NewJob} NewJob
 */
import {
  families,
  isFamily,
  normalize,
  recordKey,
  integer,
  unhex,
} from "./codec.js";
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
      // An upgrade request owns a live version-change transaction here.
      const upgrade = q.transaction;
      if (!upgrade) throw Error("Missing local database upgrade transaction");
      if (event.oldVersion < 1) {
        const rows = db.createObjectStore("records", {
          keyPath: ["family", "key"],
        });
        rows.createIndex("family", "family");
        db.createObjectStore("meta");
        db.createObjectStore("jobs", { keyPath: "id" });
      }
      if (event.oldVersion < 2) upgradeOutbox(db, upgrade);
      if (event.oldVersion < 3)
        db.createObjectStore("backupStage", { keyPath: ["family", "key"] });
      if (event.oldVersion < 5) {
        // Count existing keys once during upgrade, including over-budget legacy
        // libraries. Normal reads and mutations never recount the record ranges.
        /** @type {Statistics} */
        const statistics = {
          total: 0,
          chapters: 0,
          series: 0,
          lists: 0,
          members: 0,
        };
        let pending = families.length;
        for (const family of families) {
          const count = upgrade
            .objectStore("records")
            .index("family")
            .count(IDBKeyRange.only(family));
          count.onsuccess = () => {
            statistics[family] = count.result;
            statistics.total += count.result;
            if (--pending === 0) {
              const meta = upgrade.objectStore("meta");
              meta.put(statistics, "recordStatistics");
              meta.delete("recordCount");
            }
          };
        }
      }
    };
    /** @type {IDBDatabase} */
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
  /** @param {IDBDatabase} db */
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
  /** @param {string} family
   * @param {unknown} value
   */
  async write(family, value) {
    if (!isFamily(family)) throw Error("Invalid library record");
    const record = normalize(family, value);
    const revision = await transaction(
      this.db,
      ["records", "meta"],
      "readwrite",
      async (tx) => {
        const rows = tx.objectStore("records"),
          meta = tx.objectStore("meta"),
          key = recordKey(family, record);
        /** @type {[IDBValidKey | undefined, Statistics]} */
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
  /** @param {string} family
   * @param {string} key
   */
  async remove(family, key) {
    if (!isFamily(family)) throw Error("Invalid family");
    const revision = await transaction(
      this.db,
      ["records", "meta"],
      "readwrite",
      async (tx) => {
        const rows = tx.objectStore("records"),
          meta = tx.objectStore("meta");
        /** @type {[IDBValidKey | undefined, Statistics]} */
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
  /** @param {string} family
   * @param {string | null} [after]
   * @param {number} [limit]
   * @returns {Promise<LibraryRecord[]>}
   */
  async list(family, after = null, limit = 100) {
    if (!isFamily(family)) throw Error("Invalid family");
    integer(limit, 500);
    if (!limit) return [];
    return transaction(this.db, "records", "readonly", async (tx) => {
      const range = IDBKeyRange.bound(
        [family, after ?? ""],
        [family, "\uffff"],
        after !== null,
      );
      /** @type {RecordRow[]} */
      const rows = await request(
        tx.objectStore("records").getAll(range, limit),
      );
      return rows.map((row) => row.record);
    });
  }

  /** @returns {Promise<Counts>} */
  counts() {
    return transaction(this.db, "meta", "readonly", async (tx) => {
      const meta = tx.objectStore("meta");
      /** @type {[Statistics, number | undefined]} */
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
  /** @returns {Promise<LocalSnapshot>} */
  snapshot() {
    return transaction(this.db, ["records", "meta"], "readonly", async (tx) => {
      const revision = await request(tx.objectStore("meta").get("revision"));
      /** @type {Records} */
      const records = { chapters: [], series: [], lists: [], members: [] };
      await cursor(tx.objectStore("records").openCursor(), (row) => {
        /** @type {RecordRow} */
        const saved = row.value;
        records[saved.family].push(saved.record);
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
  /** @param {unknown} snapshot
   * @param {number} expectedRevision
   * @param {PublicationClock | null} [observedClock]
   * @param {number | null} [expectedRelayRevision]
   */
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
      !snapshot ||
      typeof snapshot !== "object" ||
      !("format" in snapshot) ||
      snapshot.format !== "manga-local-library" ||
      !("version" in snapshot) ||
      snapshot.version !== 1 ||
      !("records" in snapshot) ||
      !snapshot.records ||
      typeof snapshot.records !== "object" ||
      Object.keys(snapshot.records).sort().join("|") !==
        [...families].sort().join("|")
    )
      throw Error("Unsupported backup");
    /** @type {Statistics} */
    const statistics = {
      total: 0,
      chapters: 0,
      series: 0,
      lists: 0,
      members: 0,
    };
    /** @type {RecordRow[]} */
    const rows = [];
    // The exact family-key check above establishes this dictionary boundary.
    const recordsByFamily = /** @type {Record<string, unknown>} */ (
      snapshot.records
    );
    for (const family of families) {
      const records = recordsByFamily[family];
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
          /** @type {[PublicationClock | undefined, PublicationClock | undefined]} */
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
  /** @overload
   * @param {"revision" | "relayRevision"} key
   * @returns {Promise<number | undefined>}
   */
  /** @overload
   * @param {"relays"} key
   * @returns {Promise<Relay[] | undefined>}
   */
  /** @overload
   * @param {string} key
   * @returns {Promise<unknown>}
   */
  /** @param {string} key
   * @returns {Promise<unknown>}
   */
  getMeta(key) {
    return transaction(this.db, "meta", "readonly", (tx) =>
      request(tx.objectStore("meta").get(key)),
    );
  }
  relayPreferences() {
    return this.outbox.preferences();
  }
  /** @param {Relay[]} relays
   * @param {number} [expectedRevision]
   */
  setRelays(relays, expectedRevision) {
    return this.outbox.setRelays(relays, expectedRevision);
  }
  /** @param {string} owner
   * @param {Job[]} [jobs]
   * @returns {Promise<number | null>}
   */
  publicationClock(owner, jobs = []) {
    return transaction(this.db, "meta", "readonly", async (tx) => {
      const meta = tx.objectStore("meta");
      /** @type {[PublicationClock | undefined, PublicationClock | undefined]} */
      const [saved, legacy] = await Promise.all([
        request(meta.get("publication:" + owner)),
        request(meta.get("lastPublication")),
      ]);
      /** @type {number | null} */
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
  /** @param {string} id */
  job(id) {
    return this.outbox.get(id);
  }
  /** @param {Job} job */
  saveJob(job) {
    return this.outbox.save(job);
  }
  /** @param {NewJob} job */
  createJob(job) {
    return this.outbox.create(job);
  }
  /** @param {string} id
   * @param {number} start */
  jobBatch(id, start) {
    return this.outbox.batch(id, start);
  }
  /** @param {string} id */
  deleteJob(id) {
    return this.outbox.remove(id);
  }
}
