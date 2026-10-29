// Exact hash-pinned reader discovery/copy repairs. Preserved inputs are never written.
export const discoveryCopyPatches = [
  {
    "path": "src/lib/Locale.ts",
    "label": "correct Finnish language name",
    "sha256": "24683b1d2e4490108de376ef1afee3891e2a04994670f661f57dc753f1dbd214",
    "count": 1,
    "before": "fi: \"Finish\"",
    "after": "fi: \"Finnish\""
  },
  {
    "path": "src/lib/Locale.ts",
    "label": "correct Ukrainian language name",
    "sha256": "24683b1d2e4490108de376ef1afee3891e2a04994670f661f57dc753f1dbd214",
    "count": 1,
    "before": "uk: \"Ukranian\"",
    "after": "uk: \"Ukrainian\""
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "accurate total selections reading_status",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "+{search.reading_status.length}",
    "after": "{search.reading_status.length} selected",
    "approved_discovery_copy_exception": "accurate_selected_total"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "accurate total selections status",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "+{search.status.length}",
    "after": "{search.status.length} selected",
    "approved_discovery_copy_exception": "accurate_selected_total"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "accurate total selections contentRating",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "+{search.contentRating.length}",
    "after": "{search.contentRating.length} selected",
    "approved_discovery_copy_exception": "accurate_selected_total"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "accurate total selections demographic",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "+{search.demographic.length}",
    "after": "{search.demographic.length} selected",
    "approved_discovery_copy_exception": "accurate_selected_total"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "accurate total selections availableTranslatedLang",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "+{search.availableTranslatedLang.length}",
    "after": "{search.availableTranslatedLang.length} selected",
    "approved_discovery_copy_exception": "accurate_selected_total"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "accurate total selections lang",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "+{search.lang.length}",
    "after": "{search.lang.length} selected",
    "approved_discovery_copy_exception": "accurate_selected_total"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "explicit discovery language link import",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { page } from \"$app/state\";\r\n\timport { discoveryReadingQuery } from \"$platform/discovery-reading\";"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "derive title link without changing saved preferences",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "const slug = Utils.makeSlug(manga.title);",
    "after": "const slug = Utils.makeSlug(manga.title);\r\n\tconst titleLink = $derived(`/title/${manga.id}${slug}${discoveryReadingQuery(page.url, Locale.Languages)}`);"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "carry explicit search language through list title links",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 2,
    "before": "href=\"/title/{manga.id}{slug}\"",
    "after": "href={titleLink}",
    "approved_discovery_copy_exception": "search_reading_language_link"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "carry explicit search language through grid title link",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "href={isEdit ? undefined : `/title/${manga.id}${slug}`}",
    "after": "href={isEdit ? undefined : titleLink}",
    "approved_discovery_copy_exception": "search_reading_language_link"
  },
  {
    "path": "src/routes/(app)/library/+page.svelte",
    "label": "derive selected on-device list name",
    "sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
    "count": 1,
    "before": "let feed = $state<MangaFollowFeedResponse>();",
    "after": "let feed = $state<MangaFollowFeedResponse>();\r\n\tconst selectedListName = $derived($localLibraryState.ready && $localLibraryState.mode === \"device\" ? $localLibraryState.records.find(row => row.family === \"lists\" && row.id === page.url.searchParams.get(\"list\"))?.name : undefined);"
  },
  {
    "path": "src/routes/(app)/library/+page.svelte",
    "label": "name selected library list in page heading",
    "sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
    "count": 1,
    "before": "<h2 class=\"text-xl font-medium\">My Library</h2>",
    "after": "<h2 class=\"text-xl font-medium\">{selectedListName ? `${selectedListName} \u2014 My Library` : \"My Library\"}</h2>",
    "approved_discovery_copy_exception": "selected_list_name"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "wrap feed metadata instead of crowding comments",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "<div class=\"text-foreground/75 flex items-center gap-5\">",
    "after": "<div class=\"text-foreground/75 flex flex-wrap items-start gap-x-3 gap-y-1\">",
    "approved_discovery_copy_exception": "feed_metadata_wrapping"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "allow long author and group metadata to wrap",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "<div class=\"flex w-0 flex-auto items-center gap-1\">",
    "after": "<div class=\"flex min-w-0 basis-48 flex-auto flex-wrap items-center gap-x-1 gap-y-0.5\">",
    "approved_discovery_copy_exception": "feed_metadata_wrapping"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "show chapter language as text as well as flag",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "\t\t\t\t\talt={Locale.toLanguage(chapter.language)}\r\n\t\t\t\t/>",
    "after": "\t\t\t\t\talt={Locale.toLanguage(chapter.language)}\r\n\t\t\t\t/>\r\n\t\t\t\t<span class=\"shrink-0 text-xs\">{Locale.toLanguage(chapter.language)}</span>",
    "approved_discovery_copy_exception": "chapter_language_name"
  }
];
