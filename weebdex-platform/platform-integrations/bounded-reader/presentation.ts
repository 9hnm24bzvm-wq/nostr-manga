import type { Row } from './contract';
export function label(row: Pick<Row, 'volume' | 'chapter'>): string {
  const volume = row.volume === null ? '' : 'Vol. ' + (row.volume === '' ? '(empty label)' : row.volume) + ' · ';
  return volume + (row.chapter === null ? 'Oneshot' : row.chapter === '' ? 'Chapter (empty label)' : 'Ch. ' + row.chapter);
}
// Semantic buttons intentionally do not expose a stale direct-navigation href.
export function plainAction(event: MouseEvent, action: () => void | Promise<void>) {
  if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
  event.preventDefault(); void action(); // Controller catches every operation and publishes its owned error.
}

export function continueHistoryExplanation(mode: 'device' | 'account', publicCatalogue: boolean, pending: boolean): string {
  if (pending) return 'A library selection is pending. Continue waits before using either history source.';
  if (mode === 'device') return 'Continue checks device completion locally. Device history is never sent to the server.';
  if (publicCatalogue) return 'Native-account completion is unavailable in this public catalogue. Choose device history explicitly in Library if you want to use it.';
  return 'Continue checks selected chapter IDs against the signed-in native cookie account’s server history. Device completion remains local and is not sent with these requests.';
}

// Display metadata never affects the server-owned chapter or edition ordering.
// The compact reader DTO has no release/group names, so do not invent them or
// expose ID fragments as a substitute. Unknown languages stay explicit.
export function editionLanguage(code: string, names: Readonly<Record<string, string>>): string {
  const named = names[code] ?? names[code.toLowerCase()];
  if (typeof named === 'string' && named.trim()) return named;
  try { return new Intl.DisplayNames(['en'], { type: 'language', fallback: 'none' }).of(code) ?? 'Other language'; }
  catch { return 'Other language'; }
}
export function editionReleaseDate(publishedAt: string): string {
  const date = new Date(publishedAt.replace(' ', 'T').replace(/\+00$/, 'Z'));
  if (!Number.isFinite(date.getTime())) return 'Release date unavailable';
  return 'Published ' + new Intl.DateTimeFormat('en', { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' }).format(date);
}
export function editionChoices(rows: readonly Row[], names: Readonly<Record<string, string>>) {
  const languages = rows.map(row => editionLanguage(row.language, names)), counts = new Map<string, number>(), seen = new Map<string, number>();
  for (const language of languages) counts.set(language, (counts.get(language) ?? 0) + 1);
  return rows.map((row, index) => {
    const language = languages[index], ordinal = (seen.get(language) ?? 0) + 1; seen.set(language, ordinal);
    return { row, name: language + ((counts.get(language) ?? 0) > 1 ? ' · Translation ' + ordinal : ''), detail: editionReleaseDate(row.publishedAt) };
  });
}
