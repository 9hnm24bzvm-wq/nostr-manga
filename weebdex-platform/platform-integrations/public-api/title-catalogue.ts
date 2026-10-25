import { PublicApiClient, type Transport } from './client';
import { prepareMangaSearch } from './search-reads';
import { normalizeFeed, normalizeManga } from './normalize';
import { publicLibraryCandidates } from './library-catalogue';
import { type HistoryBudget } from '../local-library/history-pagination';
import { historyJson } from '../local-library/history-transport';
// Same reviewed owned byte/time/cleanup transport as device history. No change
// to history behavior, and never the unbounded general catalogue transport.
export async function publicTitleBatch(ids: string[], input: URLSearchParams, budget: HistoryBudget, transport?: Transport): Promise<Manga[]> {
  if (!ids.length) return [];
  if (ids.length > 100) throw Error('Title hydration requires at most 100 identities');
  const api = new PublicApiClient(transport ?? (request => historyJson('/api/v1' + request.path, budget)));
  return publicLibraryCandidates<Manga>('/manga', 'id', ids, input, async (_operation, options) => {
    budget.check();
    const prepared = prepareMangaSearch(options.query ?? new URLSearchParams(), ['cover_art', 'author', 'artist']);
    if (prepared.emptySelection) return { status: 200, body: { total: 0, limit: 100, offset: 0, data: [] } };
    const response = await api.collection('/manga', 'manga', prepared.query, budget.signal); budget.check();
    return response.status < 200 || response.status >= 300 || !response.body ? response : { ...response, body: normalizeFeed(response.body, normalizeManga) };
  });
}
