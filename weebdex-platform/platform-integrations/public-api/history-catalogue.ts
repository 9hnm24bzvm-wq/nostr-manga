import { PublicApiClient, type Transport } from './client';
import { publicQuery, chapterParentControls } from './query';
import { normalizeManga, normalizeChapter } from './normalize';
import { publicUuid } from './validation';
import type { Resource } from './types';
import { type HistoryKey, type HistoryBudget } from '../local-library/history-pagination';
import { historyJson } from '../local-library/history-transport';
// Unlike global chapter composition, every title and chapter query is bounded
// by this local batch. Title preferences are applied before selecting rows.
export async function publicHistoryBatch(keys: readonly HistoryKey[], input: URLSearchParams, budget: HistoryBudget, transport?: Transport): Promise<Chapter[]> {
  if (!keys.length) return [];
  if (keys.length > 100) throw Error('History hydration requires at most 100 identities');
  const q = new URLSearchParams(input); for (const key of ['page', 'limit', 'sort', 'order']) q.delete(key);
  const empty = q.getAll('contentRating').includes('__none__');
  if (empty) q.delete('contentRating');
  const translated = publicQuery('chapterView', q), parents = new URLSearchParams(), chapters = new URLSearchParams();
  const parentOnly = new Set([...chapterParentControls, 'originalLanguage[]', 'excludedOriginalLanguage[]']);
  for (const [key, value] of translated) {
    if (['limit', 'offset'].includes(key)) continue;
    if (key === 'ids[]' || key === 'manga' || key.startsWith('order[')) throw Error('History scope and ordering are owned by the local snapshot');
    if (parentOnly.has(key) || key === 'contentRating[]') parents.append(key, value);
    if (!parentOnly.has(key)) chapters.append(key, value);
  }
  if (empty) return [];
  const api = new PublicApiClient(transport ?? (request => historyJson('/api/v1' + request.path, budget)));
  async function collection(path: '/manga' | '/chapter', ids: string[], query: URLSearchParams): Promise<Resource[]> {
    if (!ids.length) return [];
    const wanted = new Set(ids.map(publicUuid)), rows: Resource[] = [], seen = new Set<string>(); let expected: number | undefined;
    const base = new URLSearchParams(query); for (const id of wanted) base.append('ids[]', id);
    for (;;) {
      budget.check(); const page = new URLSearchParams(base); page.set('limit', '100'); page.set('offset', String(rows.length));
      const response = await api.collection(path, path === '/manga' ? 'manga' : 'chapter', publicQuery(path === '/manga' ? 'manga' : 'chapter', page, path === '/manga' ? ['cover_art', 'author', 'artist'] : ['manga', 'scanlation_group', 'user']), budget.signal);
      budget.check(); if (response.status < 200 || response.status >= 300 || !response.body) throw Error('History catalogue lookup failed (' + response.status + '). Retry without skipping this batch.');
      const body = response.body;
      if (body.total > wanted.size || body.limit !== 100 || body.offset !== rows.length || expected !== undefined && expected !== body.total) throw Error('History catalogue scope or total changed; reload the view');
      expected = body.total;
      for (const row of body.data) { if (!wanted.has(row.id) || seen.has(row.id)) throw Error('History catalogue returned an unrequested or duplicate identity'); seen.add(row.id); rows.push(row); }
      if (rows.length === expected) return rows;
      if (!body.data.length) throw Error('History catalogue returned an incomplete page');
    }
  }
  const parentRows = await collection('/manga', [...new Set(keys.map(row => row.mangaId))], parents);
  const parentMap = new Map<string, ReturnType<typeof normalizeManga>>(parentRows.map(row => [row.id, normalizeManga(row)]));
  const selected = keys.filter(row => parentMap.has(row.mangaId)), keyMap = new Map(selected.map(row => [row.id, row]));
  const rows = await collection('/chapter', selected.map(row => row.id), chapters);
  return rows.map(row => { const key = keyMap.get(row.id)!, parent = parentMap.get(key.mangaId)!; const relations = row.relationships.filter(relation => relation.type === 'manga'); if (relations.length !== 1 || relations[0].id !== key.mangaId) throw Error('History chapter parent changed; reload the view'); return normalizeChapter(row, parent) as unknown as Chapter; });
}
