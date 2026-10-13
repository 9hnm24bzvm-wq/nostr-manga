// Native catalogue identifiers are a device-only extension, never protocol IDs.
import { request, transaction } from './protocol/indexeddb.js';
import { normalize, integer, hex } from './protocol/codec.js';
import { MAX_RECORDS } from './protocol/limits.js';
export const DEVICE_DATABASE = 'weebdex-device-library-v1';
export const DEVICE_LOCK = 'weebdex-device-library-write';
export const statuses = ['reading', 'on_hold', 'plan_to_read', 'dropped', 're_reading', 'completed'] as const;
export type Family = 'series' | 'chapters' | 'lists' | 'members';
export type DeviceRecord = { family: Family; key: string; id?: string; mangaId?: string; status?: string; notification?: boolean; followedAt?: string; completed?: boolean; page?: number | null; readAt?: string; list?: string; series?: string; position?: number; name?: string; updated_at: number };
export type InstanceScope = { origin: string; serviceKey: string | null; catalogNamespace: string | null };
export const sameScope = (a: InstanceScope, b: InstanceScope) => a.origin === b.origin && a.catalogNamespace === b.catalogNamespace && a.serviceKey === b.serviceKey;
export type DeviceSnapshot = { scope: InstanceScope; format: 'weebdex-device-library'; version: 1; revision: number; exported_at: string; records: DeviceRecord[]; migration_reason?: string };
export const catalogueId = (id: unknown): string => {
  if (typeof id !== 'string' || !/^[a-zA-Z0-9]{10}$/.test(id)) throw Error('Invalid catalogue identifier');
  return id;
};
const date = (value: unknown): string => {
  if (typeof value !== 'string' || !Number.isFinite(Date.parse(value)) || value.length > 40) throw Error('Invalid library date');
  return value;
};
export function normalizeDevice(value: DeviceRecord): DeviceRecord {
  const updated_at = integer(value.updated_at ?? Math.floor(Date.now() / 1000));
  if (value.family === 'series') {
    const id = catalogueId(value.id);
    if (!statuses.includes(value.status as typeof statuses[number])) throw Error('Invalid reading status');
    if (typeof value.notification !== 'boolean') throw Error('Invalid notification preference');
    return { family: 'series', key: id, id, status: value.status, notification: value.notification, followedAt: date(value.followedAt), updated_at };
  }
  if (value.family === 'chapters') {
    const id = catalogueId(value.id), mangaId = catalogueId(value.mangaId);
    if (typeof value.completed !== 'boolean') throw Error('Invalid completion state');
    return { family: 'chapters', key: id, id, mangaId, completed: value.completed, page: value.page == null ? null : integer(value.page, 0xffffffff), readAt: value.readAt === undefined ? undefined : date(value.readAt), updated_at };
  }
  if (value.family === 'lists') {
    const record = normalize('lists', value);
    return { family: 'lists', key: record.id, ...record };
  }
  if (value.family === 'members') {
    // Lists use the original 128-bit IDs. Native series refs remain device-only.
    if (typeof value.list !== 'string' || !/^[0-9a-f]{32}$/.test(value.list)) throw Error('Invalid list identifier');
    const series = catalogueId(value.series);
    return { family: 'members', key: value.list + ':' + series, list: value.list, series, position: integer(value.position), updated_at };
  }
  throw Error('Unsupported device record');
}
export function validateSnapshot(input: unknown): DeviceSnapshot {
  const value = input as DeviceSnapshot;
  if (!value?.scope || (value.scope.catalogNamespace !== null && (typeof value.scope.catalogNamespace !== 'string' || !/^[0-9a-f-]{32,36}$/.test(value.scope.catalogNamespace))) || typeof value.scope.origin !== 'string' || new URL(value.scope.origin).origin !== value.scope.origin || (value.scope.serviceKey !== null && !/^[0-9a-f]{64}$/.test(value.scope.serviceKey))) throw Error('Backup is missing a trustworthy instance scope');
  if (value?.format !== 'weebdex-device-library' || value.version !== 1 || !Array.isArray(value.records) || value.records.length > MAX_RECORDS) throw Error('Unsupported or oversized device backup');
  const records = value.records.map(normalizeDevice), seen = new Set<string>();
  for (const row of records) {
    const key = row.family + ':' + row.key;
    if (seen.has(key)) throw Error('Duplicate backup record');
    seen.add(key);
  }
  const lists = new Set(records.filter(row => row.family === 'lists').map(row => row.id));
  if (records.some(row => row.family === 'members' && !lists.has(row.list))) throw Error('Backup contains an unknown list');
  return { scope: { origin: value.scope.origin, serviceKey: value.scope.serviceKey, catalogNamespace: value.scope.catalogNamespace }, format: 'weebdex-device-library', version: 1, revision: integer(value.revision), exported_at: date(value.exported_at), records };
}
export async function deviceWriteLock<T>(operation: () => Promise<T>): Promise<T> {
  return navigator.locks ? navigator.locks.request(DEVICE_LOCK, operation) : operation();
}
export class DeviceStore {
  readonly channel: BroadcastChannel | null;
  private closed = false;
  constructor(readonly db: IDBDatabase, public scope: InstanceScope, onChanged: () => void, onClosed: () => void) {
    this.channel = typeof BroadcastChannel === 'function' ? new BroadcastChannel(db.name) : null;
    if (this.channel) this.channel.onmessage = onChanged;
    db.onversionchange = () => { this.closed = true; db.close(); this.channel?.close(); onClosed(); };
  }
  static async open(scope: InstanceScope, onChanged: () => void, onClosed: () => void) {
    if (!globalThis.indexedDB) throw Error('This browser cannot provide durable IndexedDB storage');
    const query = indexedDB.open(DEVICE_DATABASE + ':' + encodeURIComponent(scope.origin), 1);
    query.onupgradeneeded = () => {
      query.result.createObjectStore('records', { keyPath: ['family', 'key'] });
      query.result.createObjectStore('meta');
      query.transaction!.objectStore('meta').put(0, 'revision');
      query.transaction!.objectStore('meta').put(0, 'count');
    };
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      let blocked = false;
      query.onblocked = () => { blocked = true; reject(Error('Close other WeebDex tabs and reload to upgrade local storage')); };
      query.onerror = () => reject(query.error ?? Error('Cannot open local storage'));
      query.onsuccess = () => { if (blocked) query.result.close(); else resolve(query.result); };
    });
    let bound: InstanceScope;
    try {
      bound = await transaction(db, ['records', 'meta'], 'readwrite', async (tx: IDBTransaction) => {
        const meta = tx.objectStore('meta'), remembered = await request(meta.get('instanceScope'));
        if (remembered?.serviceKey && scope.serviceKey && remembered.serviceKey !== scope.serviceKey) throw Error('This instance changed its protocol service identity. Existing device data is preserved; a reviewed migration is required.');
        if (remembered?.catalogNamespace && scope.catalogNamespace && remembered.catalogNamespace !== scope.catalogNamespace) throw Error('This instance rebuilt its native catalogue namespace. Existing device data is preserved; a reviewed migration is required.');
        const count = await request(tx.objectStore('records').count());
        const bound = { origin: scope.origin, serviceKey: remembered?.serviceKey ?? (count ? null : scope.serviceKey), catalogNamespace: remembered?.catalogNamespace ?? (count ? null : scope.catalogNamespace) };
        meta.put(bound, 'instanceScope'); return bound;
      });
    } catch (error) { db.close(); throw error; }
    return new DeviceStore(db, bound, onChanged, onClosed);
  }
  async verifyLiveScope(live: InstanceScope): Promise<{ status: 'verified' | 'origin_only' | 'migration_required'; reason?: string }> {
    this.ensureOpen();
    const result = await deviceWriteLock(() => transaction(this.db, ['records', 'meta'], 'readwrite', async (tx: IDBTransaction) => {
      const meta = tx.objectStore('meta'), scope: InstanceScope = await request(meta.get('instanceScope'));
      const previousReason = await request(meta.get('scopeMigrationReason'));
      if ((scope.catalogNamespace && live.catalogNamespace && scope.catalogNamespace !== live.catalogNamespace) || (scope.serviceKey && live.serviceKey && scope.serviceKey !== live.serviceKey) || scope.origin !== live.origin) {
        const reason = 'This instance changed its catalogue namespace or service identity. Saved records can be inspected and backed up in their previous scope; all native mutations are fenced until the original scope is verified or a migration is reviewed.';
        meta.put(reason, 'scopeMigrationReason');
        return { status: 'migration_required' as const, reason };
      }
      // An unavailable identity is not proof that a previously detected mismatch
      // disappeared. This durable fence is shared by already-open tabs.
      if (previousReason && (scope.catalogNamespace !== live.catalogNamespace || (scope.serviceKey && scope.serviceKey !== live.serviceKey))) return { status: 'migration_required' as const, reason: previousReason };
      const count = await request(tx.objectStore('records').count());
      if (!scope.catalogNamespace && count) return { status: 'origin_only' as const, reason: 'This populated origin-only library needs explicit adoption of the verified catalogue/service identity before catalogue hydration or publication.' };
      if (!live.catalogNamespace) throw Error('Current catalogue namespace is unavailable');
      if (!count && (!scope.catalogNamespace || !scope.serviceKey)) meta.put({ origin: scope.origin, catalogNamespace: scope.catalogNamespace ?? live.catalogNamespace, serviceKey: scope.serviceKey ?? live.serviceKey }, 'instanceScope');
      meta.delete('scopeMigrationReason');
      return { status: 'verified' as const };
    }));
    this.channel?.postMessage({ scopeStatus: result.status });
    return result;
  }
  private ensureOpen() { if (this.closed) throw Error('Local database changed in another tab. Reload before saving.'); }
  async snapshot(): Promise<DeviceSnapshot> {
    this.ensureOpen();
    return transaction(this.db, ['records', 'meta'], 'readonly', async (tx: IDBTransaction) => ({
      scope: await request(tx.objectStore('meta').get('instanceScope')), format: 'weebdex-device-library', version: 1, revision: (await request(tx.objectStore('meta').get('revision'))) ?? 0,
      exported_at: new Date().toISOString(), records: await request(tx.objectStore('records').getAll()), migration_reason: await request(tx.objectStore('meta').get('scopeMigrationReason'))
    }));
  }
  revision(): Promise<number> { this.ensureOpen(); return transaction(this.db, 'meta', 'readonly', (tx: IDBTransaction) => request(tx.objectStore('meta').get('revision'))); }
  publicationState(): Promise<{ revision: number; migration_reason?: string }> {
    this.ensureOpen();
    return transaction(this.db, 'meta', 'readonly', async (tx: IDBTransaction) => ({ revision: await request(tx.objectStore('meta').get('revision')), migration_reason: await request(tx.objectStore('meta').get('scopeMigrationReason')) }));
  }
  async mutate(operation: (rows: IDBObjectStore, meta: IDBObjectStore) => Promise<void>, expectedRevision?: number, alreadyLocked = false) {
    const run = async () => {
      this.ensureOpen();
      const revision = await transaction(this.db, ['records', 'meta'], 'readwrite', async (tx: IDBTransaction) => {
        const rows = tx.objectStore('records'), meta = tx.objectStore('meta'), old = (await request(meta.get('revision'))) ?? 0;
        const migrationReason = await request(meta.get('scopeMigrationReason'));
        if (migrationReason) throw Error(migrationReason);
        if (expectedRevision !== undefined && old !== expectedRevision) throw Error('Device library changed. Review the import again.');
        await operation(rows, meta);
        const count = await request(rows.count());
        if (count > MAX_RECORDS) throw Error('Device library record limit reached');
        meta.put(count, 'count'); meta.put(old + 1, 'revision');
        return old + 1;
      });
      this.channel?.postMessage({ revision });
      return revision;
    };
    return alreadyLocked ? run() : deviceWriteLock(run);
  }
  writeBatch(values: DeviceRecord[], remove: [Family, string][] = [], expectedRevision?: number) {
    const records = values.map(normalizeDevice);
    return this.mutate(async (rows) => { for (const key of remove) rows.delete(key); for (const row of records) rows.put(row); }, expectedRevision);
  }
  async remove(family: Family, key: string) {
    return this.mutate(async rows => {
      rows.delete([family, key]);
      if (family === 'lists') for (const row of await request(rows.getAll())) if (row.family === 'members' && row.list === key) rows.delete([row.family, row.key]);
      if (family === 'series') for (const row of await request(rows.getAll())) if (row.family === 'members' && row.series === key) rows.delete([row.family, row.key]);
    });
  }
  async import(snapshot: DeviceSnapshot, expectedRevision: number, mode: 'merge' | 'replace', admitCurrentCatalogue?: (scope: InstanceScope) => void) {
    const checked = validateSnapshot(snapshot);
    
    if (!['merge', 'replace'].includes(mode)) throw Error('Choose merge or replace');
    return this.mutate(async (rows, meta) => {
      const scope = await request(meta.get('instanceScope'));
      if (!sameScope(checked.scope, scope)) throw Error('Native IDs belong to a different instance or catalogue epoch. Restore this backup on its source instance.');
      admitCurrentCatalogue?.(scope);
      if (mode === 'replace') rows.clear();
      for (const row of checked.records) {
        const old = mode === 'merge' ? await request(rows.get([row.family, row.key])) : undefined;
        // Newer local records win. Equal-clock conflicts retain local state.
        if (!old || row.updated_at > old.updated_at) rows.put(row);
      }
    }, expectedRevision);
  }
  async pinServiceIdentity(serviceKey: string, catalogNamespace: string, expectedRevision: number) {
    if (!/^[0-9a-f-]{32,36}$/.test(catalogNamespace)) throw Error('Invalid catalogue namespace');
    if (!/^[0-9a-f]{64}$/.test(serviceKey)) throw Error('Invalid service public key');
    await this.mutate(async (_rows, meta) => {
      const scope = await request(meta.get('instanceScope'));
      if ((scope.serviceKey && scope.serviceKey !== serviceKey) || (scope.catalogNamespace && scope.catalogNamespace !== catalogNamespace)) throw Error('Instance identity or catalogue namespace changed; migration required');
      meta.put({ origin: scope.origin, serviceKey, catalogNamespace }, 'instanceScope');
    }, expectedRevision);
    this.scope = { origin: this.scope.origin, serviceKey, catalogNamespace };
  }
  newListId() { return hex(crypto.getRandomValues(new Uint8Array(16))); }
}
