import { apiRead, params, assertCatalogueScope } from './library';
// Retrieve every filtered candidate before a single global sort/page. Each request
// has <=100 IDs and <=100 results. Empty sets never become a global catalogue query.
export async function filteredCatalog<T extends { id: string }>(path: string, key: string, ids: string[], query: URLSearchParams): Promise<T[]> {
  if (!ids.length) return [];
  await assertCatalogueScope();
  return scopedCatalogCandidates<T>(path, key, ids, query);
}
// Used only by an explicit adoption preview: results are prospective identities,
// never silently applied to existing native records. Caller verifies live epoch.
export async function scopedCatalogCandidates<T extends { id: string }>(path: string, key: string, ids: string[], query: URLSearchParams): Promise<T[]> {
  if (!ids.length) return [];
  const base = params(query); base.delete('page'); base.delete('limit'); base.delete(key);
  const unique = [...new Set(ids)], results = new Map<string, T>();
  for (let start = 0; start < unique.length; start += 100) {
    const q = params(base);
    for (const id of unique.slice(start, start + 100)) q.append(key, id);
    q.set('limit', '100');
    for (let page = 1;; page++) {
      q.set('page', String(page));
      const result = await apiRead<{ total: number; data?: T[] }>(path, q);
      if (!Number.isSafeInteger(result.total) || result.total < 0 || !Array.isArray(result.data)) throw Error('Invalid catalogue page');
      for (const row of result.data) results.set(row.id, row);
      if (page * 100 >= result.total) break;
      if (!result.data.length) throw Error('Catalogue returned an incomplete page; retry the search');
    }
  }
  return [...results.values()];
}
export function pageRows<T>(rows: T[], query: URLSearchParams) {
  const q = params(query), requested = Number(q.get('limit') ?? 20), requestedPage = Number(q.get('page') ?? 1);
  const limit = Number.isSafeInteger(requested) && requested >= 0 ? Math.min(100, requested) : 20;
  const page = Number.isSafeInteger(requestedPage) && requestedPage > 0 ? requestedPage : 1;
  return { total: rows.length, limit, page, data: limit ? rows.slice((page - 1) * limit, page * limit) : [] };
}
export function compareValue(a: unknown, b: unknown) {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  const left = String(a ?? ''), right = String(b ?? '');
  return left < right ? -1 : left > right ? 1 : 0;
}

export function compareCatalogueRows(a: { id: string; _sort_value?: unknown; _sort_text?: string }, b: { id: string; _sort_value?: unknown; _sort_text?: string }, direction: number) {
  if (a._sort_value === null || a._sort_value === undefined) { if (b._sort_value !== null && b._sort_value !== undefined) return 1; }
  else if (b._sort_value === null || b._sort_value === undefined) return -1;
  const primary = direction * compareValue(a._sort_value, b._sort_value);
  return primary || direction * compareValue(a._sort_text, b._sort_text) || compareValue(a.id, b.id);
}
