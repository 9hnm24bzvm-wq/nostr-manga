// Selected public catalogue only. The native parser keeps its Locale vocabulary.
import * as native from '../frontend-behavior/manga-search-query';
import { publicSearchLanguage } from './language';
import { get } from 'svelte/store';
import { localLibraryState } from '../local-library/library';
import { uuidPattern } from './validation';
const acceptsList = (value: string) => get(localLibraryState).mode === 'account' ? uuidPattern.test(value) : /^[0-9a-f]{32}$/.test(value);
export { serializeMangaSearchQuery, searchValidationError, unresolvedSearchIssues } from '../frontend-behavior/manga-search-query';
export type { MangaFilterRequest, SearchIssue, ParsedMangaSearch } from '../frontend-behavior/manga-search-query';
export const parseMangaSearchQuery = (...args: Parameters<typeof native.parseMangaSearchQuery>) => native.parseMangaSearchQuery(args[0], args[1], args[2], publicSearchLanguage, acceptsList);
export const validateMangaSearchQuery = (...args: Parameters<typeof native.validateMangaSearchQuery>) => native.validateMangaSearchQuery(args[0], args[1], publicSearchLanguage, acceptsList);
export const mangaSearchReadQuery = (...args: Parameters<typeof native.mangaSearchReadQuery>) => native.mangaSearchReadQuery(args[0], args[1], publicSearchLanguage, acceptsList);
export const effectiveMangaSearchReadQuery = (...args: Parameters<typeof native.effectiveMangaSearchReadQuery>) => native.effectiveMangaSearchReadQuery(args[0], args[1], args[2], args[3], publicSearchLanguage, acceptsList);
