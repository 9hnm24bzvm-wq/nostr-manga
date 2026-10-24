// Typed UI state is separate from unowned query passthrough. Bounds and
// vocabularies mirror catalog QueryParams and native device-list admission.
import type { SearchMangaRequest } from '$lib/API/Manga';
import * as Manga from '$lib/constants/Manga';
import * as MangaFollow from '$lib/constants/MangaFollow';
import { Languages } from '$lib/Locale';
import { Error as SearchError } from '$lib/constants/Error';
export type MangaFilterRequest = SearchMangaRequest & { id?: string[]; artist?: string[]; authorOrArtist?: string[]; list?: string };
export type SearchIssue = { fields: (keyof MangaFilterRequest)[]; message: string };
export type ParsedMangaSearch = { search: MangaFilterRequest; passthrough: URLSearchParams; issues: SearchIssue[]; explicitHasChapters: boolean; explicitEmptyRating: boolean; page?: number };
const arrayKeys = ['lang', 'langx', 'demographic', 'status', 'reading_status', 'contentRating', 'availableTranslatedLang', 'tag', 'tagx', 'author', 'artist', 'artists', 'id', 'authorOrArtist'] as const;
const scalarKeys = ['title', 'yearFrom', 'yearTo', 'hasChapters', 'tmod', 'txmod', 'limit', 'sort', 'order', 'list'] as const;
// The backend also admits the retained legacy mx code absent from Locale labels.
export const searchLanguages = new Set([...Object.keys(Languages), 'mx']);
const sharedScalarKeys = ['name', 'rank', 'time', 'draft'] as const;
const sharedLanguageKeys = ['tlang', 'tlangx'] as const;
const sharedIdKeys = ['manga', 'uploader', 'uploaderx', 'group', 'groupx'] as const;
const sharedKeys = new Set<string>([...sharedScalarKeys, ...sharedLanguageKeys, ...sharedIdKeys]);
const id = (value: string) => /^[a-zA-Z0-9_:-]{1,128}$/.test(value);
const invalid = (key: string): never => { throw new Error('Invalid ' + key); };
function choice<T extends string>(key: string, value: string, choices: readonly T[]): T {
  const found = choices.find(candidate => candidate === value);
  return found === undefined ? invalid(key) : found;
}
function number(key: string, value: string, maximum: number, minimum = 0): number {
  if (!/^[+-]?[0-9]+$/.test(value)) return invalid(key);
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= minimum && parsed <= maximum ? parsed : invalid(key);
}
function mode(key: 'tmod' | 'txmod', value: string): string {
  if (value === '0' || value === '1') return value;
  const upper = value.toUpperCase();
  if (upper !== 'AND' && upper !== 'OR') return invalid(key);
  return upper === (key === 'tmod' ? 'AND' : 'OR') ? '0' : '1';
}
export function parseMangaSearchQuery(defaults: SearchMangaRequest, query: URLSearchParams, library: boolean, acceptsLanguage: (value:string)=>boolean = value => searchLanguages.has(value)): ParsedMangaSearch {
  const search: MangaFilterRequest = structuredClone(defaults), passthrough = new URLSearchParams(), issues: SearchIssue[] = [];
  const values = new Map<string, string[]>();
  for (const [rawKey, value] of query) {
    const key = rawKey.endsWith('[]') ? rawKey.slice(0, -2) : rawKey;
    const rows = values.get(key) ?? []; rows.push(value); values.set(key, rows);
    if (key !== 'page' && ![...arrayKeys, ...scalarKeys].some(known => known === key)) passthrough.append(sharedKeys.has(key) || library && key === 'username' ? key : rawKey, value);
  }
  let page: number | undefined;
  const apply = (fields: (keyof MangaFilterRequest)[], operation: () => void) => {
    try { operation(); } catch (error) { issues.push({ fields, message: error instanceof Error ? error.message : 'Invalid search filter' }); }
  };
  if (new TextEncoder().encode(query.toString().replace(/^\?/, '')).length > 16384) issues.push({ fields: [...arrayKeys, ...scalarKeys].filter(key => values.has(key)), message: 'Query is too long' });
  for (const [key, rows] of values) {
    if (new TextEncoder().encode(key).length > 64 || rows.length > 100 || rows.some(value => new TextEncoder().encode(value).length > 1000)) {
      const field = [...arrayKeys, ...scalarKeys].find(field => field === key);
      issues.push({ fields: field ? [field] : [], message: 'Invalid query value budget' });
    }
  }
  for (const key of sharedScalarKeys) {
    const rows = values.get(key); if (rows && rows.length !== 1) issues.push({ fields: [], message: key + ' must occur once' });
  }
  const usernames = values.get('username');
  if (library && usernames && usernames.length !== 1) issues.push({ fields: [], message: 'username must occur once' });
  for (const key of sharedLanguageKeys) {
    const rows = values.get(key); if (rows?.some(value => !acceptsLanguage(value))) issues.push({ fields: [], message: 'Invalid ' + key });
  }
  for (const key of sharedIdKeys) {
    const rows = values.get(key); if (rows?.some(value => !id(value))) issues.push({ fields: [], message: 'Invalid ' + key });
  }
  const pages = values.get('page');
  if (pages) apply([], () => { if (pages.length !== 1) invalid('page'); page = number('page', pages[0], 1000000, 1); });
  for (const key of arrayKeys) {
    const rows = values.get(key); if (!rows) continue;
    apply([key], () => {
      switch (key) {
        case 'demographic': search.demographic = rows.map(value => choice(key, value, Object.values(Manga.Demographic))); break;
        case 'status': search.status = rows.map(value => choice(key, value, Object.values(Manga.PublicationStatus))); break;
        case 'contentRating':
          if (rows.includes('__none__')) { if (rows.length !== 1) invalid(key); search.contentRating = []; }
          else search.contentRating = rows.map(value => choice(key, value, Object.values(Manga.ContentRating)));
          break;
        case 'reading_status': search.reading_status = rows.map(value => choice(key, value, Object.values(Manga.ReadingStatus))); break;
        case 'lang': case 'langx': case 'availableTranslatedLang':
          if (rows.some(value => !acceptsLanguage(value))) invalid(key);
          search[key] = rows; break;
        default:
          if (rows.some(value => !id(value))) invalid(key);
          search[key] = rows;
      }
    });
  }
  for (const key of scalarKeys) {
    const rows = values.get(key); if (!rows) continue;
    apply([key], () => {
      if (rows.length !== 1) invalid(key);
      const value = rows[0];
      switch (key) {
        case 'title': search.title = value; break;
        case 'yearFrom': case 'yearTo': search[key] = number(key, value, 9999); break;
        case 'limit': search.limit = number(key, value, 100); break;
        case 'hasChapters':
          if (!['true', 'false', '1', '0'].includes(value)) invalid(key);
          search.hasChapters = value === 'true' || value === '1'; break;
        case 'tmod': case 'txmod': search[key] = mode(key, value); break;
        case 'order': search.order = choice(key, value, ['asc', 'desc']); break;
        case 'sort': search.sort = library ? choice(key, value, Object.values(MangaFollow.SortBy)) : choice(key, value, Object.values(Manga.SortBy)); break;
        case 'list': if (!library || !/^[0-9a-f]{32}$/.test(value)) invalid(key); search.list = value; break;
      }
    });
  }
  if (search.yearFrom !== undefined && search.yearTo !== undefined && search.yearFrom > search.yearTo) issues.push({ fields: ['yearFrom', 'yearTo'], message: 'yearFrom exceeds yearTo' });
  return { search, passthrough, issues, explicitHasChapters: values.has('hasChapters'), explicitEmptyRating: values.get('contentRating')?.length === 1 && values.get('contentRating')?.[0] === '__none__', page };
}
export function searchValidationError(issues: readonly SearchIssue[]): SearchError | undefined {
  return issues.length ? new SearchError('Invalid search filters', [...new Set(issues.map(issue => issue.message))].join('. ') + '. Correct the filters or use Reset. No replacement results were loaded.') : undefined;
}
export function validateMangaSearchQuery(query: URLSearchParams, library: boolean, acceptsLanguage?: (value:string)=>boolean): SearchError | undefined {
  return searchValidationError(parseMangaSearchQuery({ limit: 20, sort: library ? MangaFollow.SortBy.FollowedAt : Manga.SortBy.LastUploadedChapterAt, order: 'desc' }, query, library, acceptsLanguage).issues);
}
export function unresolvedSearchIssues(issues: readonly SearchIssue[], initial: MangaFilterRequest, edited: MangaFilterRequest): SearchIssue[] {
  return issues.filter(issue => !issue.fields.length || issue.fields.every(field => JSON.stringify(initial[field]) === JSON.stringify(edited[field])));
}
export function serializeMangaSearchQuery(search: MangaFilterRequest, passthrough: URLSearchParams, explicitHasChapters: boolean, explicitEmptyRating = false): URLSearchParams {
  // Never allow passthrough to replace a validated field or restore pagination.
  const query = new URLSearchParams();
  for (const [rawKey, value] of passthrough) {
    const key = rawKey.endsWith('[]') ? rawKey.slice(0, -2) : rawKey;
    if (key !== 'page' && ![...arrayKeys, ...scalarKeys].some(known => known === key)) query.append(rawKey, value);
  }
  for (const [key, value] of Object.entries(search)) {
    if (value === undefined || value === null || typeof value === 'string' && !value.length) continue;
    if (Array.isArray(value)) for (const item of value) query.append(key, item);
    else query.set(key, String(value));
  }
  if (explicitEmptyRating && !search.contentRating?.length) query.set('contentRating', '__none__');
  if (!search.hasChapters && !explicitHasChapters) query.delete('hasChapters');
  if (search.tmod !== undefined) query.set('tmod', search.tmod === '0' ? 'AND' : search.tmod === '1' ? 'OR' : search.tmod);
  if (search.txmod !== undefined) query.set('txmod', search.txmod === '0' ? 'OR' : search.txmod === '1' ? 'AND' : search.txmod);
  return query;
}

// Initial reads use the same accepted query values as Apply, before preference
// defaults are added. Only explicitly present fields are emitted; pagination is
// validated separately and retained here, while Apply always starts a new page.
export function mangaSearchReadQuery(input: URLSearchParams, library: boolean, acceptsLanguage?: (value:string)=>boolean): { query: URLSearchParams; error?: SearchError } {
  const parsed = parseMangaSearchQuery({ limit: 20, sort: library ? MangaFollow.SortBy.FollowedAt : Manga.SortBy.LastUploadedChapterAt, order: 'desc' }, input, library, acceptsLanguage);
  const query = serializeMangaSearchQuery(parsed.search, parsed.passthrough, parsed.explicitHasChapters, parsed.explicitEmptyRating);
  const present = new Set([...input.keys()].map(key => key.endsWith('[]') ? key.slice(0, -2) : key));
  for (const key of [...arrayKeys, ...scalarKeys]) if (!present.has(key)) query.delete(key);
  if (parsed.page !== undefined) query.set('page', String(parsed.page));
  return { query, error: searchValidationError(parsed.issues) };
}

// Cache admission and the actual API read must validate the same final wire,
// including preference rows and the default limit appended by the route.
export function effectiveMangaSearchReadQuery(input: URLSearchParams, library: boolean, preferences: { applyMangaPreferences(query: URLSearchParams): void }, defaultLimit: string, acceptsLanguage?: (value:string)=>boolean): { query: URLSearchParams; error?: SearchError } {
  const admitted = mangaSearchReadQuery(input, library, acceptsLanguage);
  if (admitted.error) return admitted;
  if (!library) preferences.applyMangaPreferences(admitted.query);
  if (!admitted.query.has('limit')) admitted.query.set('limit', defaultLimit);
  return { query: admitted.query, error: validateMangaSearchQuery(admitted.query, library, acceptsLanguage) };
}
