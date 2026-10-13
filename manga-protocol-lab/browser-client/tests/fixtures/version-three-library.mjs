// Frozen pre-capacity schema input. Tests assert outcomes only through current
// library/outbox operations; this fixture can retain an older open connection.
export async function versionThreeLibrary(
  name,
  { records = [], jobs = [], objects = [] } = {},
) {
  const query = indexedDB.open(name, 3);
  query.onupgradeneeded = () => {
    const db = query.result;
    db.createObjectStore("records", { keyPath: ["family", "key"] }).createIndex(
      "family",
      "family",
    );
    db.createObjectStore("meta");
    db.createObjectStore("jobs", { keyPath: "id" });
    db.createObjectStore("jobObjects", { keyPath: ["job", "index"] });
    db.createObjectStore("backupStage", { keyPath: ["family", "key"] });
  };
  const db = await new Promise((resolve, reject) => {
    query.onsuccess = () => resolve(query.result);
    query.onerror = () => reject(query.error);
  });
  const tx = db.transaction(["records", "jobs", "jobObjects"], "readwrite");
  const done = new Promise((resolve, reject) => {
    tx.oncomplete = resolve;
    tx.onabort = () => reject(tx.error);
  });
  for (const row of records) tx.objectStore("records").add(row);
  for (const job of jobs) tx.objectStore("jobs").add(job);
  for (const object of objects) tx.objectStore("jobObjects").add(object);
  await done;
  return db;
}
