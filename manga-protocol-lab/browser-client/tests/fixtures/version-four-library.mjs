import { versionThreeLibrary } from "./version-three-library.mjs";

// Frozen pre-family-statistics input, including the old capacity total. This
// fixture only arranges old data; assertions use the current public operations.
export async function versionFourLibrary(name, input = {}) {
  const old = await versionThreeLibrary(name, input);
  old.close();
  const query = indexedDB.open(name, 4);
  query.onupgradeneeded = () => {
    const meta = query.transaction.objectStore("meta");
    meta.put(input.records?.length ?? 0, "recordCount");
    meta.put(input.revision ?? 0, "revision");
  };
  return new Promise((resolve, reject) => {
    query.onsuccess = () => resolve(query.result);
    query.onerror = () => reject(query.error);
  });
}
