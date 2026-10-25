import { publicHistoryBatch } from './history-catalogue';
import { PublicApiClient, type Transport } from './client';
import { publicQuery, chapterParentControls } from './query';
import { normalizeManga, normalizeChapter } from './normalize';
import { publicUuid } from './validation';
import { decimalKey, utcOrderKey } from './exact-order';
import { historyJson } from '../local-library/history-transport';
import type { HistoryBudget } from '../local-library/history-pagination';
import type { UpdateKey, UpdateRow } from '../local-library/updates-pagination';
export function updateOrder<T extends { id: string; chapter?: string | null; volume?: string | null; created_at?: string; updated_at?: string; published_at?: string }>(row: T, sort: string): T & UpdateRow {
  let value: unknown, text = ''; const decimal = sort === 'chapter' || sort === 'volume';
  if (decimal) { const source = row[sort as 'chapter' | 'volume']; value = typeof source === 'string' && /^\d+(?:\.\d+)?$/.test(source) ? decimalKey(source) : null; text = typeof source === 'string' ? source.toLowerCase() : ''; }
  else value = utcOrderKey(sort === 'createdAt' ? row.created_at : sort === 'updatedAt' ? row.updated_at : row.published_at);
  return { ...row, _sort_value: value, _sort_text: text, _sort_decimal: decimal } as T & UpdateRow;
}
// Each call reads exactly one parent and at most one chapter page, never the
// discovery composition that materializes an entire filtered catalogue.
export function publicUpdatesSource(input: URLSearchParams, transport?: Transport) {
  const q = new URLSearchParams(input), requested = q.get('sort') ?? 'publishedAt', sort = requested === 'readAt' ? 'publishedAt' : requested;
  if (!['publishedAt','publishAt','createdAt','updatedAt','chapter','volume'].includes(sort)) throw Error('Unsupported public update ordering');
  q.set('sort', sort === 'publishedAt' ? 'publishAt' : sort);
  for (const key of ['page','offset','limit']) q.delete(key);
  const ratings = [...q.getAll('contentRating'), ...q.getAll('contentRating[]')], empty = ratings.includes('__none__');
  if (empty) { if (ratings.length !== 1) throw Error('Invalid empty rating selection'); q.delete('contentRating'); q.delete('contentRating[]'); }
  const translated = publicQuery('chapterView', q), parentFilters = new URLSearchParams(), chapterFilters = new URLSearchParams();
  const parentOnly = new Set([...chapterParentControls, 'originalLanguage[]', 'excludedOriginalLanguage[]']);
  for (const [key,value] of translated) {
    if (['limit','offset'].includes(key)) continue;
    if (key === 'ids[]' || key === 'manga') throw Error('Update scope is owned by the saved device library');
    if (parentOnly.has(key) || key === 'contentRating[]') parentFilters.append(key,value);
    if (!parentOnly.has(key)) chapterFilters.append(key,value);
  }
  function client(budget: HistoryBudget) { return new PublicApiClient(transport ?? (request => historyJson('/api/v1' + request.path, budget))); }
  async function parent(id: string, budget: HistoryBudget) {
    const query = new URLSearchParams(parentFilters); query.set('ids[]', publicUuid(id)); query.set('limit','100'); query.set('offset','0');
    const response = await client(budget).collection('/manga','manga',publicQuery('manga',query,['cover_art','author','artist']),budget.signal); budget.check();
    if (response.status < 200 || response.status >= 300 || !response.body) throw Error('Update parent lookup failed (' + response.status + ')');
    const body = response.body;
    if (body.limit !== 100 || body.offset !== 0 || body.total > 1 || body.data.length !== body.total || body.data.some(row => row.id !== id)) throw Error('Update parent lookup returned an incomplete or unrequested resource');
    return body.data.length ? normalizeManga(body.data[0]) : undefined;
  }
  async function chapters(id: string, offset: number, limit: number, budget: HistoryBudget, keys?: readonly UpdateKey[]) {
    if (empty) return { total: 0, data: [] };
    if (offset > 10000) throw Error('This parent exceeds the public 10,000-offset query ceiling. No chapter was skipped; this scan is incomplete. Narrow chapter preferences and rebuild.');
    const manga = await parent(id,budget); if (!manga) return { total: 0, data: [] };
    const query = new URLSearchParams(chapterFilters); query.set('manga',publicUuid(id)); query.set('offset',String(offset)); query.set('limit',String(limit));
    if (keys) for (const key of keys) query.append('ids[]',publicUuid(key.id));
    const response = await client(budget).collection('/chapter','chapter',publicQuery('chapter',query,['manga','scanlation_group','user']),budget.signal); budget.check();
    if (response.status < 200 || response.status >= 300 || !response.body) throw Error('Update chapter lookup failed (' + response.status + ')');
    const body = response.body;
    if (body.limit !== limit || body.offset !== offset || body.data.length !== Math.min(limit,Math.max(0,body.total-offset)) || keys && body.total > keys.length) throw Error('Update chapter lookup returned an incomplete or unscoped page');
    const wanted = keys ? new Set(keys.map(key => key.id)) : undefined, seen = new Set<string>();
    const data = body.data.map(row => {
      const parents = row.relationships.filter(relation => relation.type === 'manga');
      if (parents.length !== 1 || parents[0].id !== id || seen.has(row.id) || wanted && !wanted.has(row.id)) throw Error('Update chapter identity or parent changed');
      seen.add(row.id); return updateOrder(normalizeChapter(row,manga),sort) as unknown as Chapter & UpdateRow;
    });
    return { total: body.total, data };
  }
  return {
    // The public server tie-breaks chapters with an unexposed native identity.
    // Only its primary comparator can be checked during traversal. The scratch
    // index separately preserves the retained client text/public-ID tie order.
    primaryOrderOnly: true,
    scan: (id: string, offset: number, limit: number, budget: HistoryBudget) => chapters(id,offset,limit,budget),
    async hydrate(keys: readonly UpdateKey[], budget: HistoryBudget): Promise<(Chapter & UpdateRow)[]> {
      const rows = await publicHistoryBatch(keys.map(key => ({ ...key, readAt: '1970-01-01T00:00:00Z' })), input, budget, transport);
      return rows.map(row => updateOrder(row,sort));
    }
  };
}
