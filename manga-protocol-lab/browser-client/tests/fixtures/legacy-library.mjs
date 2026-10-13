// Frozen version-one persisted input; never inspect current storage to assert results.
export async function legacyLibrary(
  name,
  { records = [], jobs = [], metadata = {} } = {},
) {
  const query = indexedDB.open(name, 1);
  query.onupgradeneeded = () => {
    const db = query.result;
    db.createObjectStore("records", { keyPath: ["family", "key"] }).createIndex(
      "family",
      "family",
    );
    db.createObjectStore("meta");
    db.createObjectStore("jobs", { keyPath: "id" });
  };
  const db = await new Promise((resolve, reject) => {
    query.onsuccess = () => resolve(query.result);
    query.onerror = () => reject(query.error);
  });
  try {
    const tx = db.transaction(["records", "jobs", "meta"], "readwrite");
    const done = new Promise((resolve, reject) => {
      tx.oncomplete = resolve;
      tx.onabort = () => reject(tx.error);
    });
    for (const row of records) tx.objectStore("records").add(row);
    for (const job of jobs) tx.objectStore("jobs").add(job);
    for (const [key, value] of Object.entries(metadata))
      tx.objectStore("meta").put(value, key);
    await done;
  } finally {
    db.close();
  }
}
