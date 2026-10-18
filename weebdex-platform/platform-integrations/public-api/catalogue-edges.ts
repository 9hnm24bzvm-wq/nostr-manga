// Read persisted public edges, then hydrate only their actual visible targets.
// Related/recommended UUIDs are never replaced by synthetic/native IDs.
import type { PublicApiClient } from './client';
import { normalizeManga } from './normalize';
import { publicUuid, PublicApiContractError, record } from './validation';
import type { Collection, MangaView, PublicUuid, Resource, Response, ViewFeed } from './types';

type EdgeKind = 'relation' | 'recommendation';
export type EdgeManga = MangaView & { type?: string };
const ratings = ['safe', 'suggestive', 'erotica', 'pornographic'];
const includes = ['cover_art', 'author', 'artist'];
const MAX_RELATIONS = 4096, RELATION_PAGE = 32, MAX_RELATION_BYTES = 16 * 1024 * 1024;

function relationReadBudget(caller?: AbortSignal) {
  const controller = new AbortController(), abort = () => controller.abort();
  if (caller?.aborted) abort(); else caller?.addEventListener('abort', abort, { once: true });
  const timer = setTimeout(abort, 60_000);
  let bytes = 0;
  return {
    signal: controller.signal,
    check() { if (controller.signal.aborted) throw new PublicApiContractError(caller?.aborted ? 'Related title read cancelled.' : 'Related title read exceeded its time budget. No partial list was returned.', caller?.aborted ? 499 : 503); },
    consume(body: unknown, page = false) {
      const size = new TextEncoder().encode(JSON.stringify(body)).byteLength;
      if (page && size > 1024 * 1024 || size > MAX_RELATION_BYTES - bytes) throw new PublicApiContractError('Related title read exceeded its bounded response budget. No partial list was returned.', 503);
      bytes += size;
    },
    close() { clearTimeout(timer); caller?.removeEventListener('abort', abort); }
  };
}
type RelationBudget = ReturnType<typeof relationReadBudget>;

// Optional instance extension: follow whole-resource prefixes, never guessed
// page sizes. The original requested UUID remains bound by the typed proof.
async function relationCollection(api: PublicApiClient, requested: PublicUuid, input: URLSearchParams, budget: RelationBudget) {
  const query = edgeQuery('relation', input), data: Resource[] = [], edgeIds = new Set<string>(), targets = new Map<string, number>();
  let offset = 0, owner = requested, version = 0, snapshot = '', total = 0, first: Response<Collection> | undefined;
  for (let page = 0; page < MAX_RELATIONS; page++) {
    budget.check(); const next = new URLSearchParams(query); next.set('limit', String(RELATION_PAGE)); next.set('offset', String(offset));
    if (page) { next.set('ownerVersion', String(version)); next.set('snapshot', snapshot); }
    const response = await api.collection(`/manga/${requested}/relation`, 'manga_relation', next, budget.signal); budget.check();
    if (response.status < 200 || response.status >= 300 || !response.body) {
      if (!page) return { owner: requested, targets, response };
      throw new PublicApiContractError('Related title pages changed or could not be loaded. Refresh the view; no partial list was returned.', response.status);
    }
    budget.consume(response.body, true);
    const body = response.body, meta = record(record(body).nostr, 'relation page provenance'), proof = record(meta.identity, 'relation owner identity');
    const canonical = publicUuid(record(body).ownerMangaId), ownerVersion = record(body).ownerVersion, token = record(body).snapshot;
    if (canonical !== record(body).ownerMangaId || meta.scope !== 'instance-local' || meta.versionScope !== 'owner_manga' || meta.pagination !== 'bounded-relations-v1' || proof.kind !== 'manga' || proof.requestedId !== requested || proof.canonicalId !== canonical || proof.source !== 'persisted-public-resource-ids' || !Number.isSafeInteger(ownerVersion) || Number(ownerVersion) < 1 || Number(ownerVersion) > 2147483647 || typeof token !== 'string' || !/^[0-9a-f]{64}$/.test(token) || Object.hasOwn(body, 'persistedTotal')) throw new PublicApiContractError('Invalid bounded relation owner or snapshot proof');
    if (body.limit !== RELATION_PAGE || body.offset !== offset || body.total > MAX_RELATIONS || body.data.length > RELATION_PAGE || offset + body.data.length > body.total || !body.data.length && offset < body.total) throw new PublicApiContractError('Invalid bounded relation page progress');
    if (page && (canonical !== owner || ownerVersion !== version || token !== snapshot || body.total !== total)) throw new PublicApiContractError('Related title snapshot changed. Refresh the view; no partial list was returned.', 409);
    if (!page) { first = response; owner = canonical; version = Number(ownerVersion); snapshot = token; total = body.total; }
    for (const row of body.data) {
      const edgeId = publicUuid(row.id), relation = row.attributes.relation, extension = record(row.attributes.nostr, 'relation metadata');
      if (row.id !== edgeId || edgeIds.has(edgeId) || row.relationships.length !== 1 || row.relationships[0].type !== 'manga' || row.attributes.version !== version || typeof relation !== 'string' || !/^[a-z_]{1,40}$/.test(relation) || extension.scope !== 'instance-local' || extension.versionScope !== 'owner_manga') throw new PublicApiContractError('Invalid persisted relation page metadata');
      const target = publicUuid(row.relationships[0].id), attributes = record(row.relationships[0].attributes, 'snapshot target attributes'), targetVersion = attributes.version;
      if (row.relationships[0].id !== target || target === owner || targets.has(target) || !Number.isSafeInteger(targetVersion) || Number(targetVersion) < 1 || Number(targetVersion) > 2147483647) throw new PublicApiContractError('Invalid or duplicate relation snapshot target');
      edgeIds.add(edgeId); targets.set(target, Number(targetVersion)); data.push(row);
    }
    const end = offset + body.data.length, expectedNext = end < total ? end : null;
    if (record(body).nextOffset !== expectedNext) throw new PublicApiContractError('Related title continuation skipped, repeated or truncated a page');
    if (expectedNext === null) return { owner, targets, response: { ...first!, body: { result: 'ok' as const, response: 'collection' as const, data, limit: total, offset: 0, total } } };
    offset = expectedNext;
  }
  throw new PublicApiContractError('Related title view exceeds its bounded page budget.', 503);
}

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

async function hydrate(api: PublicApiClient, ids: string[], signal?: AbortSignal, versions?: ReadonlyMap<string, number>, budget?: RelationBudget): Promise<Map<string, MangaView>> {
  const result = new Map<string, MangaView>();
  const batchSize = versions ? RELATION_PAGE : 100;
  for (let start = 0; start < ids.length; start += batchSize) {
    budget?.check(); const batch = ids.slice(start, start + batchSize), requested = new Set(batch);
    const query = new URLSearchParams({ limit: String(batchSize) });
    for (const id of batch) query.append('ids[]', id);
    for (const rating of ratings) query.append('contentRating[]', rating);
    for (const include of includes) query.append('includes[]', include);
    const response = await api.collection('/manga', 'manga', query, signal);
    budget?.check();
    if (response.status < 200 || response.status >= 300 || !response.body) throw new PublicApiContractError('Related title lookup failed', response.status);
    if (response.body.offset !== 0 || response.body.total !== batch.length || response.body.data.length !== batch.length || versions && response.body.limit !== batchSize) throw new PublicApiContractError('Related titles changed while loading. Retry this view.', 409);
    budget?.consume(response.body);
    for (const row of response.body.data) {
      if (versions && row.attributes.version !== versions.get(publicUuid(row.id))) throw new PublicApiContractError('Related title metadata changed after the captured snapshot. Refresh this view.', 409);
      const manga = normalizeManga(row);
      if (!requested.has(manga.id) || result.has(manga.id)) throw new PublicApiContractError('Related title lookup escaped the exact requested UUIDs');
      result.set(manga.id, manga);
    }
  }
  return result;
}

export async function catalogueEdges(api: PublicApiClient, kind: EdgeKind, ownerId: string, query = new URLSearchParams(), signal?: AbortSignal): Promise<Response<ViewFeed<EdgeManga>>> {
  const requested = publicUuid(ownerId), budget = kind === 'relation' ? relationReadBudget(signal) : undefined;
  try {
  const relation = budget ? await relationCollection(api, requested, query, budget) : undefined, owner = relation?.owner ?? requested;
  const response = relation?.response ?? await api.collection(`/manga/${owner}/${kind}`, kind === 'relation' ? 'manga_relation' : 'manga_recommendation', edgeQuery(kind, query), signal);
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
  const titles = await hydrate(api, [...targets], budget?.signal ?? signal, relation?.targets, budget);
  const data: EdgeManga[] = edges.map(edge => {
    const manga = titles.get(edge.target);
    if (!manga) throw new PublicApiContractError('A related title is no longer visible. Retry this view.', 409);
    return { ...manga, ...(edge.relation ? { type: edge.relation } : {}), public_api: { ...manga.public_api, edge: { owner, resource: edge.row } } };
  });
  return { ...response, body: { total: wire.total, limit: wire.limit, offset: wire.offset, page: 1, data } };
  } catch (error) { budget?.check(); throw error; }
  finally { budget?.close(); }
}
