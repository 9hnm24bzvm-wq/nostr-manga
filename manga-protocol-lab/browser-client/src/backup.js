/** @typedef {import("./store.js").LocalLibrary} LocalLibrary
 * @typedef {import("./types.js").RecordRow} RecordRow
 * @typedef {import("./types.js").Family} Family
 * @typedef {Pick<Blob, "size" | "slice" | "text">} BackupFile
 */
import { families, isFamily, normalize, recordKey } from "./codec.js";
import { request, transaction, cursor } from "./indexeddb.js";
import { MAX_RECORDS } from "./limits.js";
const PAGE = 500,
  PART = 65536,
  MAX_FILE = 4 * 1024 ** 3;
const header = { format: "manga-local-library", version: 2 };
/** @param {IDBDatabase} db */
const importLock = (db) => "manga-backup-import:" + db.name;
/** @param {IDBDatabase} db */
const clearStage = (db) =>
  transaction(db, "backupStage", "readwrite", (tx) => {
    tx.objectStore("backupStage").clear();
  });
// Internal lifecycle seam used by LocalLibrary.open. Only the import lock proves
// that no live tab owns these temporary records; age and tab IDs do not.
/** @param {IDBDatabase} db */
export async function reclaimBackupStage(db) {
  if (!globalThis.navigator?.locks) return;
  try {
    await navigator.locks.request(
      importLock(db),
      { ifAvailable: true },
      async (lock) => {
        if (lock) await clearStage(db);
      },
    );
  } catch {
    // Reclamation is best-effort; storage/lock failure must not hide saved data.
  }
}
/** @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
const object = (value) => value !== null && typeof value === "object";
const utf8 = new TextEncoder(),
  decoder = new TextDecoder("utf-8", { fatal: true });
/** @param {unknown} value */
function line(value) {
  const result = JSON.stringify(value) + "\n";
  if (utf8.encode(result).length > PART) throw Error("Backup line limit");
  return result;
}
/** @param {LocalLibrary} store */
export async function* exportBackup(store) {
  yield* exportRecords(store, false);
}
// Rescue streams intentionally are not normal importable backups: old libraries
// may exceed the supported total. Consumers must stream to a sink, then review
// and split records into supported libraries using write/replace/import.
/** @param {LocalLibrary} store */
export async function* exportRescue(store) {
  yield* exportRecords(store, true);
}
/** @param {LocalLibrary} store
 * @param {boolean} rescue
 */
async function* exportRecords(store, rescue) {
  const { revision, overBudget } = await store.counts();
  if (overBudget && !rescue)
    throw Error(
      "Backup record limit; use the streaming rescue export for this legacy library",
    );
  yield line({
    ...(rescue ? { format: "manga-local-library-rescue", version: 1 } : header),
    revision,
    exported_at: new Date().toISOString(),
  });
  let count = 0;
  for (const family of families) {
    let after = null;
    for (;;) {
      const rows = await store.list(family, after, PAGE);
      if (((await store.getMeta("revision")) ?? 0) !== revision)
        throw Error("Local library changed during export; try again");
      for (const record of rows) {
        if (++count > MAX_RECORDS && !rescue)
          throw Error("Backup record limit");
        yield line([family, record]);
      }
      if (rows.length < PAGE) break;
      after = recordKey(family, rows[rows.length - 1]);
    }
  }
  if (((await store.getMeta("revision")) ?? 0) !== revision)
    throw Error("Local library changed during export; try again");
  yield line({ end: count });
}
/** @param {LocalLibrary} store */
export async function backupBlob(store) {
  const parts = [];
  let buffer = new Uint8Array(PART),
    used = 0,
    size = 0;
  for await (const row of exportBackup(store)) {
    const bytes = utf8.encode(row);
    size += bytes.length;
    if (size > MAX_FILE) throw Error("Backup file limit");
    for (let offset = 0; offset < bytes.length; ) {
      const length = Math.min(PART - used, bytes.length - offset);
      buffer.set(bytes.subarray(offset, offset + length), used);
      used += length;
      offset += length;
      if (used === PART) {
        parts.push(new Blob([buffer]));
        buffer = new Uint8Array(PART);
        used = 0;
      }
    }
  }
  if (used) parts.push(new Blob([buffer.subarray(0, used)]));
  return new Blob(parts, { type: "application/x-ndjson" });
}
/** @param {BackupFile} file
 * @returns {AsyncGenerator<unknown>}
 */
async function* lines(file) {
  let buffer = new Uint8Array(PART),
    used = 0;
  for (let offset = 0; offset < file.size; offset += PART) {
    const bytes = new Uint8Array(
      await file.slice(offset, offset + PART).arrayBuffer(),
    );
    if (bytes.length > PART) throw Error("Backup read limit");
    let start = 0;
    for (let end = 0; end < bytes.length; end++) {
      if (bytes[end] !== 10) continue;
      const segment = bytes.subarray(start, end);
      if (used + segment.length + 1 > PART) throw Error("Backup line limit");
      buffer.set(segment, used);
      used += segment.length;
      if (!used) throw Error("Empty backup record");
      yield JSON.parse(decoder.decode(buffer.subarray(0, used)));
      used = 0;
      start = end + 1;
    }
    const rest = bytes.subarray(start);
    if (used + rest.length >= PART) throw Error("Backup line limit");
    buffer.set(rest, used);
    used += rest.length;
  }
  if (used) throw Error("Truncated backup");
}
/** @param {LocalLibrary} store
 * @param {BackupFile} file
 * @param {number} expectedRevision
 */
export async function importBackup(store, file, expectedRevision) {
  if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > MAX_FILE)
    throw Error("Backup file limit");
  const beginning = await file.slice(0, 4096).text();
  /** @type {unknown} */
  let first;
  try {
    first = JSON.parse(beginning.split("\n")[0]);
  } catch {}
  if (object(first) && first.format === "manga-local-library-rescue")
    throw Error(
      "Unsupported backup: split the rescue archive into supported libraries before import",
    );
  if (!object(first) || first.format !== header.format || first.version !== 2) {
    if (file.size > 256 * 1024 * 1024)
      throw Error(
        "Legacy JSON backup exceeds 256 MiB; use a version-two backup",
      );
    return store.replace(JSON.parse(await file.text()), expectedRevision);
  }
  if (!globalThis.navigator?.locks)
    throw Error("Web Locks required for safe streamed backup import");
  return navigator.locks.request(importLock(store.db), async () => {
    const clear = () => clearStage(store.db);
    await clear();
    let count = 0,
      started = false,
      ended = false,
      committed = false;
    /** @type {RecordRow[]} */
    let batch = [];
    /** @type {Record<Family, number>} */
    const statistics = { chapters: 0, series: 0, lists: 0, members: 0 };
    const stage = async () => {
      await transaction(store.db, "backupStage", "readwrite", (tx) => {
        for (const row of batch) tx.objectStore("backupStage").add(row);
      });
      batch = [];
    };
    try {
      for await (const value of lines(file)) {
        if (!started) {
          if (
            !object(value) ||
            value.format !== header.format ||
            value.version !== 2
          )
            throw Error("Unsupported backup");
          started = true;
          continue;
        }
        if (ended) throw Error("Data after backup footer");
        if (!Array.isArray(value)) {
          if (
            !object(value) ||
            value.end !== count ||
            Object.keys(value).join() !== "end"
          )
            throw Error("Invalid backup footer");
          ended = true;
          continue;
        }
        if (value.length !== 2 || !isFamily(value[0]) || ++count > MAX_RECORDS)
          throw Error("Invalid backup record or record limit");
        const family = value[0],
          record = normalize(family, value[1]);
        statistics[family]++;
        batch.push({ family, key: recordKey(family, record), record });
        if (batch.length === PAGE) await stage();
      }
      if (!ended) throw Error("Truncated backup");
      if (batch.length) await stage();
      const revision = await transaction(
        store.db,
        ["records", "meta", "backupStage"],
        "readwrite",
        async (tx) => {
          const meta = tx.objectStore("meta"),
            old = (await request(meta.get("revision"))) ?? 0;
          if (old !== expectedRevision)
            throw Error("Local library changed. Review the import again.");
          const rows = tx.objectStore("records");
          rows.clear();
          await cursor(tx.objectStore("backupStage").openCursor(), (row) => {
            rows.put(row.value);
          });
          meta.put({ ...statistics, total: count }, "recordStatistics");
          meta.put(old + 1, "revision");
          tx.objectStore("backupStage").clear();
          return old + 1;
        },
      );
      committed = true;
      store.channel?.postMessage({ revision });
      return revision;
    } finally {
      if (!committed) await clear().catch(() => {});
    }
  });
}
