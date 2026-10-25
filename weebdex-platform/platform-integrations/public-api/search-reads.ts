import type { PublicApiClient } from './client';
import { normalizeFeed, normalizeTag } from './normalize';
import { publicQuery } from './query';
import { PublicApiContractError } from './validation';
import type { Collection, Response, TagView, ViewFeed } from './types';

// Retained discovery emits the native default sort implicitly and uses a
// deliberate empty-selection sentinel. Translate both before public transport.
export function prepareMangaSearch(input: URLSearchParams, includes: readonly string[] = []) {
  const effective = new URLSearchParams(input);
  const selected = [...effective.getAll('contentRating'), ...effective.getAll('contentRating[]')];
  const emptySelection = selected.includes('__none__');
  if (emptySelection) {
    if (selected.length !== 1) throw new PublicApiContractError('An empty rating selection cannot include other ratings.', 422);
    effective.delete('contentRating'); effective.delete('contentRating[]');
  }
  if (effective.has('order') && !effective.has('sort') && ![...effective.keys()].some(key => key.startsWith('order['))) effective.set('sort', 'lastUploadedChapterAt');
  // Validate every remaining control even when no rows can be selected.
  return { query: publicQuery('manga', effective, includes), emptySelection };
}

function tagPage(input: URLSearchParams) {
  const allowed = new Set(['limit', 'page', 'offset']), seen = new Set<string>();
  for (const [key, value] of input) {
    if (!allowed.has(key) || seen.has(key) || !/^\d+$/.test(value) || !Number.isSafeInteger(Number(value))) throw new PublicApiContractError('Invalid retained tag pagination control: ' + key, 422);
    seen.add(key);
  }
  if (input.has('page') && input.has('offset')) throw new PublicApiContractError('Tag page and offset cannot be combined.', 422);
  const limit = Number(input.get('limit') ?? 100), page = Number(input.get('page') ?? 1);
  const offset = input.has('offset') ? Number(input.get('offset')) : (page - 1) * limit;
  if (limit < 0 || limit > 100 || page < 1 || offset < 0 || offset > 10000) throw new PublicApiContractError('Retained tag pagination exceeds its bounds.', 422);
  return { paginated: seen.size > 0, limit, offset };
}

export async function readRetainedTags(api: PublicApiClient, input: URLSearchParams, signal?: AbortSignal): Promise<Response<ViewFeed<TagView>>> {
  const pagination = tagPage(input);
  // The actual public endpoint accepts no controls and returns one complete
  // bounded snapshot. Apply retained pagination locally rather than dropping it
  // or sending unsupported page/limit controls to that endpoint.
  const response = await api.collection('/manga/tag', 'tag', publicQuery('tag', new URLSearchParams()), signal);
  if (response.status < 200 || response.status >= 300 || !response.body) return response as unknown as Response<ViewFeed<TagView>>;
  const body = response.body;
  if (body.offset !== 0 || body.data.length !== body.total || body.total > 4096) throw new PublicApiContractError('The public tag catalogue was not a complete bounded snapshot.');
  const selected: Collection = pagination.paginated
    ? { ...body, limit: pagination.limit, offset: pagination.offset, data: body.data.slice(pagination.offset, pagination.offset + pagination.limit) }
    : body;
  return { ...response, body: normalizeFeed(selected, normalizeTag) };
}
