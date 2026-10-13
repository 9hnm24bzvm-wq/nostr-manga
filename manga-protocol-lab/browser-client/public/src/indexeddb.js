// Internal IndexedDB adapter. Every operation owns completion and abort lifetime.
export const request = (query) =>
  new Promise((resolve, reject) => {
    query.onsuccess = () => resolve(query.result);
    query.onerror = () => reject(query.error);
  });
export function completed(transaction) {
  const done = new Promise((resolve, reject) => {
    transaction.oncomplete = resolve;
    transaction.onabort = () =>
      reject(transaction.error ?? Error("Local transaction aborted"));
    transaction.onerror = () => {};
  });
  done.catch(() => {}); // Requests may reject before the caller reaches completion.
  return done;
}
export async function transaction(db, stores, mode, operation) {
  const tx = db.transaction(stores, mode),
    done = completed(tx);
  try {
    const result = await operation(tx);
    await done;
    return result;
  } catch (error) {
    try {
      tx.abort();
    } catch {}
    await done.catch(() => {});
    throw error;
  }
}
export const cursor = (query, visit) =>
  new Promise((resolve, reject) => {
    query.onerror = () => reject(query.error);
    query.onsuccess = () => {
      const row = query.result;
      try {
        if (!row || visit(row) === false) return resolve();
        row.continue();
      } catch (error) {
        reject(error);
      }
    };
  });
