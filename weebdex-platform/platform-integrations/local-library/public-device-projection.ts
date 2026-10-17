// Only explicit reviewed publication projects catalogue UUIDs into actual
// original event IDs. Local-only and unresolved legacy rows remain local.
import type { DeviceRecord } from './device-store';
import type { SourceReference, VerifiedSourceMapping } from '../public-api/resource-mappings';
import { publicUuid, uuidPattern } from '../public-api/validation';
export function publicSourceReferences(records: readonly DeviceRecord[]): SourceReference[] {
  const refs = new Map<string, SourceReference>();
  const add = (kind: SourceReference['kind'], input: string | undefined) => { if (input && uuidPattern.test(input)) { const id = publicUuid(input); refs.set(kind + ':' + id, { kind, id }); } };
  for (const row of records) {
    if (row.family === 'series') add('manga', row.id);
    if (row.family === 'members') add('manga', row.series);
    if (row.family === 'chapters') { add('chapter', row.id); add('manga', row.mangaId); }
  }
  return [...refs.values()];
}
export function eligiblePublicMappings(records: readonly DeviceRecord[], input: readonly VerifiedSourceMapping[]): VerifiedSourceMapping[] {
  const requested = new Set(publicSourceReferences(records).map(row => row.kind + ':' + row.id));
  const maps = new Map<string, VerifiedSourceMapping>();
  for (const row of input) { const key = row.kind + ':' + row.id; if (!requested.has(key) || maps.has(key)) throw Error('Unexpected or duplicate public source mapping'); maps.set(key, row); }
  const parents = new Map(records.filter(row => row.family === 'chapters').map(row => [row.id!, row.mangaId!]));
  return [...maps.values()].filter(row => {
    if (row.kind === 'manga') return true;
    const parent = maps.get('manga:' + parents.get(row.id));
    return !!parent && !!row.parent && parent.event_id === row.parent.event_id && parent.canonical_id === row.parent.id;
  });
}
export function mappingProofRows(input: readonly { kind: string; id: string; event_id: string; canonical_id?: string; publisher?: string; source_kind?: number; compatibility_id: string; parent?: { id: string; event_id: string } | null }[]) {
  return input.map(row => [row.kind, row.id, row.event_id, row.canonical_id ?? null, row.publisher ?? null, row.source_kind ?? null, row.compatibility_id, row.parent?.id ?? null, row.parent?.event_id ?? null]).sort((a, b) => String(a[0] + ':' + a[1]).localeCompare(String(b[0] + ':' + b[1])));
}
