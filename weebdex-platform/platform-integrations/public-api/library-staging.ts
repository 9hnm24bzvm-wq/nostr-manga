// Explicit preview-only fence. It never converts, deletes or rebinds saved data.
import {PUBLIC_MANGADEX_CATALOGUE} from '$env/static/public';
import {HTTPError} from '$lib/HTTP';
export const cataloguePreviewLibraryUnavailable=PUBLIC_MANGADEX_CATALOGUE==='1';
export const CATALOGUE_PREVIEW_LIBRARY_FENCE='Saved-library changes are unavailable in this catalogue preview. Your saved data is unchanged. Inspection and backups are still available.';
export function assertCataloguePreviewLibraryReady(){if(cataloguePreviewLibraryUnavailable)throw new HTTPError(501,CATALOGUE_PREVIEW_LIBRARY_FENCE,'Saved library unavailable');}
