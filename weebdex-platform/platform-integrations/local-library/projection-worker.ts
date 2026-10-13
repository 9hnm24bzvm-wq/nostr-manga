// Pure reviewed graph preparation runs off the UI thread. Codec bytes and
// canonical LocalLibrary semantics are the original dependency, unchanged.
import { LocalLibrary } from './protocol/store.js';
import { buildGraph, normalize, recordKey, families, hash } from './protocol/codec.js';
import { request, transaction } from './protocol/indexeddb.js';
import type { DeviceRecord } from './device-store';
import type { SourceMapping } from './publication';
self.onmessage = async ({ data }) => {
  let protocol: LocalLibrary | undefined, device: IDBDatabase | undefined;
  try {
    const mappings: SourceMapping[] = data.mappings;
    const mappingProof = hash(new TextEncoder().encode(JSON.stringify(mappings.map(row => [row.kind, row.id, row.event_id]).sort((a, b) => (a[0] + ':' + a[1]).localeCompare(b[0] + ':' + b[1])))));
    if (data.fingerprintOnly) { self.postMessage({ mappingProof }); return; }
    protocol = await LocalLibrary.open(data.protocolDatabase);
    const open = indexedDB.open(data.deviceDatabase, 1);
    device = await new Promise<IDBDatabase>((resolve, reject) => { open.onsuccess = () => resolve(open.result); open.onerror = () => reject(open.error); open.onblocked = () => reject(Error('Device database is blocked in graph worker')); });
    const snapshot = await transaction(device, ['records', 'meta'], 'readonly', async (tx: IDBTransaction) => ({ revision: await request(tx.objectStore('meta').get('revision')), records: await request(tx.objectStore('records').getAll()) as DeviceRecord[] }));
    const canonical = await protocol.snapshot();
    const maps = new Map(mappings.map(row => [row.kind + ':' + row.id, row]));
    const records: Record<string, unknown[]> = Object.fromEntries(families.map((family: string) => [family, []]));
    const byFamily = Object.fromEntries(families.map((family: string) => [family, new Map(canonical.records[family].map((row: unknown) => [recordKey(family, row), row]))]));
    const excluded: string[] = [];
    for (const row of snapshot.records) {
      let record: unknown;
      if (row.family === 'lists') record = normalize('lists', row);
      else {
        const key = row.family === 'chapters' ? 'chapter:' + row.id : 'manga:' + (row.family === 'members' ? row.series : row.id), mapping = maps.get(key);
        if (!mapping) { excluded.push(row.family + ':' + row.key); continue; }
        if (row.family === 'series') record = normalize('series', { id: mapping.event_id, status: row.status, updated_at: row.updated_at });
        if (row.family === 'chapters') record = normalize('chapters', { id: mapping.event_id, completed: row.completed, page: row.page, updated_at: row.updated_at });
        if (row.family === 'members') record = normalize('members', { list: row.list, series: mapping.event_id, position: row.position, updated_at: row.updated_at });
      }
      const key = recordKey(row.family, record), previous = byFamily[row.family].get(key);
      if (!previous || (record as { updated_at: number }).updated_at > (previous as { updated_at: number }).updated_at) byFamily[row.family].set(key, record);
    }
    for (const family of families) records[family] = [...byFamily[family].values()];
    const graph = buildGraph(records);
    self.postMessage({ deviceRevision: snapshot.revision, protocolRevision: canonical.revision, graph, excluded, mappingProof }, graph.objects.map((object: { data: Uint8Array }) => object.data.buffer));
  } catch (error) { self.postMessage({ error: error instanceof Error ? error.message : String(error) }); }
  finally { protocol?.close(); device?.close(); }
};
