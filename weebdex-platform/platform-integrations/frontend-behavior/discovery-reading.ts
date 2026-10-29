// A search's explicit translated-language selection travels with its title link,
// not into saved preferences. Other discovery fields never become reader filters.
export function discoveryReadingQuery(url: URL, supportedLanguages: Record<string, string>): string {
  if (url.pathname !== '/search') return '';
  const enabled = url.searchParams.getAll('hasChapters');
  if (enabled.length > 1 || enabled.some(value => value !== 'true' && value !== '1')) return '';
  const selected = [...url.searchParams.getAll('availableTranslatedLang'), ...url.searchParams.getAll('availableTranslatedLang[]')];
  if (!selected.length || selected.length > 100 || selected.some(code => !Object.hasOwn(supportedLanguages, code))) return '';
  const query = new URLSearchParams();
  for (const code of new Set(selected)) query.append('tlang', code);
  return '?' + query.toString().replace(/^\?/, '');
}
