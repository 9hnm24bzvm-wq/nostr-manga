// Pure reviewed graph preparation runs off the UI thread. Codec bytes and
// canonical LocalLibrary semantics are the original dependency, unchanged.
import { LocalLibrary } from './protocol/store.js';
import { buildGraph, normalize, recordKey, families, hash } from './protocol/codec.js';
import { request, transaction } from './protocol/indexeddb.js';
import type { DeviceRecord } from './device-store';
import type { SourceMapping } from './publication';
import { canonicalLibrary, checkedPublicDeviceScope } from '../public-api/device-id-state';
import { eligiblePublicMappings, mappingProofRows } from './public-device-projection';
import { graphTransferBuffers } from './graph-transfer';
import type { Family, LibraryRecord, Records } from './protocol/types.d.ts';
self.onmessage = async ({ data }) => {
  let protocol: LocalLibrary | undefined, device: IDBDatabase | undefined;
  try {
    const mappings: SourceMapping[] = data.mappings;
    const mappingProof = hash(new TextEncoder().encode(JSON.stringify(mappingProofRows(mappings))));
    if (data.fingerprintOnly) { self.postMessage({ mappingProof }); return; }
    protocol = await LocalLibrary.open(data.protocolDatabase);
    const open = indexedDB.open(data.deviceDatabase);
    open.onupgradeneeded = () => { open.transaction?.abort(); }; // Never create or upgrade a publication source.
    device = await new Promise<IDBDatabase>((resolve, reject) => { let blocked = false; open.onsuccess = () => { if (blocked) open.result.close(); else resolve(open.result); }; open.onerror = () => reject(open.error); open.onblocked = () => { blocked = true; reject(Error('Device database is blocked in graph worker')); }; });
    const snapshot = await transaction(device, ['records', 'meta'], 'readonly', async (tx: IDBTransaction) => ({ revision: await request(tx.objectStore('meta').get('revision')), records: await request(tx.objectStore('records').getAll()) as DeviceRecord[], scope: await request(tx.objectStore('meta').get('instanceScope')), marker: await request(tx.objectStore('meta').get('publicIdOwner')), owner: await request(tx.objectStore('meta').get('deviceOwnerId')), migrationReason: await request(tx.objectStore('meta').get('scopeMigrationReason')) }));
    if (snapshot.migrationReason) throw Error(String(snapshot.migrationReason));
    if (snapshot.revision !== data.deviceRevision || canonicalLibrary(snapshot.scope) !== canonicalLibrary(data.deviceScope)) throw Error('Device scope or revision changed during publication preparation');
    if (data.deviceVersion === 2) {
      if (!snapshot.marker || snapshot.marker.version !== 2 || snapshot.owner !== data.deviceOwner || snapshot.marker.ownerId !== data.deviceOwner || canonicalLibrary(checkedPublicDeviceScope(snapshot.marker.scope)) !== canonicalLibrary(checkedPublicDeviceScope(snapshot.scope))) throw Error('UUID device owner changed during publication preparation');
    } else if (snapshot.marker) throw Error('A legacy publication cannot project UUID owner-marked data');
    const canonical = await protocol.snapshot();
    const maps = new Map((data.deviceVersion === 2 ? eligiblePublicMappings(snapshot.records, mappings as import('../public-api/resource-mappings').VerifiedSourceMapping[]) : mappings).map(row => [row.kind + ':' + row.id, row]));
    const records: Records = { chapters: [], series: [], lists: [], members: [] };
    const byFamily: Record<Family, Map<string, LibraryRecord>> = { chapters: new Map(), series: new Map(), lists: new Map(), members: new Map() };
    for (const family of families) for (const row of canonical.records[family]) byFamily[family].set(recordKey(family, row), row);
    const excluded: string[] = [];
    for (const row of snapshot.records) {
      let record: LibraryRecord;
      if (row.family === 'lists') record = normalize('lists', row);
      else {
        const key = row.family === 'chapters' ? 'chapter:' + row.id : 'manga:' + (row.family === 'members' ? row.series : row.id), mapping = maps.get(key);
        if (!mapping) { excluded.push(row.family + ':' + row.key); continue; }
        if (row.family === 'series') record = normalize('series', { id: mapping.event_id, status: row.status, updated_at: row.updated_at });
        else if (row.family === 'chapters') record = normalize('chapters', { id: mapping.event_id, completed: row.completed, page: row.page, updated_at: row.updated_at });
        else record = normalize('members', { list: row.list, series: mapping.event_id, position: row.position, updated_at: row.updated_at });
      }
      const key = recordKey(row.family, record), previous = byFamily[row.family].get(key);
      if (!previous || record.updated_at > previous.updated_at) byFamily[row.family].set(key, record);
    }
    for (const family of families) records[family] = [...byFamily[family].values()];
    const graph = buildGraph(records);
    self.postMessage({ deviceRevision: snapshot.revision, protocolRevision: canonical.revision, graph, excluded, mappingProof }, graphTransferBuffers(graph));
  } catch (error) { self.postMessage({ error: error instanceof Error ? error.message : String(error) }); }
  finally { protocol?.close(); device?.close(); }
};
