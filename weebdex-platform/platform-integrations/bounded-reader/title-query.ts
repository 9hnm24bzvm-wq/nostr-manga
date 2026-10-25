// Preserve MangaActions' exact title-page overrides before native/public filter
// translation. An absent title key leaves preferences intact; repeated values
// replace all preferred values for that key, without widening query controls.
export function titleReaderFilters(applyPreferences: (query: URLSearchParams) => void, titleQuery: URLSearchParams): URLSearchParams {
  const query = new URLSearchParams(); applyPreferences(query);
  for (const key of ['tlang', 'group']) if (titleQuery.has(key)) {
    query.delete(key); titleQuery.getAll(key).forEach(value => query.append(key, value));
  }
  return query;
}
