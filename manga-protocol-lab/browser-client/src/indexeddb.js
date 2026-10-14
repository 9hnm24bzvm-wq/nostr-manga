// Internal IndexedDB adapter. Every operation owns completion and abort lifetime.
/** @template T
 * @param {IDBRequest<T>} query
 * @returns {Promise<T>}
 */
export const request = (query) =>
  new Promise((resolve, reject) => {
    query.onsuccess = () => resolve(query.result);
    query.onerror = () => reject(query.error);
  });
/** @param {IDBTransaction} transaction
 * @returns {Promise<Event>}
 */
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
/** @template T
 * @param {IDBDatabase} db
 * @param {string | string[]} stores
 * @param {IDBTransactionMode} mode
 * @param {(transaction: IDBTransaction) => T | PromiseLike<T>} operation
 * @returns {Promise<T>}
 */
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
/** @param {IDBRequest<IDBCursorWithValue | null>} query
 * @param {(row: IDBCursorWithValue) => void | boolean} visit
 * @returns {Promise<void>}
 */
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
