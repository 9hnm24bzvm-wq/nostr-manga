// Read persisted public edges, then hydrate only their actual visible targets.
// Related/recommended UUIDs are never replaced by synthetic/native IDs.
import type { PublicApiClient } from './client';
import { normalizeManga } from './normalize';
import { publicUuid, PublicApiContractError, record } from './validation';
import type { MangaView, Resource, Response, ViewFeed } from './types';

type EdgeKind = 'relation' | 'recommendation';
export type EdgeManga = MangaView & { type?: string };
const ratings = ['safe', 'suggestive', 'erotica', 'pornographic'];
const includes = ['cover_art', 'author', 'artist'];

function edgeQuery(kind: EdgeKind, input: URLSearchParams): URLSearchParams {
  const result = new URLSearchParams([['includes[]', 'manga']]);
  for (const [key, value] of input) {
    if (key === 'includes[]' && value === 'manga') continue;
    if (kind === 'recommendation' && key === 'contentRating[]' && ratings.includes(value)) result.append(key, value);
    else if (kind === 'recommendation' && key === 'order[score]' && ['asc', 'desc'].includes(value) && !result.has(key)) result.set(key, value);
    else throw new PublicApiContractError('Unsupported public ' + kind + ' control', 422);
  }
  return result;
}

async function hydrate(api: PublicApiClient, ids: string[], signal?: AbortSignal): Promise<Map<string, MangaView>> {
  const result = new Map<string, MangaView>();
  for (let start = 0; start < ids.length; start += 100) {
    const batch = ids.slice(start, start + 100), requested = new Set(batch);
    const query = new URLSearchParams({ limit: '100' });
    for (const id of batch) query.append('ids[]', id);
    for (const rating of ratings) query.append('contentRating[]', rating);
    for (const include of includes) query.append('includes[]', include);
    const response = await api.collection('/manga', 'manga', query, signal);
    if (response.status < 200 || response.status >= 300 || !response.body) throw new PublicApiContractError('Related title lookup failed', response.status);
    if (response.body.offset !== 0 || response.body.total !== batch.length || response.body.data.length !== batch.length) throw new PublicApiContractError('Related titles changed while loading. Retry this view.', 409);
    for (const row of response.body.data) {
      const manga = normalizeManga(row);
      if (!requested.has(manga.id) || result.has(manga.id)) throw new PublicApiContractError('Related title lookup escaped the exact requested UUIDs');
      result.set(manga.id, manga);
    }
  }
  return result;
}

export async function catalogueEdges(api: PublicApiClient, kind: EdgeKind, ownerId: string, query = new URLSearchParams(), signal?: AbortSignal): Promise<Response<ViewFeed<EdgeManga>>> {
  const owner = publicUuid(ownerId);
  const response = await api.collection(`/manga/${owner}/${kind}`, kind === 'relation' ? 'manga_relation' : 'manga_recommendation', edgeQuery(kind, query), signal);
  if (response.status < 200 || response.status >= 300 || !response.body) return response as unknown as Response<ViewFeed<EdgeManga>>;
  const wire = response.body;
  if (wire.offset !== 0 || wire.total < wire.data.length || wire.data.length > wire.limit || (kind === 'recommendation' && wire.limit !== 20) || wire.data.length > (kind === 'relation' ? 4096 : 20) || (kind === 'relation' && wire.total !== wire.data.length)) throw new PublicApiContractError('Invalid bounded public ' + kind + ' collection');
  const edgeIds = new Set<string>(), targets = new Set<string>();
  const edges: { row: Resource; target: string; relation?: string; score?: number }[] = [];
  for (const row of wire.data) {
    const edgeId = publicUuid(row.id);
    if (edgeIds.has(edgeId) || row.relationships.length !== 1 || row.relationships[0].type !== 'manga') throw new PublicApiContractError('Invalid or duplicate public edge');
    const target = publicUuid(row.relationships[0].id);
    if (target === owner || targets.has(target)) throw new PublicApiContractError('Public edge has a duplicate or self target');
    edgeIds.add(edgeId); targets.add(target);
    if (kind === 'recommendation') {
      const score = row.attributes.score;
      if (!Number.isSafeInteger(score)) throw new PublicApiContractError('Public recommendation score is not an exact integer');
      edges.push({ row, target, score: score as number });
    } else {
      const relation = row.attributes.relation, version = row.attributes.version, extension = record(row.attributes.nostr, 'relation provenance');
      if (typeof relation !== 'string' || !/^[a-z_]{1,40}$/.test(relation) || !Number.isSafeInteger(version) || Number(version) < 1 || extension.scope !== 'instance-local' || extension.versionScope !== 'owner_manga') throw new PublicApiContractError('Invalid persisted relation metadata');
      edges.push({ row, target, relation });
    }
  }
  const titles = await hydrate(api, [...targets], signal);
  const data: EdgeManga[] = edges.map(edge => {
    const manga = titles.get(edge.target);
    if (!manga) throw new PublicApiContractError('A related title is no longer visible. Retry this view.', 409);
    return { ...manga, ...(edge.relation ? { type: edge.relation } : {}), public_api: { ...manga.public_api, edge: { owner, resource: edge.row } } };
  });
  return { ...response, body: { total: wire.total, limit: wire.limit, offset: wire.offset, page: 1, data } };
}
