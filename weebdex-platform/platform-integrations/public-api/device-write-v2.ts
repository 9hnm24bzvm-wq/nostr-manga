// Atomic anonymous-device UUID writes. This module has no signer or transport.
// Migration sidecars and unresolved legacy records remain intact. New writes
// require UUIDs; deletion history prevents older restores resurrecting rows.
import { request, transaction } from '../local-library/protocol/indexeddb.js';
import type { DeviceRecord, Family, InstanceScope } from '../local-library/device-store';
import { publicUuid } from './validation';
import { MIGRATION_STORES, MAX_MIGRATION_RECORDS, archiveRecord, canonicalLibrary, checkedPublicDeviceScope, normalizeMigrationRecord, normalizePublicDeviceRecord, safeInteger, sourceKey, type PublicIdOwner } from './device-id-state';
const same = (a: unknown, b: unknown) => canonicalLibrary(a) === canonicalLibrary(b);
function key(input: unknown): [Family, string] {
  if (!Array.isArray(input) || input.length !== 2 || !['series', 'chapters', 'lists', 'members'].includes(input[0]) || typeof input[1] !== 'string') throw Error('Choose one exact device-library key');
  const [family, id] = input as [Family, string];
  const resource = (id: string) => { if (!/^[A-Za-z0-9]{10}$/.test(id)) publicUuid(id); };
  if (family === 'lists') { if (!/^[0-9a-f]{32}$/.test(id)) throw Error('Invalid list key'); }
  else if (family === 'members') { if (!/^[0-9a-f]{32}:/.test(id)) throw Error('Invalid member key'); resource(id.slice(33)); }
  else resource(id);
  return [family, id];
}
export async function mutatePublicDevice(db: IDBDatabase, options: {
  scope: InstanceScope; expectedRevision?: number; assertCurrent(): void;
  operation(rows: IDBObjectStore, meta: IDBObjectStore): Promise<void>;
  adoptScope?: InstanceScope;
}): Promise<number> {
  if (!MIGRATION_STORES.every(name => db.objectStoreNames.contains(name))) throw Error('Upgrade UUID device-library storage first');
  options.assertCurrent();
  return transaction(db, [...MIGRATION_STORES], 'readwrite', async (tx: IDBTransaction) => {
    const rows = tx.objectStore('records'), meta = tx.objectStore('meta'), tombstones = tx.objectStore('idMigrationTombstones');
    const [scope, marker, owner, migrationReason, oldRevision] = await Promise.all([request(meta.get('instanceScope')), request(meta.get('publicIdOwner')), request(meta.get('deviceOwnerId')), request(meta.get('scopeMigrationReason')), request(meta.get('revision'))]);
    const bound = checkedPublicDeviceScope(scope), ownerId = publicUuid(owner), revision = safeInteger(oldRevision ?? 0);
    if (!marker || marker.version !== 2 || marker.ownerId !== ownerId || !same(marker.scope, bound) || !same(options.scope, bound)) throw Error('UUID device owner or catalogue scope changed. Reload before saving.');
    if (migrationReason) throw Error(String(migrationReason));
    if (options.expectedRevision !== undefined && options.expectedRevision !== revision) throw Error('Device library changed. Review the operation again.');
    const changes = new Map<string, { key: [Family, string]; before: Promise<DeviceRecord | undefined> }>();
    const deletedRoots = new Set<string>();
    const remember = (value: [Family, string]) => { const id = sourceKey({ family: value[0], key: value[1] }); if (!changes.has(id)) { const before = request(rows.get(value)) as Promise<DeviceRecord | undefined>; void before.catch(() => {}); changes.set(id, { key: value, before }); } };
    const guarded = new Proxy(rows, { get(target, property) {
      if (property === 'put' || property === 'add') return (input: unknown) => { const value = normalizePublicDeviceRecord(input); remember([value.family, value.key]); return target[property](value); };
      if (property === 'delete') return (input: unknown) => { const value = key(input); remember(value); if (value[0] === 'series' || value[0] === 'lists') deletedRoots.add(sourceKey({ family: value[0], key: value[1] })); return target.delete(value); };
      if (property === 'openCursor' || property === 'index' || property === 'openKeyCursor') return () => { throw Error('UUID writes require exact-key read/put/delete operations'); };
      if (property === 'clear') return () => { throw Error('Replacing a UUID library requires its reviewed full-backup restore'); };
      const value = Reflect.get(target, property, target); return typeof value === 'function' ? value.bind(target) : value;
    } });
    options.assertCurrent(); await options.operation(guarded, meta); options.assertCurrent();
    if (!same(await request(meta.get('publicIdOwner')), marker) || await request(meta.get('deviceOwnerId')) !== ownerId || !same(await request(meta.get('instanceScope')), bound)) throw Error('The operation attempted to replace UUID owner or scope metadata');
    // Cascade exact local memberships, including callers using the generic
    // transaction seam. Every cascaded removal gets its own archive/tombstone.
    const absent = new Set<string>();
    for (const id of deletedRoots) { const [family, value] = JSON.parse(id); if (!await request(rows.get([family, value]))) absent.add(id); }
    if (absent.size) for (const member of await request(rows.getAll()) as DeviceRecord[]) if (member.family === 'members' && (absent.has(sourceKey({ family: 'lists', key: member.list! })) || absent.has(sourceKey({ family: 'series', key: member.series! })))) { remember(['members', member.key]); rows.delete(['members', member.key]); }
    for (const [id, change] of changes) {
      const before = await change.before, current = await request(rows.get(change.key)) as DeviceRecord | undefined;
      const deleted = await request(tombstones.get(change.key)) as { updated_at: number } | undefined;
      if (current) {
        const value = normalizePublicDeviceRecord(current);
        if (before?.family === 'chapters' && before.mangaId !== value.mangaId) throw Error('Saved chapter parent changed; a reviewed catalogue migration is required');
        if (value.family === 'members' && !await request(rows.get(['lists', value.list!]))) throw Error('Choose an existing device list');
        value.updated_at = safeInteger(Math.max(value.updated_at, before ? safeInteger(before.updated_at) + 1 : 0, deleted ? safeInteger(deleted.updated_at) + 1 : 0));
        rows.put(value);
        const issues = tx.objectStore('idMigrationIssues'), issue = await request(issues.get(id));
        if (issue && before && issue.archiveDigest === archiveRecord(normalizeMigrationRecord(before), ownerId, bound).digest) issues.put({ ...issue, reason: 'merged' });
      } else if (before) {
        const archived = archiveRecord(normalizeMigrationRecord(before), ownerId, bound);
        const updated_at = safeInteger(Math.max(Math.floor(Date.now() / 1000), before.updated_at + 1, deleted ? deleted.updated_at + 1 : 0));
        tombstones.put({ family: change.key[0], key: change.key[1], updated_at });
        tx.objectStore('idMigrationArchives').put(archived);
        tx.objectStore('idMigrationIssues').put({ sourceKey: id, family: change.key[0], key: change.key[1], reason: 'deleted', archiveDigest: archived.digest });
      }
    }
    const count = await request(rows.count()); if (count > MAX_MIGRATION_RECORDS) throw Error('Device library record limit reached');
    if (options.adoptScope) {
      const next = checkedPublicDeviceScope(options.adoptScope);
      if (next.origin !== bound.origin || next.catalogNamespace !== bound.catalogNamespace || bound.serviceKey && bound.serviceKey !== next.serviceKey || !next.serviceKey) throw Error('Known UUID catalogue/service scope cannot be rebound');
      meta.put(next, 'instanceScope'); meta.put({ version: 2, ownerId, scope: next } satisfies PublicIdOwner, 'publicIdOwner');
    }
    meta.put(count, 'count'); meta.put(safeInteger(revision + 1), 'revision'); options.assertCurrent(); return revision + 1;
  });
}
