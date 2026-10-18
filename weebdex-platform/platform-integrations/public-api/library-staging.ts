// Compatible anonymous UUID device commands now own scope/owner/CAS admission.
// Keep these old overlay names stable; account-cookie writes remain separate.
import {PUBLIC_MANGADEX_CATALOGUE} from '$env/static/public';
import {HTTPError} from '$lib/HTTP';
export const publicUuidLibraryEnabled=PUBLIC_MANGADEX_CATALOGUE==='1';
export const publicAccountLibraryUnavailable=publicUuidLibraryEnabled;
export const cataloguePreviewLibraryUnavailable=false;
export const CATALOGUE_PREVIEW_LIBRARY_FENCE='Saved-library changes need compatible catalogue commands.';
export function assertCataloguePreviewLibraryReady(){}
export function assertAccountLibraryReady(){if(publicAccountLibraryUnavailable)throw new HTTPError(501,'Use this device library for saved titles and reading progress. Account-library changes are not available for this catalogue. Your account and device records are unchanged.','Account library unavailable');}
