// Selected public catalogue only. The native parser keeps its Locale vocabulary.
import * as native from '../frontend-behavior/manga-search-query';
import { publicSearchLanguage } from './language';
export { serializeMangaSearchQuery, searchValidationError, unresolvedSearchIssues } from '../frontend-behavior/manga-search-query';
export type { MangaFilterRequest, SearchIssue, ParsedMangaSearch } from '../frontend-behavior/manga-search-query';
export const parseMangaSearchQuery = (...args: Parameters<typeof native.parseMangaSearchQuery>) => native.parseMangaSearchQuery(args[0], args[1], args[2], publicSearchLanguage);
export const validateMangaSearchQuery = (...args: Parameters<typeof native.validateMangaSearchQuery>) => native.validateMangaSearchQuery(args[0], args[1], publicSearchLanguage);
export const mangaSearchReadQuery = (...args: Parameters<typeof native.mangaSearchReadQuery>) => native.mangaSearchReadQuery(args[0], args[1], publicSearchLanguage);
export const effectiveMangaSearchReadQuery = (...args: Parameters<typeof native.effectiveMangaSearchReadQuery>) => native.effectiveMangaSearchReadQuery(args[0], args[1], args[2], args[3], publicSearchLanguage);
