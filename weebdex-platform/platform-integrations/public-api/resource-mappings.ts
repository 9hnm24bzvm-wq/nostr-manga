// Read-only, versioned public UUID-to-original provenance. Device-only saves
// never call this helper; only explicit source actions/publication use it.
import { publicUuid, record } from './validation';
export type SourceReference = { kind: 'manga' | 'chapter'; id: string };
export type VerifiedSourceMapping = SourceReference & { canonical_id: string; event_id: string; publisher: string; source_kind: 1001 | 1002; compatibility_id: string; parent: { id: string; event_id: string } | null };
export type SourceMappingScope = { publicOrigin: string; sourceOrigin: string; serviceKey: string; catalogNamespace: string };
const key = (row: SourceReference) => row.kind + ':' + row.id;
const hash = (value: unknown): string => { if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value)) throw Error('Invalid verified original identity'); return value; };
function canonical(value: string): string { const url = new URL(value); if (url.origin !== value || !['http:', 'https:'].includes(url.protocol)) throw Error('Invalid mapping origin'); return value; }
export function createPublicResourceMapper(deps: { fetch: typeof fetch } = { fetch: (...args) => fetch(...args) }) {
  return async (scope: SourceMappingScope, input: readonly SourceReference[], assertCurrent = () => {}): Promise<{ mappings: VerifiedSourceMapping[]; unresolved: SourceReference[] }> => {
    assertCurrent(); scope = { publicOrigin: canonical(scope.publicOrigin), sourceOrigin: canonical(scope.sourceOrigin), serviceKey: hash(scope.serviceKey), catalogNamespace: publicUuid(scope.catalogNamespace) };
    if (input.length > 256) throw Error('Map at most256 public resources');
    const references = input.map(row => { if (!['manga', 'chapter'].includes(row.kind)) throw Error('Invalid source reference kind'); return { kind: row.kind, id: publicUuid(row.id) }; });
    const wanted = new Set(references.map(key)); if (wanted.size !== references.length) throw Error('Duplicate source reference');
    if (!references.length) return { mappings: [], unresolved: [] };
    const response = await deps.fetch(scope.publicOrigin + '/api/v1/nostr/resources', { method: 'POST', credentials: 'omit', redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(30000), headers: { 'content-type': 'application/json' }, body: JSON.stringify({ schemaVersion: 2, catalogNamespace: scope.catalogNamespace, resources: references }) });
    assertCurrent(); if (!response.ok) throw Error(`Source mapping refused (HTTP ${response.status})`);
    const mime = response.headers.get('content-type')?.split(';', 1)[0].trim().toLowerCase(); if (mime !== 'application/json') throw Error('Invalid source mapping content type');
    const reader = response.body?.getReader(); if (!reader) throw Error('Source mapping response unavailable'); const chunks: Uint8Array[] = []; let size = 0;
    try { for (;;) { const next = await reader.read(); assertCurrent(); if (next.done) break; if (!(next.value instanceof Uint8Array) || next.value.length > 512 * 1024 - size) throw Error('Source mapping exceeds response budget'); chunks.push(next.value); size += next.value.length; } }
    catch (error) { await reader.cancel().catch(() => {}); throw error; } finally { reader.releaseLock(); }
    const bytes = new Uint8Array(size); let offset = 0; for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    const result = record(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)));
    if (result.schemaVersion !== 2 || result.idFormat !== 'public_uuid' || result.origin !== scope.sourceOrigin || result.service_key !== scope.serviceKey || result.catalog_namespace !== scope.catalogNamespace || !Array.isArray(result.data) || !Array.isArray(result.unresolved)) throw Error('Mapping response changed its verified service/catalogue scope');
    const seen = new Set<string>();
    function reference(input: unknown): SourceReference { const row = record(input); if (!['manga', 'chapter'].includes(String(row.kind))) throw Error('Mapping changed resource kind'); const ref = { kind: row.kind as SourceReference['kind'], id: publicUuid(row.id) }; const id = key(ref); if (!wanted.has(id) || seen.has(id)) throw Error('Mapping returned an unrequested or duplicate resource'); seen.add(id); return ref; }
    const mappings = result.data.map(input => { const row = record(input), ref = reference(row); if (row.source_kind !== (ref.kind === 'manga' ? 1002 : 1001)) throw Error('Mapping changed original resource kind'); const parent = row.parent === null ? null : record(row.parent); if ((ref.kind === 'chapter') !== !!parent) throw Error('Mapping omitted its exact original parent'); return { ...ref, canonical_id: publicUuid(row.canonical_id), event_id: hash(row.event_id), publisher: hash(row.publisher), compatibility_id: publicUuid(row.compatibility_id), source_kind: row.source_kind as 1001 | 1002, parent: parent ? { id: publicUuid(parent.id), event_id: hash(parent.event_id) } : null }; });
    const unresolved = result.unresolved.map(input => { const row = record(input); if (!['no-visible-original-source', 'no-visible-original-parent'].includes(String(row.reason))) throw Error('Invalid unresolved source disposition'); return reference(row); });
    if (seen.size !== wanted.size) throw Error('Mapping omitted requested resources');
    assertCurrent(); return { mappings, unresolved };
  };
}
export const publicResourceMappings = createPublicResourceMapper();
