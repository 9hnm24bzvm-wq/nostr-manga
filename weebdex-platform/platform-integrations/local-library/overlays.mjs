import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
const patches = [
  {
    "path": "src/routes/(app)/library/+layout.svelte",
    "label": "device availability scoped layout import",
    "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
    "count": 1,
    "before": "import Protected from \"$lib/components/ui/Protected.svelte\";",
    "after": "import Protected from \"$localLibrary/LibraryProtected.svelte\";"
  },
  {
    "path": "src/routes/(app)/history/+layout.svelte",
    "label": "device availability scoped layout import",
    "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
    "count": 1,
    "before": "import Protected from \"$lib/components/ui/Protected.svelte\";",
    "after": "import Protected from \"$localLibrary/LibraryProtected.svelte\";"
  },
  {
    "path": "src/routes/(app)/settings/library/+layout.svelte",
    "label": "device availability scoped layout import",
    "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
    "count": 1,
    "before": "import Protected from \"$lib/components/ui/Protected.svelte\";",
    "after": "import Protected from \"$localLibrary/LibraryProtected.svelte\";"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "independent library imports",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity, localLibraryState, openLibrary, message, captureLibraryContext, assertLibraryContext, applyAccountFollow } from \"$localLibrary/library\";"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "reactive follows from selected independent library",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "let isFollowed = $state($Session?.follows && manga.id in $Session.follows);",
    "after": "let isFollowed = $state(manga.id in $libraryIdentity.follows);\r\n\t$effect(() => { isFollowed = manga.id in $libraryIdentity.follows; });\r\n\tvoid openLibrary().catch(() => {});"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "local save availability guards",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 2,
    "before": "if (!$Session) {",
    "after": "if (!$libraryIdentity.canUse) {"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "account cache mutations remain account only",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "if ($Session) {",
    "after": "if ($Session && $libraryIdentity.mode === \"account\") {"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "account unfollow cache mutation remains account only",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "if ($Session?.follows && manga.id in $Session.follows) {",
    "after": "if ($libraryIdentity.mode === \"account\" && $Session?.follows && manga.id in $Session.follows) {"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "follow card buttons usable without account",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "{#if $Session}",
    "after": "{#if $libraryIdentity.canUse}",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "grid follow button usable without account",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "{#if $Session && !isEdit}",
    "after": "{#if $libraryIdentity.canUse && !isEdit}",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "follow card failures displayed",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t\tconst res = await API.MangaFollow.Update(manga.id, {",
    "after": "\t\t\ttry {\r\n\t\t\tconst res = await API.MangaFollow.Update(manga.id, {"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "follow catch before unfollow",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t});\r\n\r\n\tconst unfollow",
    "after": "\t\t\t} catch (error) { window.dispatchEvent(new CustomEvent(\"weebdex-library-error\", { detail: message(error) })); }\r\n\t\t});\r\n\r\n\tconst unfollow"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "unfollow catch start",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t\tconst res = await API.MangaFollow.Unfollow(manga.id);",
    "after": "\t\t\ttry {\r\n\t\t\tconst res = await API.MangaFollow.Unfollow(manga.id);"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "unfollow catch end",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t});\r\n</script>",
    "after": "\t\t\t} catch (error) { window.dispatchEvent(new CustomEvent(\"weebdex-library-error\", { detail: message(error) })); }\r\n\t\t});\r\n</script>"
  },
  {
    "path": "src/lib/components/ChapterFeedMangaEntry.svelte",
    "label": "independent library imports",
    "sha256": "fe05c6a639a507a1cfb082d3b3a322dfa998eeb1fc04064ba7690f9b6c0ed00b",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity } from \"$localLibrary/library\";"
  },
  {
    "path": "src/lib/components/ChapterFeedMangaEntry.svelte",
    "label": "reactive selected-library follow indicator",
    "sha256": "fe05c6a639a507a1cfb082d3b3a322dfa998eeb1fc04064ba7690f9b6c0ed00b",
    "count": 1,
    "before": "let isFollowed = $state($Session?.follows && manga.id in $Session.follows);",
    "after": "let isFollowed = $derived(manga.id in $libraryIdentity.follows);"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "independent library imports",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity, localLibraryState, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "anonymous progress button visibility",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "{#if $Session}",
    "after": "{#if $libraryIdentity.canUse}",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "anonymous unread border visibility",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "$Session && !isRead",
    "after": "$libraryIdentity.canUse && !isRead",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "reactive local chapter history",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "const isRead = $derived(\r\n\t\t$readHistory && manga.id in $readHistory ? chapter.id in ($readHistory[manga.id] ?? {}) : false\r\n\t);",
    "after": "const isRead = $derived($libraryIdentity.mode === \"device\"\r\n\t\t? ($localLibraryState.scopeStatus === \"verified\" && $localLibraryState.records.some(row => row.family === \"chapters\" && row.id === chapter.id && row.completed))\r\n\t\t: ($readHistory && manga.id in $readHistory ? chapter.id in ($readHistory[manga.id] ?? {}) : false));"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "independent library imports",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity, localLibraryState, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "anonymous progress button visibility",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "{#if $Session}",
    "after": "{#if $libraryIdentity.canUse}",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "anonymous unread border visibility",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "$Session && !isRead",
    "after": "$libraryIdentity.canUse && !isRead",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "reactive local chapter history",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "const isRead = $derived(\r\n\t\t$readHistory && manga.id in $readHistory ? chapter.id in ($readHistory[manga.id] ?? {}) : false\r\n\t);",
    "after": "const isRead = $derived($libraryIdentity.mode === \"device\"\r\n\t\t? ($localLibraryState.scopeStatus === \"verified\" && $localLibraryState.records.some(row => row.family === \"chapters\" && row.id === chapter.id && row.completed))\r\n\t\t: ($readHistory && manga.id in $readHistory ? chapter.id in ($readHistory[manga.id] ?? {}) : false));"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "independent library imports",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity, libraryViewKey, captureLibraryView } from \"$localLibrary/library\";"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "local chapter history fetch",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "if ($Session) newIds.push(id);",
    "after": "if ($libraryIdentity.canUse) newIds.push(id);"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "local chapter history query",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "if ($Session && newIds.length) {",
    "after": "if ($libraryIdentity.canUse && newIds.length) {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "independent library imports",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity, localLibraryState, openLibrary, captureLibraryContext, assertLibraryContext, applyAccountFollow } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "local follow initialization",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "await Session.isValid();",
    "after": "await openLibrary();\r\n\t\tif ($libraryIdentity.mode === \"account\") await Session.isValid();"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "selected library follow status lookup",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "const status = $Session?.follows?.[manga.id];",
    "after": "const status = $libraryIdentity.follows[manga.id];"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "anonymous follow status guard",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "if (!$Session || readingStatus === currentReadingStatus || !isPublished) {",
    "after": "if (!$libraryIdentity.canUse || readingStatus === currentReadingStatus || !isPublished) {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "anonymous notification guard",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "if (!$Session || !currentReadingStatus || !isPublished) {",
    "after": "if (!$libraryIdentity.canUse || !currentReadingStatus || !isPublished) {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "anonymous unfollow guard",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "if (!$Session || !isPublished) {",
    "after": "if (!$libraryIdentity.canUse || !isPublished) {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "account status cache writes only in account mode",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 2,
    "before": "if ($Session) {",
    "after": "if ($Session && $libraryIdentity.mode === \"account\") {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "account unfollow cache only in account mode",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "if ($Session?.follows && manga.id in $Session.follows) {",
    "after": "if ($libraryIdentity.mode === \"account\" && $Session?.follows && manga.id in $Session.follows) {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "anonymous follow control visibility",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "{#if $Session}",
    "after": "{#if $libraryIdentity.canUse}",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "reactive follow controls before action definition",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\tconst updateFollowStatus =",
    "after": "\t$effect(() => { const status = $libraryIdentity.follows[manga.id]; currentReadingStatus = status?.[0] as Constants.Manga.ReadingStatus | undefined; currentNotificationStatus = status ? status[1] === 1 : undefined; });\r\n\r\n\tconst updateFollowStatus ="
  },
  {
    "path": "src/lib/components/reader/reader-history.svelte",
    "label": "UI15 automatic reader history requires actual acknowledgement and current-owned failure feedback",
    "sha256": "ba14aa6c0d83334004b5d680d8cff52d842b7061bad6f31fbc7cc858d2f67052",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport API from \"$lib/API\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { useReader } from \"./context.svelte\";\r\n\r\n\tconst { chapter } = useReader();\r\n\tonMount(async () => {\r\n\t\tif (chapter && (await Session.isValid())) {\r\n\t\t\tAPI.ChapterHistory.UpdateBatch(chapter.relationships.manga.id, {\r\n\t\t\t\tread_ids: [chapter.id]\r\n\t\t\t});\r\n\t\t}\r\n\t});\r\n</script>\r\n",
    "after": "<script lang=\"ts\">\r\n  import API from \"$lib/API\";\r\n  import { onMount, onDestroy } from \"svelte\";\r\n  import { toast } from \"svelte-sonner\";\r\n  import { useReader } from \"./context.svelte\";\r\n  import { localLibraryState, saveProgress, message } from \"$localLibrary/library\";\r\n  import { createLibraryActionOwner } from \"$localLibrary/action-owner\";\r\n  import { nativeAuthority } from \"$platform/mutation-authority\";\r\n  import { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";\r\n  const reader = useReader();\r\n  const historyOwner = createLibraryActionOwner(() => JSON.stringify([reader.chapter?.id, reader.chapter?.relationships.manga.id]));\r\n  let disposed = false, progressToken = \"\", timer = 0;\r\n  type ProgressIntent = ReturnType<typeof historyOwner.capture>;\r\n  let queued: { manga: string; chapter: string; page: number; completed: boolean; intent: ProgressIntent } | undefined;\r\n  const flush = (finalPreviouslyAdmittedPage = false) => {\r\n    const pending = queued; queued = undefined;\r\n    if (!pending) return;\r\n    // Exit may finish the immutable page action already admitted while live;\r\n    // it never discovers a different account/mode after destruction.\r\n    const assert = finalPreviouslyAdmittedPage ? pending.intent.assertDestination : pending.intent.assert;\r\n    try { assert(); } catch { return; }\r\n    void saveProgress(pending.manga, pending.chapter, pending.page, pending.completed, assert).catch(error => { if (!disposed) toast.error(\"Progress save failed\", { description: message(error) }); });\r\n  };\r\n  onDestroy(() => { window.clearTimeout(timer); flush(true); disposed = true; });\r\n  onMount(() => {\r\n    const intent = historyOwner.capture();\r\n    void intent.open().catch(error => { if (intent.alive()) toast.error(\"Local library unavailable\", { description: message(error) }); });\r\n  });\r\n  let accountReadToken = \"\";\r\n  $effect(() => {\r\n    const actor = $nativeAuthority.actor, epoch = $nativeAuthority.epoch, chapter = reader.chapter;\r\n    if (disposed || !$localLibraryState.ready || $localLibraryState.mode !== \"account\" || !actor || !chapter) return;\r\n    const mangaId = chapter.relationships.manga.id, chapterId = chapter.id, token = JSON.stringify([actor, epoch, mangaId, chapterId]);\r\n    if (token === accountReadToken) return;\r\n    // A still-mounted reader can admit initial identity readiness, but a\r\n    // destroyed callback cannot select that later identity. Harmless Session\r\n    // data refreshes do not issue another history POST.\r\n    const intent = historyOwner.capture(); accountReadToken = token;\r\n    void intent.open().then(async context => {\r\n      intent.assert();\r\n      const response = await API.ChapterHistory.UpdateBatch(mangaId, { read_ids: [chapterId] }, { context, assert: intent.assert });\r\n      intent.assert();\r\n      requireMutationAcknowledgement(response, 200, true);\r\n    }).catch(error => reportCurrentMutationFailure(\"History save failed\", error, intent.alive));\r\n  });\r\n  $effect(() => {\r\n    if (disposed || !$localLibraryState.ready || $localLibraryState.mode !== \"device\" || $localLibraryState.scopeStatus !== \"verified\" || !reader.chapter || !reader.currentPage) return;\r\n    const chapter = reader.chapter, page = reader.currentPage.index + 1;\r\n    const completed = reader.isViewingLastPage || reader.currentPage.index === reader.pages.length - 1;\r\n    const intent = historyOwner.capture();\r\n    const token = JSON.stringify([intent.destinationKey, chapter.id, page, completed]);\r\n    if (token === progressToken) return;\r\n    progressToken = token; window.clearTimeout(timer);\r\n    queued = { manga: chapter.relationships.manga.id, chapter: chapter.id, page, completed, intent };\r\n    timer = window.setTimeout(() => flush(), 350);\r\n  });\r\n</script>\r\n"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "independent library imports",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { captureImportContext, assertImportContext, isAccountMode, exportDeviceCsv, parseDeviceCsv, downloadBlob } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "device export is scoped browser backup",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "\tconst onExport = () =>\r\n\t\tmutex.exec(() => {",
    "after": "\tconst onExport = () =>\r\n\t\tmutex.exec(async () => {"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "device export branch",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "\t\t\t\tconst anchor = document.createElement(\"a\");",
    "after": "\t\t\t\tif (!(await isAccountMode())) { downloadBlob(await exportDeviceCsv(), \"weebdex-device-library.csv\"); return; }\r\n\t\t\t\tconst anchor = document.createElement(\"a\");"
  },
  {
    "path": "src/lib/components/AppSidebar.svelte",
    "label": "independent library imports",
    "sha256": "4b791bc6e08322d2fcb4e161c7c8861502073fb76abce47d42f33fa9d7e985c5",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { libraryIdentity } from \"$localLibrary/library\";"
  },
  {
    "path": "src/lib/components/AppSidebar.svelte",
    "label": "library settings independent access",
    "sha256": "4b791bc6e08322d2fcb4e161c7c8861502073fb76abce47d42f33fa9d7e985c5",
    "count": 1,
    "before": "action: $Session ? { name: \"Settings\", url: \"/settings/library\", icon: SettingsIcon } : undefined",
    "after": "action: $libraryIdentity.canUse ? { name: \"Settings\", url: \"/settings/library\", icon: SettingsIcon } : undefined"
  },
  {
    "path": "src/routes/(app)/+page.svelte",
    "label": "independent library imports",
    "sha256": "9829b73f4767505d3187c913b95e645c6af6f34884684463f2210a296efb205d",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\n\timport { libraryIdentity, localLibraryState, libraryViewKey } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/+page.svelte",
    "label": "device updates carousel visibility",
    "sha256": "9829b73f4767505d3187c913b95e645c6af6f34884684463f2210a296efb205d",
    "count": 1,
    "before": "{#if $Session}",
    "after": "{#if $libraryIdentity.canUse}",
    "approved_library_exception": "library_availability_guard"
  },
  {
    "path": "src/routes/(app)/+page.svelte",
    "label": "home update source boundary",
    "sha256": "9829b73f4767505d3187c913b95e645c6af6f34884684463f2210a296efb205d",
    "count": 1,
    "before": "\t\t<MyUpdatesCarousel />",
    "after": "\t\t{#key $libraryViewKey}\n\t\t<MyUpdatesCarousel />\n\t\t{/key}",
    "approved_library_exception": "library_reactivity_key"
  },
  {
    "path": "src/lib/components/MangaSearch.svelte",
    "label": "private library bypasses unscoped history cache",
    "sha256": "29ab147906ba70a2301b96e54c03eb4f454e0c67b43a9bfb8d72a7e2ea930da5",
    "count": 1,
    "before": "if (state._$MangaSearch_feed) {",
    "after": "if (!window.location.pathname.startsWith(\"/library\") && state._$MangaSearch_feed) {"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "private history bypasses unscoped history cache",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "if (state._$ChapterFeed_feed && state._$ChapterFeed_entries) {",
    "after": "if (![\"/library\", \"/history\"].some(prefix => window.location.pathname.startsWith(prefix)) && state._$ChapterFeed_feed && state._$ChapterFeed_entries) {"
  },
  {
    "path": "src/lib/components/MyUpdatesCarousel.svelte",
    "label": "source-specific carousel state is instance local",
    "sha256": "9335b54eab4aef35bd6a42dab0f2157e898a05eb85d0b207169c1d6db7ea2ccd",
    "count": 1,
    "before": "<script lang=\"ts\" module>\r\n\tlet feed = $state<MappedChapterFeedResponse>();\r\n\tlet entries = $state<Chapter[]>([]);\r\n\tlet idMap = $state<Record<string, undefined>>({});\r\n</script>",
    "after": "<script lang=\"ts\" module>\r\n</script>"
  },
  {
    "path": "src/lib/components/MyUpdatesCarousel.svelte",
    "label": "carousel instance caches",
    "sha256": "9335b54eab4aef35bd6a42dab0f2157e898a05eb85d0b207169c1d6db7ea2ccd",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\tlet feed = $state<MappedChapterFeedResponse>();\r\n\tlet entries = $state<Chapter[]>([]);\r\n\tlet idMap = $state<Record<string, undefined>>({});"
  },
  {
    "path": "src/routes/(app)/library/+page.svelte",
    "label": "independent library imports",
    "sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
    "count": 1,
    "before": "<script lang=\"ts\" module>",
    "after": "<script lang=\"ts\" module>\r\n\timport { localLibraryState, libraryViewKey } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/library/+page.svelte",
    "label": "selected library search cache invalidation",
    "sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
    "count": 2,
    "before": "{#key page.url.search}",
    "after": "{#key page.url.search + \":\" + $libraryViewKey}",
    "approved_library_exception": "library_reactivity_key"
  },
  {
    "path": "src/routes/(app)/library/updates/+page.svelte",
    "label": "independent library imports",
    "sha256": "9a9b125397128409cad1ac460caeab3786c685e63c72e046db888075c289f89b",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { localLibraryState, libraryViewKey } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/library/updates/+page.svelte",
    "label": "selected library mode boundary",
    "sha256": "9a9b125397128409cad1ac460caeab3786c685e63c72e046db888075c289f89b",
    "count": 1,
    "before": "\t<ChapterFeed",
    "after": "\t{#key $libraryViewKey}\r\n\t<ChapterFeed",
    "approved_library_exception": "library_reactivity_key"
  },
  {
    "path": "src/routes/(app)/library/updates/+page.svelte",
    "label": "selected library mode boundary end",
    "sha256": "9a9b125397128409cad1ac460caeab3786c685e63c72e046db888075c289f89b",
    "count": 1,
    "before": "\t/>\r\n</section>",
    "after": "\t/>\r\n\t{/key}\r\n</section>",
    "approved_library_exception": "library_reactivity_key"
  },
  {
    "path": "src/routes/(app)/history/+page.svelte",
    "label": "independent library imports",
    "sha256": "e5485a54b15a1477c1fde99cb8e248dfc5e0b8a4ab95733eeb7b6971cf08a59f",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { localLibraryState, libraryViewKey } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/history/+page.svelte",
    "label": "selected library mode boundary",
    "sha256": "e5485a54b15a1477c1fde99cb8e248dfc5e0b8a4ab95733eeb7b6971cf08a59f",
    "count": 1,
    "before": "\t<ChapterFeed",
    "after": "\t{#key $libraryViewKey}\r\n\t<ChapterFeed",
    "approved_library_exception": "library_reactivity_key"
  },
  {
    "path": "src/routes/(app)/history/+page.svelte",
    "label": "selected library mode boundary end",
    "sha256": "e5485a54b15a1477c1fde99cb8e248dfc5e0b8a4ab95733eeb7b6971cf08a59f",
    "count": 1,
    "before": "\t/>\r\n</section>",
    "after": "\t/>\r\n\t{/key}\r\n</section>",
    "approved_library_exception": "library_reactivity_key"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "CSV exporter and retained importer agree with explicit scope metadata",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "\t\tconst entries = await new Promise<{ id: string; status: Constants.Manga.ReadingStatus }[]>(resolve => {\r\n\t\t\tPapa.parse(csvString, {\r\n\t\t\t\tcomplete: results => {\r\n\t\t\t\t\tresolve(\r\n\t\t\t\t\t\tresults.data\r\n\t\t\t\t\t\t\t.slice(1)\r\n\t\t\t\t\t\t\t.map(([id, , status]: any) => ({ id, status }))\r\n\t\t\t\t\t\t\t.filter(e => e.id.length === 10 && e.status in Constants.Manga.ReadingStatusText)\r\n\t\t\t\t\t);\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t});",
    "after": "\t\tconst entries = await parseDeviceCsv(csvString, Papa) as { id: string; status: Constants.Manga.ReadingStatus }[];"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "importWeebDex captures one whole-file import destination",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "\tconst importWeebDex = async (file: File) => {",
    "after": "\tconst importWeebDex = async (file: File) => {\r\n\t\tconst importContext = await captureImportContext();"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "importMAL captures one whole-file import destination",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "\tconst importMAL = async (file: File) => {",
    "after": "\tconst importMAL = async (file: File) => {\r\n\t\tconst importContext = await captureImportContext();"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "weebdex batch retains frozen mode scope and account",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "const result = await API.MangaFollow.Import({ weebdex: chunk });",
    "after": "assertImportContext(importContext);\r\n\t\t\tconst result = await API.MangaFollow.Import({ weebdex: chunk }, importContext);\r\n\t\t\tassertImportContext(importContext);"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "myanimelist batch retains frozen mode scope and account",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "const result = await API.MangaFollow.Import({ myanimelist: chunk });",
    "after": "assertImportContext(importContext);\r\n\t\t\tconst result = await API.MangaFollow.Import({ myanimelist: chunk }, importContext);\r\n\t\t\tassertImportContext(importContext);"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "read control captures intended state and library generation before update",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "\t\tisUpdatingReadHistory = true;",
    "after": "\t\tisUpdatingReadHistory = true;\r\n\t\tconst wasRead = isRead, view = captureLibraryContext(), mangaId = manga.id, chapterId = chapter.id;"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "read mutation uses captured intended chapter state",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "\t\t\tmanga.id,\r\n\t\t\t\tisRead ? { unread_ids: [chapter.id] } : { read_ids: [chapter.id] }",
    "after": "\t\t\tmangaId,\r\n\t\t\t\twasRead ? { unread_ids: [chapterId] } : { read_ids: [chapterId] }"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "label": "read response updates only original current library context",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "before": "\t\t\tif (res?.status === StatusCodes.OK && res?.body) {\r\n\t\t\t\tif (isRead) {",
    "after": "\t\t\tassertLibraryContext(view);\r\n\t\t\tif (manga.id !== mangaId || chapter.id !== chapterId) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res?.body) {\r\n\t\t\t\tif (wasRead) {"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "read control captures intended state and library generation before update",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "\t\tisUpdatingReadHistory = true;",
    "after": "\t\tisUpdatingReadHistory = true;\r\n\t\tconst wasRead = isRead, view = captureLibraryContext(), mangaId = manga.id, chapterId = chapter.id;"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "read mutation uses captured intended chapter state",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "\t\t\tmanga.id,\r\n\t\t\t\tisRead ? { unread_ids: [chapter.id] } : { read_ids: [chapter.id] }",
    "after": "\t\t\tmangaId,\r\n\t\t\t\twasRead ? { unread_ids: [chapterId] } : { read_ids: [chapterId] }"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "label": "read response updates only original current library context",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "before": "\t\t\tif (res?.status === StatusCodes.OK && res?.body) {\r\n\t\t\t\tif (isRead) {",
    "after": "\t\t\tassertLibraryContext(view);\r\n\t\t\tif (manga.id !== mangaId || chapter.id !== chapterId) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res?.body) {\r\n\t\t\t\tif (wasRead) {"
  },
  {
    "path": "src/routes/(app)/title/Manga.svelte",
    "label": "title read history clears reloads and rejects late library ownership changes",
    "sha256": "960e709e68d2d0919580f8d38f058d45a5b4fcde66aed7cfe61ee699814920ff",
    "count": 1,
    "before": "\tconst getReadHistory = async () => {\r\n\t\tif (!isPublished) return;\r\n\t\tconst res = await API.ChapterHistory.List(manga.id);\r\n\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t$readHistory = res.body;\r\n\t\t}\r\n\t};",
    "after": "\tlet historyGeneration = 0, historyDisposed = false;\r\n\tonDestroy(() => { historyDisposed = true; historyGeneration++; readHistory.set(undefined); });\r\n\t$effect(() => {\r\n\t\t$libraryViewKey; manga.id; $libraryIdentity.canUse;\r\n\t\treadHistory.set(undefined); void getReadHistory();\r\n\t});\r\n\tconst getReadHistory = async () => {\r\n\t\tconst generation = ++historyGeneration, mangaId = manga.id, view = captureLibraryView();\r\n\t\tif (!isPublished || !$libraryIdentity.canUse || historyDisposed) return;\r\n\t\ttry {\r\n\t\t\tconst res = await API.ChapterHistory.List(mangaId);\r\n\t\t\tif (historyDisposed || generation !== historyGeneration || manga.id !== mangaId || captureLibraryView() !== view) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) readHistory.set(res.body);\r\n\t\t} catch (error) { if (!historyDisposed && generation === historyGeneration) readHistory.set(undefined); }\r\n\t};"
  },
  {
    "path": "src/routes/(app)/title/Manga.svelte",
    "label": "title history source context import",
    "sha256": "960e709e68d2d0919580f8d38f058d45a5b4fcde66aed7cfe61ee699814920ff",
    "count": 1,
    "before": "\timport { onMount, tick } from \"svelte\";",
    "after": "\timport { onDestroy, onMount, tick } from \"svelte\";\r\n\timport { libraryIdentity, libraryViewKey, captureLibraryView } from \"$localLibrary/library\";"
  },
  {
    "path": "src/routes/(app)/title/Manga.svelte",
    "label": "only reactive effect owns title private history reads",
    "sha256": "960e709e68d2d0919580f8d38f058d45a5b4fcde66aed7cfe61ee699814920ff",
    "count": 1,
    "before": "await Promise.all([getReadHistory(), getMangaStats()]);",
    "after": "await getMangaStats();"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "chapter feed cached public metadata never restores private history",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\t\t\t$readHistory = state._$ChapterFeed_history;",
    "after": "\t\t\t$readHistory = undefined;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "one reactive owner reloads chapter feed private flags without navigation cache",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\t\t\t\t\tif ($libraryIdentity.canUse && newIds.length) {\r\n\t\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\t\tnewIds.forEach(id => q.append(\"id\", id));\r\n\t\t\t\t\t\tAPI.ChapterHistory.Search(q)?.then(res => {\r\n\t\t\t\t\t\t\tif (window.location.pathname === pathname && res.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t\t\t$readHistory = { ...$readHistory, ...res.body };\r\n\t\t\t\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t\t\t\t_$ChapterFeed_history: $readHistory\r\n\t\t\t\t\t\t\t\t});\r\n\t\t\t\t\t\t\t}\r\n\t\t\t\t\t\t});\r\n\t\t\t\t\t}",
    "after": "\t\t\t\t\t// Private read flags are loaded by the selected-library effect below."
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "chapter feed private history has context and lifetime ownership",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "chapter feed history invalidates across all current source selections",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\tlet error = $state<Constants.Error.Error>();",
    "after": "\tlet error = $state<Constants.Error.Error>();\r\n\tlet historyGeneration = 0, historyDisposed = false;\r\n\tonDestroy(() => { historyDisposed = true; historyGeneration++; readHistory.set(undefined); });\r\n\t$effect(() => {\r\n\t\tconst view = $libraryViewKey, ids = entries.map(row => row.id), allowed = $libraryIdentity.canUse;\r\n\t\tconst generation = ++historyGeneration; readHistory.set(undefined);\r\n\t\tif (!allowed || !ids.length || historyDisposed) return;\r\n\t\tconst query = new URLSearchParams(); ids.forEach(id => query.append(\"id\", id));\r\n\t\tvoid API.ChapterHistory.Search(query).then(res => {\r\n\t\t\tif (!historyDisposed && generation === historyGeneration && captureLibraryView() === view && res?.status === StatusCodes.OK && res.body) readHistory.set(res.body);\r\n\t\t}).catch(() => { if (!historyDisposed && generation === historyGeneration) readHistory.set(undefined); });\r\n\t});"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "follow mutation captures stable selected library and target",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 2,
    "before": "\t\t\tconst { pathname } = window.location;\r\n\t\t\tisUpdatingReadingStatus = true;",
    "after": "\t\t\tconst { pathname } = window.location;\r\n\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tisUpdatingReadingStatus = true;"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "follow results cannot attach across selected authority or title",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 3,
    "before": "\t\t\t\tif (window.location.pathname !== pathname) {",
    "after": "\t\t\t\tassertLibraryContext(followContext);\r\n\t\t\t\tif (manga.id !== targetId || window.location.pathname !== pathname) {"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "matched account follow cache update [readingStatus, currentNotificationStatus ? 1 : 0]",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\t\t\t\t\tif ($Session && $libraryIdentity.mode === \"account\") {\r\n\t\t\t\t\t\tObject.assign(($Session.follows ??= {}), {\r\n\t\t\t\t\t\t\t[manga.id]: [readingStatus, currentNotificationStatus ? 1 : 0]\r\n\t\t\t\t\t\t});\r\n\t\t\t\t\t}",
    "after": "\t\t\t\t\tapplyAccountFollow(followContext, targetId, [readingStatus, currentNotificationStatus ? 1 : 0]);"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "matched account follow cache update [currentReadingStatus, notificationStatus ? 1 : 0]",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\t\t\t\t\tif ($Session && $libraryIdentity.mode === \"account\") {\r\n\t\t\t\t\t\tObject.assign(($Session.follows ??= {}), {\r\n\t\t\t\t\t\t\t[manga.id]: [currentReadingStatus, notificationStatus ? 1 : 0]\r\n\t\t\t\t\t\t});\r\n\t\t\t\t\t}",
    "after": "\t\t\t\t\tapplyAccountFollow(followContext, targetId, [currentReadingStatus, notificationStatus ? 1 : 0]);"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "matched account unfollow cache only",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\t\t\t\t\tif ($libraryIdentity.mode === \"account\" && $Session?.follows && manga.id in $Session.follows) {\r\n\t\t\t\t\t\tdelete $Session.follows[manga.id];\r\n\t\t\t\t\t}",
    "after": "\t\t\t\t\tapplyAccountFollow(followContext, targetId, undefined);"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "search follow captures immutable target and selected library",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t\tconst res = await API.MangaFollow.Update(manga.id, {",
    "after": "\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tconst res = await API.MangaFollow.Update(targetId, {"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "search unfollow captures immutable target and selected library",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t\tconst res = await API.MangaFollow.Unfollow(manga.id);",
    "after": "\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tconst res = await API.MangaFollow.Unfollow(targetId);"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "search follow result ownership fence",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 2,
    "before": "\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {",
    "after": "\t\t\tassertLibraryContext(followContext);\r\n\t\t\tif (manga.id !== targetId) return;\r\n\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "search matched account follow cache only",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t\t\tif ($Session && $libraryIdentity.mode === \"account\") {\r\n\t\t\t\t\tObject.assign(($Session.follows ??= {}), {\r\n\t\t\t\t\t\t[manga.id]: [Constants.Manga.ReadingStatus.Reading, 1]\r\n\t\t\t\t\t});\r\n\t\t\t\t}",
    "after": "\t\t\t\tapplyAccountFollow(followContext, targetId, [Constants.Manga.ReadingStatus.Reading, 1]);"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "label": "search matched account unfollow cache only",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "before": "\t\t\t\tif ($libraryIdentity.mode === \"account\" && $Session?.follows && manga.id in $Session.follows) {\r\n\t\t\t\t\tdelete $Session.follows[manga.id];\r\n\t\t\t\t}",
    "after": "\t\t\t\tapplyAccountFollow(followContext, targetId, undefined);"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "notification mutation captures stable selected library and target",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\t\t\tconst { pathname } = window.location;\r\n\t\t\tisUpdatingNotificationStatus = true;",
    "after": "\t\t\tconst { pathname } = window.location;\r\n\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tisUpdatingNotificationStatus = true;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 feed continuation imports ownership and DOM-settle helpers",
    "before": "import { libraryIdentity, libraryViewKey, captureLibraryView } from \"$localLibrary/library\";",
    "after": "import { libraryIdentity, libraryViewKey, captureLibraryView, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";\r\n\timport { createFeedContinuation } from \"$platform/feed-continuation\";\r\n\timport { captureNavigationEpoch, sameNavigationEpoch } from \"$platform/navigation-authority\";\r\n\timport { Button } from \"$lib/components/ui/button\";"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 DOM layout settles before the next visible-footer read",
    "before": "import { onDestroy, onMount } from \"svelte\";",
    "after": "import { onDestroy, onMount, tick } from \"svelte\";"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 feed read lifetime starts before its request",
    "before": "\tlet historyGeneration = 0, historyDisposed = false;",
    "after": "\tlet historyGeneration = 0, historyDisposed = false, feedGeneration = 0;\r\n\tlet paginationPaused = $state(false), paginationFailed = $state(false);\r\n\tlet continuation: ReturnType<typeof createFeedContinuation> | undefined;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 destroyed feed cancels continuation and invalidates safe read attachment",
    "before": "onDestroy(() => { historyDisposed = true; historyGeneration++; readHistory.set(undefined); });",
    "after": "onDestroy(() => { historyDisposed = true; historyGeneration++; feedGeneration++; continuation?.dispose(); readHistory.set(undefined); });"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 initial read does not mount a disposed feed",
    "before": "\t\tisMounted = true;",
    "after": "\t\tif (!historyDisposed) isMounted = true;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 feed captures route/library/request ownership before yielding",
    "before": "\t\tif (isLoading) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst { pathname } = window.location;\r\n\t\tisLoading = true;",
    "after": "\t\tif (isLoading || historyDisposed) return false;\r\n\t\tconst { pathname } = window.location, href = window.location.href, search = onSearch;\r\n\t\tconst context = captureLibraryContext(), navigation = captureNavigationEpoch(), generation = ++feedGeneration;\r\n\t\tconst owned = () => {\r\n\t\t\tif (historyDisposed || generation !== feedGeneration || href !== window.location.href || search !== onSearch || !sameNavigationEpoch(navigation)) return false;\r\n\t\t\ttry { assertLibraryContext(context); return true; } catch { return false; }\r\n\t\t};\r\n\t\tisLoading = true;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 reject stale or nonadvancing feed response before appending",
    "before": "\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tfeed = res.body;",
    "after": "\t\t\tif (!owned()) return false;\r\n\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tconst next = res.body;\r\n\t\t\t\tif (!Number.isSafeInteger(next.page) || next.page < 1 || !Number.isSafeInteger(next.limit) || next.limit < 1 || !Number.isSafeInteger(next.total) || next.total < 0 || !Array.isArray(next.data) || (n !== undefined && next.page !== n)) throw new Constants.Error.Error(\"Feed pagination stopped\", \"The server returned an invalid or nonadvancing page. Existing updates remain available; retry only after checking the current feed.\");\r\n\t\t\t\tif (feed && n !== undefined && next.data.length) {\r\n\t\t\t\t\tconst known = new Set(entries.flatMap(manga => manga.relationships._chapters?.map(chapter => chapter.id) ?? []));\r\n\t\t\t\t\tif (next.data.every(chapter => known.has(chapter.id))) throw new Constants.Error.Error(\"Feed pagination stopped\", \"The next page repeated only existing chapters. No automatic requests will continue; retry or reload the current feed.\");\r\n\t\t\t\t}\r\n\t\t\t\tfeed = next;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 successful page explicitly allows one bounded next read",
    "before": "\t\t\t\t\t// Private read flags are loaded by the selected-library effect below.\r\n\t\t\t\t}\r\n\t\t\t} else if",
    "after": "\t\t\t\t\t// Private read flags are loaded by the selected-library effect below.\r\n\t\t\t\t}\r\n\t\t\t\treturn true;\r\n\t\t\t} else if"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 stale safe-read errors have no UI side effects",
    "before": "\t\t} catch (e) {\r\n\t\t\tif (isMounted) {",
    "after": "\t\t} catch (e) {\r\n\t\t\tif (!owned()) return false;\r\n\t\t\tif (isMounted) {"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 bounded continuation consumes truthful failure and final loading state",
    "before": "\t\t} finally {\r\n\t\t\tisLoading = false;\r\n\t\t}\r\n\t};",
    "after": "\t\t\treturn false;\r\n\t\t} finally {\r\n\t\t\tif (generation === feedGeneration && !historyDisposed) isLoading = false;\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 sequential visible-footer continuation with explicit bounded resume",
    "before": "\t$effect(() => {\r\n\t\tif (!footerRef) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst observer = new IntersectionObserver(\r\n\t\t\tasync ([entry]) => {\r\n\t\t\t\tif (!entry.isIntersecting || !feed || isLoading) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tawait load(feed.page + 1);\r\n\t\t\t\tif (!feed.data?.length) {\r\n\t\t\t\t\tif (footerRef) {\r\n\t\t\t\t\t\tobserver.unobserve(footerRef);\r\n\t\t\t\t\t}\r\n\t\t\t\t\tobserver.disconnect();\r\n\t\t\t\t}\r\n\t\t\t},\r\n\t\t\t{ rootMargin: window.innerHeight + \"px\" }\r\n\t\t);\r\n\t\tobserver.observe(footerRef);\r\n\t\treturn () => {\r\n\t\t\tif (footerRef) observer.unobserve(footerRef);\r\n\t\t\tobserver.disconnect();\r\n\t\t};\r\n\t});",
    "after": "\t$effect(() => {\r\n\t\tconst target = footerRef;\r\n\t\tif (!target || isEntity) return;\r\n\t\tcontinuation = createFeedContinuation({\r\n\t\t\ttarget, isAlive: () => !historyDisposed && footerRef === target,\r\n\t\t\tgetFeed: () => feed, loadPage: load, settle: tick,\r\n\t\t\tcaptureContext: () => { const context = captureLibraryContext(), navigation = captureNavigationEpoch(), href = window.location.href, search = onSearch; return () => { if (historyDisposed || footerRef !== target || href !== window.location.href || onSearch !== search || !sameNavigationEpoch(navigation)) return false; try { assertLibraryContext(context); return true; } catch { return false; } }; },\r\n\t\t\tpause: (paused, failed) => { if (!historyDisposed) { paginationPaused = paused; paginationFailed = failed; } }\r\n\t\t});\r\n\t\tconst owner = continuation;\r\n\t\treturn () => { owner.dispose(); if (continuation === owner) continuation = undefined; };\r\n\t});"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI12 approved nonentity pagination resume control",
    "before": "\t\t\t\t\t{/if}\r\n\t\t\t\t</footer>",
    "after": "\t\t\t\t\t{/if}\r\n\t\t\t\t\t{#if paginationPaused}\r\n\t\t\t\t\t\t<Button variant=\"outline\" class=\"w-full\" disabled={isLoading} onclick={() => continuation?.resume()}>{paginationFailed ? \"Retry loading updates\" : \"Load more updates\"}</Button>\r\n\t\t\t\t\t{/if}\r\n\t\t\t\t</footer>",
    "approved_feed_pagination_exception": "bounded_continuation_resume"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 history cache records its route and selected-library context",
    "before": "\t\t\t_$ChapterFeed_entries: typeof entries;",
    "after": "\t\t\t_$ChapterFeed_entries: typeof entries;\r\n\t\t\t_$ChapterFeed_context?: string;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 only restore a coherent current-context accumulated feed",
    "before": "if (![\"/library\", \"/history\"].some(prefix => window.location.pathname.startsWith(prefix)) && state._$ChapterFeed_feed && state._$ChapterFeed_entries) {",
    "after": "if (![\"/library\", \"/history\"].some(prefix => window.location.pathname.startsWith(prefix)) && state._$ChapterFeed_feed && state._$ChapterFeed_entries && state._$ChapterFeed_context === (() => { const context = captureLibraryContext(); return JSON.stringify([window.location.href, context.generation, context.key]); })()) {"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 inconsistent empty-before-end stops before state/history commit",
    "before": "\t\t\t\tfeed = next;\r\n\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t...page.state,\r\n\t\t\t\t\t_$ChapterFeed_feed: $state.snapshot(feed)\r\n\t\t\t\t});",
    "after": "\t\t\t\tif (!next.data.length && next.limit * (next.page - 1) < next.total) throw new Constants.Error.Error(\"Feed pagination stopped\", \"The server returned an empty page before its reported end. Previous acknowledged updates are retained; retry or reload the current feed.\");\r\n\t\t\t\tfeed = next;\r\n\t\t\t\t// Feed and accumulated entries are committed together only after append."
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 history stores accumulated acknowledgements with coherent map and context",
    "before": "\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$ChapterFeed_entries: $state.snapshot(entries)\r\n\t\t\t\t\t});",
    "after": "\t\t\t\t\tconst acknowledgedEntries = $state.snapshot(entries);\r\n\t\t\t\t\tconst acknowledgedManga: Record<string, Manga> = {}, acknowledgedChapters = new Map<string, Chapter>();\r\n\t\t\t\t\tfor (const manga of acknowledgedEntries) {\r\n\t\t\t\t\t\tconst previous = acknowledgedManga[manga.id], chapters = new Map<string, Chapter>();\r\n\t\t\t\t\t\tfor (const chapter of [...(previous?.relationships._chapters ?? []), ...(manga.relationships._chapters ?? [])]) { chapters.set(chapter.id, chapter); acknowledgedChapters.set(chapter.id, chapter); }\r\n\t\t\t\t\t\tacknowledgedManga[manga.id] = { ...manga, relationships: { ...manga.relationships, _chapters: [...chapters.values()] } };\r\n\t\t\t\t\t}\r\n\t\t\t\t\treplaceState(href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$ChapterFeed_feed: { ...$state.snapshot(feed), data: [...acknowledgedChapters.values()], map: { ...$state.snapshot(feed.map), manga: acknowledgedManga } },\r\n\t\t\t\t\t\t_$ChapterFeed_entries: acknowledgedEntries,\r\n\t\t\t\t\t\t_$ChapterFeed_context: JSON.stringify([href, context.generation, context.key])\r\n\t\t\t\t\t});"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 approved accumulated-results guard retains acknowledged cards after terminal empty",
    "before": "\t{#if feed.data?.length}",
    "after": "\t{#if entries.length}",
    "approved_feed_pagination_exception": "accumulated_acknowledged_results_guard"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 private read readiness and stable authority subscriptions",
    "before": "import { libraryIdentity, libraryViewKey, captureLibraryView, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";",
    "after": "import { libraryIdentity, libraryViewKey, localLibraryState, captureLibraryView, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";\r\n\timport { nativeAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 classify personal feeds without binding public updates to local discovery",
    "before": "\tconst isEntity =",
    "after": "\tconst isPrivateFeed = [\"/library\", \"/history\"].some(prefix => window.location.pathname.startsWith(prefix));\r\n\tconst isEntity ="
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 destroyed private read wakes its coalesced drain without new requests",
    "before": "onDestroy(() => { historyDisposed = true; historyGeneration++; feedGeneration++; continuation?.dispose(); readHistory.set(undefined); });",
    "after": "onDestroy(() => { historyDisposed = true; historyGeneration++; feedGeneration++; pendingPrivateRead = undefined; releaseReadWaiters(); continuation?.dispose(); readHistory.set(undefined); });"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 private initial load is driven by current readiness instead of stale mount admission",
    "before": "\tonMount(async () => {\r\n\t\tconst state =",
    "after": "\tonMount(async () => {\r\n\t\tif (isPrivateFeed) { isMounted = true; return; }\r\n\t\tconst initialHref = window.location.href;\r\n\t\tconst state ="
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 public history cache uses its actual route instead of unrelated library scope",
    "before": "state._$ChapterFeed_context === (() => { const context = captureLibraryContext(); return JSON.stringify([window.location.href, context.generation, context.key]); })()",
    "after": "state._$ChapterFeed_context === window.location.href"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 an interrupted initial public read exposes recovery instead of a spinner",
    "before": "\t\tif (!historyDisposed) isMounted = true;",
    "after": "\t\tif (!historyDisposed) {\r\n\t\t\tif (!feed && !error && window.location.href === initialHref) error = new Constants.Error.Error(\"Feed load interrupted\", \"The previous route request no longer belongs to the current view. Retry the current feed.\");\r\n\t\t\tisMounted = true;\r\n\t\t}"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 each private request captures view revision; public safe reads own only route and query callback",
    "before": "\t\tconst context = captureLibraryContext(), navigation = captureNavigationEpoch(), generation = ++feedGeneration;",
    "after": "\t\tconst context = isPrivateFeed ? captureLibraryContext() : undefined, view = isPrivateFeed ? captureLibraryView() : undefined;\r\n\t\tconst navigation = captureNavigationEpoch(), generation = ++feedGeneration;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 public response cannot be discarded by independent scope or Session initialization",
    "before": "\t\t\ttry { assertLibraryContext(context); return true; } catch { return false; }",
    "after": "\t\t\tif (context) { try { assertLibraryContext(context); if (captureLibraryView() !== view) return false; } catch { return false; } }\r\n\t\t\treturn true;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 personal feed DTOs never enter navigation history cache",
    "before": "\t\t\t\t\treplaceState(href, {\r\n",
    "after": "\t\t\t\t\tif (!isPrivateFeed) replaceState(href, {\r\n"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 accumulated public cache matches current actual URL",
    "before": "_$ChapterFeed_context: JSON.stringify([href, context.generation, context.key])",
    "after": "_$ChapterFeed_context: href"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 read settlement releases only sequential owned refreshes",
    "before": "\t\t\tif (generation === feedGeneration && !historyDisposed) isLoading = false;",
    "after": "\t\t\tif (generation === feedGeneration && !historyDisposed) isLoading = false;\r\n\t\t\treleaseReadWaiters();"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 one latest-context private read coalesces initialization and authority switches",
    "before": "\t$effect(() => {\r\n\t\tconst target = footerRef;",
    "after": "\tlet privateRequestKey = \"\", privateSearch: typeof onSearch | undefined, privateDraining = false;\r\n\tlet pendingPrivateRead: { href: string; navigation: number; search: typeof onSearch } | undefined;\r\n\tconst readWaiters = new Set<() => void>();\r\n\tfunction releaseReadWaiters() { for (const resolve of readWaiters) resolve(); readWaiters.clear(); }\r\n\tconst waitForReadIdle = () => isLoading ? new Promise<void>(resolve => readWaiters.add(resolve)) : Promise.resolve();\r\n\tasync function drainPrivateReads() {\r\n\t\tif (privateDraining || historyDisposed) return;\r\n\t\tprivateDraining = true;\r\n\t\ttry {\r\n\t\t\twhile (pendingPrivateRead && !historyDisposed) {\r\n\t\t\t\tawait waitForReadIdle();\r\n\t\t\t\tif (historyDisposed) return;\r\n\t\t\t\tconst request = pendingPrivateRead; pendingPrivateRead = undefined;\r\n\t\t\t\tif (!request || request.href !== window.location.href || request.search !== onSearch || !sameNavigationEpoch(request.navigation)) continue;\r\n\t\t\t\tconst value = new URL(request.href).searchParams.get(\"page\");\r\n\t\t\t\tawait load(value ? Number(value) : undefined);\r\n\t\t\t\tif (!historyDisposed && !pendingPrivateRead && !feed && !error && request.href === window.location.href) error = new Constants.Error.Error(\"Feed load interrupted\", \"The personal feed changed while loading. Retry the currently selected library; no previous account rows are displayed.\");\r\n\t\t\t}\r\n\t\t} finally { privateDraining = false; }\r\n\t}\r\n\tfunction queuePrivateRead() {\r\n\t\tif (historyDisposed) return;\r\n\t\tpendingPrivateRead = { href: window.location.href, navigation: captureNavigationEpoch(), search: onSearch };\r\n\t\t// Clear old personal DTOs synchronously, before any latest-context read.\r\n\t\tfeed = undefined; entries = []; idMap = {}; readHistory.set(undefined); error = undefined; paginationPaused = false;\r\n\t\tvoid Promise.resolve().then(drainPrivateReads);\r\n\t}\r\n\t$effect.pre(() => {\r\n\t\tif (!isPrivateFeed || !isMounted || !$localLibraryState.ready || historyDisposed) return;\r\n\t\tconst view = $libraryViewKey, authority = $nativeAuthority, href = page.url.href, search = onSearch;\r\n\t\tconst context = captureLibraryContext(), key = JSON.stringify([view, authority.actor, authority.epoch, context.generation, context.key, href]);\r\n\t\tif (key === privateRequestKey && search === privateSearch) return;\r\n\t\tprivateRequestKey = key; privateSearch = search; queuePrivateRead();\r\n\t});\r\n\tasync function retryFeed() {\r\n\t\tif (historyDisposed || isLoading) return;\r\n\t\tif (isPrivateFeed) { queuePrivateRead(); return; }\r\n\t\tconst href = window.location.href; error = undefined;\r\n\t\tconst value = new URL(href).searchParams.get(\"page\");\r\n\t\tawait load(value ? Number(value) : undefined);\r\n\t\tif (!historyDisposed && !feed && !error && href === window.location.href) error = new Constants.Error.Error(\"Feed load interrupted\", \"The route changed while loading. Retry the current feed.\");\r\n\t}\r\n\r\n\t$effect(() => {\r\n\t\tconst target = footerRef;"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 continuation public/private context classification",
    "before": "captureContext: () => { const context = captureLibraryContext(), navigation = captureNavigationEpoch(), href = window.location.href, search = onSearch; return () => { if (historyDisposed || footerRef !== target || href !== window.location.href || onSearch !== search || !sameNavigationEpoch(navigation)) return false; try { assertLibraryContext(context); return true; } catch { return false; } }; },",
    "after": "captureContext: () => { const context = isPrivateFeed ? captureLibraryContext() : undefined, view = isPrivateFeed ? captureLibraryView() : undefined, navigation = captureNavigationEpoch(), href = window.location.href, search = onSearch; return () => { if (historyDisposed || footerRef !== target || href !== window.location.href || onSearch !== search || !sameNavigationEpoch(navigation)) return false; if (context) { try { assertLibraryContext(context); if (captureLibraryView() !== view) return false; } catch { return false; } } return true; }; },"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "label": "UI13 approved explicit initial-feed retry in existing nonentity error branch",
    "before": "\t<ErrorComponent {error} />",
    "after": "\t<ErrorComponent {error} />\r\n\t{#if !isEntity}\r\n\t\t<Button variant=\"outline\" disabled={isLoading} onclick={retryFeed}>Retry feed</Button>\r\n\t{/if}",
    "approved_feed_pagination_exception": "initial_feed_retry_control"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "UI13 initial and retry errors keep their actual error surface",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\t\t\tif (isMounted) {",
    "after": "\t\t\tif (isMounted && feed) {"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "UI14 approved entity paginator remains on actual final page",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\t\t{#if feed && feed.limit * feed.page < feed.total}",
    "after": "\t\t{#if feed && (isEntity ? (feed.total > feed.limit || feed.page > 1) : feed.limit * feed.page < feed.total)}",
    "approved_feed_pagination_exception": "entity_last_page_pagination_guard"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "label": "UI14 approved unchanged entity paginator also follows empty-state results",
    "sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "count": 1,
    "before": "\t\t</Empty.Root>\r\n\t{/if}\r\n{:else}",
    "after": "\t\t</Empty.Root>\r\n\t{/if}\r\n\t{#if isEntity && !entries.length && (feed.total > feed.limit || feed.page > 1)}\r\n\t\t<footer>\r\n\t\t\t<GenericPagination {feed} />\r\n\t\t</footer>\r\n\t{/if}\r\n{:else}",
    "approved_feed_pagination_exception": "entity_empty_page_existing_paginator"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "label": "UI14 ChapterFeedChapterEntry shared owned action and truthful mutation feedback",
    "before": "import { libraryIdentity, localLibraryState, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";",
    "after": "import { libraryIdentity, localLibraryState, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";\r\n\timport { createLibraryActionOwner } from \"$localLibrary/action-owner\";\r\n\timport { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "label": "UI14 ChapterFeedChapterEntry chapter lifetime belongs to the original read action",
    "before": "\tlet isUpdatingReadHistory = $state(false);",
    "after": "\tlet isUpdatingReadHistory = $state(false);\r\n\tconst readActionOwner = createLibraryActionOwner(() => JSON.stringify([manga.id, chapter.id]));"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "count": 1,
    "label": "UI14 ChapterFeedChapterEntry current actor gets real rejection uncertainty or storage failure",
    "before": "\tconst updateReadHistory = async () => {\r\n\t\tif (isUpdatingReadHistory) return;\r\n\t\tisUpdatingReadHistory = true;\r\n\t\tconst wasRead = isRead, view = captureLibraryContext(), mangaId = manga.id, chapterId = chapter.id;\r\n\r\n\t\ttry {\r\n\t\t\tconst res = await API.ChapterHistory.UpdateBatch(\r\n\t\t\t\tmangaId,\r\n\t\t\t\twasRead ? { unread_ids: [chapterId] } : { read_ids: [chapterId] }\r\n\t\t\t);\r\n\t\t\tassertLibraryContext(view);\r\n\t\t\tif (manga.id !== mangaId || chapter.id !== chapterId) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res?.body) {\r\n\t\t\t\tif (wasRead) {\r\n\t\t\t\t\tif ($readHistory && manga.id in $readHistory) {\r\n\t\t\t\t\t\tdelete $readHistory[manga.id][chapter.id];\r\n\t\t\t\t\t}\r\n\t\t\t\t} else {\r\n\t\t\t\t\tif (!$readHistory) $readHistory = {};\r\n\t\t\t\t\t($readHistory[manga.id] ??= {})[chapter.id] = null;\r\n\t\t\t\t}\r\n\t\t\t\t$readHistory = $readHistory;\r\n\t\t\t}\r\n\t\t} finally {\r\n\t\t\tisUpdatingReadHistory = false;\r\n\t\t}\r\n\t};",
    "after": "\tconst updateReadHistory = async () => {\r\n\t\tif (isUpdatingReadHistory) return;\r\n\t\tconst wasRead = isRead, view = captureLibraryContext(), mangaId = manga.id, chapterId = chapter.id;\r\n\t\tlet intent: ReturnType<typeof readActionOwner.capture> | undefined;\r\n\t\tisUpdatingReadHistory = true;\r\n\t\ttry {\r\n\t\t\tintent = readActionOwner.capture(); intent.assert();\r\n\t\t\tconst res = await API.ChapterHistory.UpdateBatch(\r\n\t\t\t\tmangaId,\r\n\t\t\t\twasRead ? { unread_ids: [chapterId] } : { read_ids: [chapterId] },\r\n\t\t\t\t{ context: view, assert: intent.assert }\r\n\t\t\t);\r\n\t\t\tintent.assert(); assertLibraryContext(view);\r\n\t\t\trequireMutationAcknowledgement(res, StatusCodes.OK, true);\r\n\t\t\tif (wasRead) {\r\n\t\t\t\tif ($readHistory && mangaId in $readHistory) delete $readHistory[mangaId][chapterId];\r\n\t\t\t} else {\r\n\t\t\t\tif (!$readHistory) $readHistory = {};\r\n\t\t\t\t($readHistory[mangaId] ??= {})[chapterId] = null;\r\n\t\t\t}\r\n\t\t\t$readHistory = $readHistory;\r\n\t\t} catch (error) {\r\n\t\t\treportCurrentMutationFailure(\"Read history update failed\", error, () => !!intent?.alive());\r\n\t\t} finally {\r\n\t\t\tisUpdatingReadHistory = false;\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "label": "UI14 ChapterListEntry shared owned action and truthful mutation feedback",
    "before": "import { libraryIdentity, localLibraryState, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";",
    "after": "import { libraryIdentity, localLibraryState, captureLibraryContext, assertLibraryContext } from \"$localLibrary/library\";\r\n\timport { createLibraryActionOwner } from \"$localLibrary/action-owner\";\r\n\timport { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "label": "UI14 ChapterListEntry chapter lifetime belongs to the original read action",
    "before": "\tlet isUpdatingReadHistory = $state(false);",
    "after": "\tlet isUpdatingReadHistory = $state(false);\r\n\tconst readActionOwner = createLibraryActionOwner(() => JSON.stringify([manga.id, chapter.id]));"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "count": 1,
    "label": "UI14 ChapterListEntry current actor gets real rejection uncertainty or storage failure",
    "before": "\tconst updateReadHistory = async () => {\r\n\t\tif (isUpdatingReadHistory) return;\r\n\t\tisUpdatingReadHistory = true;\r\n\t\tconst wasRead = isRead, view = captureLibraryContext(), mangaId = manga.id, chapterId = chapter.id;\r\n\r\n\t\ttry {\r\n\t\t\tconst res = await API.ChapterHistory.UpdateBatch(\r\n\t\t\t\tmangaId,\r\n\t\t\t\twasRead ? { unread_ids: [chapterId] } : { read_ids: [chapterId] }\r\n\t\t\t);\r\n\t\t\tassertLibraryContext(view);\r\n\t\t\tif (manga.id !== mangaId || chapter.id !== chapterId) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res?.body) {\r\n\t\t\t\tif (wasRead) {\r\n\t\t\t\t\tif ($readHistory && manga.id in $readHistory) {\r\n\t\t\t\t\t\tdelete $readHistory[manga.id][chapter.id];\r\n\t\t\t\t\t}\r\n\t\t\t\t} else {\r\n\t\t\t\t\tif (!$readHistory) $readHistory = {};\r\n\t\t\t\t\t($readHistory[manga.id] ??= {})[chapter.id] = null;\r\n\t\t\t\t}\r\n\t\t\t\t$readHistory = $readHistory;\r\n\t\t\t}\r\n\t\t} finally {\r\n\t\t\tisUpdatingReadHistory = false;\r\n\t\t}\r\n\t};",
    "after": "\tconst updateReadHistory = async () => {\r\n\t\tif (isUpdatingReadHistory) return;\r\n\t\tconst wasRead = isRead, view = captureLibraryContext(), mangaId = manga.id, chapterId = chapter.id;\r\n\t\tlet intent: ReturnType<typeof readActionOwner.capture> | undefined;\r\n\t\tisUpdatingReadHistory = true;\r\n\t\ttry {\r\n\t\t\tintent = readActionOwner.capture(); intent.assert();\r\n\t\t\tconst res = await API.ChapterHistory.UpdateBatch(\r\n\t\t\t\tmangaId,\r\n\t\t\t\twasRead ? { unread_ids: [chapterId] } : { read_ids: [chapterId] },\r\n\t\t\t\t{ context: view, assert: intent.assert }\r\n\t\t\t);\r\n\t\t\tintent.assert(); assertLibraryContext(view);\r\n\t\t\trequireMutationAcknowledgement(res, StatusCodes.OK, true);\r\n\t\t\tif (wasRead) {\r\n\t\t\t\tif ($readHistory && mangaId in $readHistory) delete $readHistory[mangaId][chapterId];\r\n\t\t\t} else {\r\n\t\t\t\tif (!$readHistory) $readHistory = {};\r\n\t\t\t\t($readHistory[mangaId] ??= {})[chapterId] = null;\r\n\t\t\t}\r\n\t\t\t$readHistory = $readHistory;\r\n\t\t} catch (error) {\r\n\t\t\treportCurrentMutationFailure(\"Read history update failed\", error, () => !!intent?.alive());\r\n\t\t} finally {\r\n\t\t\tisUpdatingReadHistory = false;\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "label": "UI14 card follows share owned acknowledgement feedback",
    "before": "import { libraryIdentity, localLibraryState, openLibrary, message, captureLibraryContext, assertLibraryContext, applyAccountFollow } from \"$localLibrary/library\";",
    "after": "import { libraryIdentity, localLibraryState, openLibrary, message, captureLibraryContext, assertLibraryContext, applyAccountFollow } from \"$localLibrary/library\";\r\n\timport { createLibraryActionOwner } from \"$localLibrary/action-owner\";\r\n\timport { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "label": "UI14 card action captures component target lifetime",
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst followActionOwner = createLibraryActionOwner(() => manga.id);"
  },
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "count": 1,
    "label": "UI14 card follow or unfollow rejection is visible only to its owned intent",
    "before": "\tconst follow = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!$libraryIdentity.canUse) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\ttry {\r\n\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tconst res = await API.MangaFollow.Update(targetId, {\r\n\t\t\t\tstatus: Constants.Manga.ReadingStatus.Reading,\r\n\t\t\t\tnotification: true\r\n\t\t\t});\r\n\t\t\tassertLibraryContext(followContext);\r\n\t\t\tif (manga.id !== targetId) return;\r\n\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\tisFollowed = true;\r\n\t\t\t\tapplyAccountFollow(followContext, targetId, [Constants.Manga.ReadingStatus.Reading, 1]);\r\n\t\t\t}\r\n\t\t\t} catch (error) { window.dispatchEvent(new CustomEvent(\"weebdex-library-error\", { detail: message(error) })); }\r\n\t\t});\r\n\r\n\tconst unfollow = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!$libraryIdentity.canUse) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\ttry {\r\n\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tconst res = await API.MangaFollow.Unfollow(targetId);\r\n\t\t\tassertLibraryContext(followContext);\r\n\t\t\tif (manga.id !== targetId) return;\r\n\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\tisFollowed = false;\r\n\t\t\t\tapplyAccountFollow(followContext, targetId, undefined);\r\n\t\t\t}\r\n\t\t\t} catch (error) { window.dispatchEvent(new CustomEvent(\"weebdex-library-error\", { detail: message(error) })); }\r\n\t\t});",
    "after": "\tconst changeFollowing = (followed: boolean) => {\r\n\t\tif (!$libraryIdentity.canUse) return;\r\n\t\tconst context = captureLibraryContext(), targetId = manga.id, intent = followActionOwner.capture();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tconst admitted = { context, assert: intent.assert };\r\n\t\t\t\tconst res = followed\r\n\t\t\t\t\t? await API.MangaFollow.Update(targetId, { status: Constants.Manga.ReadingStatus.Reading, notification: true }, admitted)\r\n\t\t\t\t\t: await API.MangaFollow.Unfollow(targetId, admitted);\r\n\t\t\t\tintent.assert(); requireMutationAcknowledgement(res, StatusCodes.NO_CONTENT);\r\n\t\t\t\tisFollowed = followed;\r\n\t\t\t\tapplyAccountFollow(context, targetId, followed ? [Constants.Manga.ReadingStatus.Reading, 1] : undefined);\r\n\t\t\t} catch (error) {\r\n\t\t\t\treportCurrentMutationFailure(\"Library update failed\", error, intent.alive);\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\tconst follow = () => changeFollowing(true);\r\n\tconst unfollow = () => changeFollowing(false);"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "label": "UI14 title unfollow uses shared truthful owned feedback",
    "before": "import { libraryIdentity, localLibraryState, openLibrary, captureLibraryContext, assertLibraryContext, applyAccountFollow } from \"$localLibrary/library\";",
    "after": "import { libraryIdentity, localLibraryState, openLibrary, captureLibraryContext, assertLibraryContext, applyAccountFollow } from \"$localLibrary/library\";\r\n\timport { createLibraryActionOwner } from \"$localLibrary/action-owner\";\r\n\timport { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "label": "UI14 title unfollow target lifetime is owned",
    "before": "\tconst manga = $derived(getManga()());",
    "after": "\tconst manga = $derived(getManga()());\r\n\tconst unfollowActionOwner = createLibraryActionOwner(() => manga.id);"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "label": "UI14 title unfollow catches actual rejected acknowledgement and uncertain outcome",
    "before": "\tconst unfollow = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!$libraryIdentity.canUse || !isPublished) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst { pathname } = window.location;\r\n\t\t\tconst followContext = captureLibraryContext(), targetId = manga.id;\r\n\t\t\tisUpdatingReadingStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.MangaFollow.Unfollow(manga.id);\r\n\t\t\t\tassertLibraryContext(followContext);\r\n\t\t\t\tif (manga.id !== targetId || window.location.pathname !== pathname) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tcurrentReadingStatus = undefined;\r\n\t\t\t\t\tcurrentNotificationStatus = undefined;\r\n\t\t\t\t\tapplyAccountFollow(followContext, targetId, undefined);\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisUpdatingReadingStatus = false;\r\n\t\t\t}\r\n\t\t});\r\n",
    "after": "\tconst unfollow = () => {\r\n\t\tif (!$libraryIdentity.canUse || !isPublished) return;\r\n\t\tconst context = captureLibraryContext(), targetId = manga.id, href = window.location.href, intent = unfollowActionOwner.capture();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tisUpdatingReadingStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tconst res = await API.MangaFollow.Unfollow(targetId, { context, assert: intent.assert });\r\n\t\t\t\tintent.assert(); if (href !== window.location.href) return;\r\n\t\t\t\trequireMutationAcknowledgement(res, StatusCodes.NO_CONTENT);\r\n\t\t\t\tcurrentReadingStatus = undefined; currentNotificationStatus = undefined;\r\n\t\t\t\tapplyAccountFollow(context, targetId, undefined);\r\n\t\t\t} catch (error) {\r\n\t\t\t\treportCurrentMutationFailure(\"Library update failed\", error, () => intent.alive() && href === window.location.href);\r\n\t\t\t} finally { isUpdatingReadingStatus = false; }\r\n\t\t});\r\n\t};\r\n"
  },
  {
    "path": "src/lib/components/LatestUpdatesGrid.svelte",
    "sha256": "a8c52cc549ac4d7b32521104fdbeb27f53d61f55323bc047365f2056e2ebe481",
    "count": 1,
    "label": "UI14 mounted public grid owns real response errors and explicit safe recovery",
    "before": "<script lang=\"ts\">\r\n\timport { replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { API, Constants } from \"$lib\";\r\n\timport Grid from \"$lib/components/ui/Grid.svelte\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport ErrorComponent from \"./ErrorComponent.svelte\";\r\n\timport LatestUpdatesGridEntry from \"./LatestUpdatesGridEntry.svelte\";\r\n\timport LatestUpdatesGridSkeleton from \"./LatestUpdatesGridSkeleton.svelte\";\r\n\timport MangaSearchEntryCard from \"./MangaSearchEntryCard.svelte\";\r\n\timport MangaSearchEntrySkeleton from \"./MangaSearchEntrySkeleton.svelte\";\r\n\r\n\tlet feed = $state<ChapterFeedResponse>();\r\n\tlet entries = $state<Chapter[]>([]);\r\n\tlet idMap = $state<Record<string, undefined>>({});\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet isLoading = $state(false);\r\n\tlet isCollapsed = $state(false);\r\n\tlet footerRef = $state<HTMLElement>();\r\n\tlet error = $state<Constants.Error.Error>();\r\n\r\n\tonMount(async () => {\r\n\t\tconst state = page.state as {\r\n\t\t\t_$LatestUpdatesGrid_feed: typeof feed;\r\n\t\t\t_$LatestUpdatesGrid_entries: typeof entries;\r\n\t\t\t_$LatestUpdatesGrid_idMap: typeof idMap;\r\n\t\t};\r\n\r\n\t\tif (state._$LatestUpdatesGrid_feed && state._$LatestUpdatesGrid_entries && state._$LatestUpdatesGrid_idMap) {\r\n\t\t\tfeed = state._$LatestUpdatesGrid_feed;\r\n\t\t\tentries = state._$LatestUpdatesGrid_entries;\r\n\t\t\tidMap = state._$LatestUpdatesGrid_idMap;\r\n\t\t} else {\r\n\t\t\tawait load();\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\tconst load = async (n?: number) => {\r\n\t\tif (isLoading) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst { pathname } = window.location;\r\n\t\tisLoading = true;\r\n\t\ttry {\r\n\t\t\tconst q = new URLSearchParams();\r\n\t\t\tprefs.applyMangaPreferences(q);\r\n\t\t\tprefs.applyChapterPreferences(q);\r\n\r\n\t\t\tif (n) q.set(\"page\", n.toString());\r\n\t\t\tq.set(\"limit\", \"70\");\r\n\r\n\t\t\tconst res = await API.Chapter.LatestUpdates(q);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tfeed = res.body;\r\n\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t...page.state,\r\n\t\t\t\t\t_$LatestUpdatesGrid_feed: $state.snapshot(feed)\r\n\t\t\t\t});\r\n\r\n\t\t\t\tif (feed?.data?.length) {\r\n\t\t\t\t\tfor (const chapter of feed.data) {\r\n\t\t\t\t\t\tconst id = chapter.relationships.manga.id;\r\n\t\t\t\t\t\tif (!(id in idMap)) {\r\n\t\t\t\t\t\t\tidMap[id] = undefined;\r\n\t\t\t\t\t\t\tentries.push(chapter);\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tentries = entries;\r\n\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$LatestUpdatesGrid_entries: $state.snapshot(entries),\r\n\t\t\t\t\t\t_$LatestUpdatesGrid_idMap: $state.snapshot(idMap)\r\n\t\t\t\t\t});\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tif (isMounted) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} else {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t} finally {\r\n\t\t\tisLoading = false;\r\n\t\t}\r\n\t};\r\n\r\n\t$effect(() => {\r\n\t\tif (!footerRef || isCollapsed) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst observer = new IntersectionObserver(\r\n\t\t\tasync ([entry]) => {\r\n\t\t\t\tif (!entry.isIntersecting || !feed || isCollapsed || isLoading) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tawait load(feed.page + 1);\r\n\t\t\t\tif (!feed.data?.length) {\r\n\t\t\t\t\tif (footerRef) {\r\n\t\t\t\t\t\tobserver.unobserve(footerRef);\r\n\t\t\t\t\t}\r\n\t\t\t\t\tobserver.disconnect();\r\n\t\t\t\t}\r\n\t\t\t},\r\n\t\t\t{ rootMargin: window.innerHeight + \"px\" }\r\n\t\t);\r\n\t\tobserver.observe(footerRef);\r\n\t\treturn () => observer.disconnect();\r\n\t});\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { API, Constants } from \"$lib\";\r\n\timport Grid from \"$lib/components/ui/Grid.svelte\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { HTTPError } from \"$lib/HTTP\";\r\n\timport { createSafeReadOwner } from \"$platform/safe-read-owner\";\r\n\timport { requireReadAcknowledgement, readFailure } from \"$platform/mutation-feedback\";\r\n\timport ErrorComponent from \"./ErrorComponent.svelte\";\r\n\timport LatestUpdatesGridEntry from \"./LatestUpdatesGridEntry.svelte\";\r\n\timport LatestUpdatesGridSkeleton from \"./LatestUpdatesGridSkeleton.svelte\";\r\n\timport MangaSearchEntryCard from \"./MangaSearchEntryCard.svelte\";\r\n\timport MangaSearchEntrySkeleton from \"./MangaSearchEntrySkeleton.svelte\";\r\n\tlet feed = $state<ChapterFeedResponse>(), entries = $state<Chapter[]>([]), idMap = $state<Record<string, undefined>>({});\r\n\tlet isMounted = $state(false), isLoading = $state(false), isCollapsed = $state(false), footerRef = $state<HTMLElement>();\r\n\tlet error = $state<Constants.Error.Error>();\r\n\tconst reads = createSafeReadOwner();\r\n\tconst query = (n?: number) => { const q = new URLSearchParams(); prefs.applyMangaPreferences(q); prefs.applyChapterPreferences(q); if (n) q.set(\"page\", n.toString()); q.set(\"limit\", \"70\"); return q; };\r\n\tconst queryKey = () => JSON.stringify([window.location.href, [...query().entries()].filter(([key]) => key !== \"page\")]);\r\n\tonMount(() => {\r\n\t\tconst state = page.state as { _$LatestUpdatesGrid_feed?: ChapterFeedResponse; _$LatestUpdatesGrid_entries?: Chapter[]; _$LatestUpdatesGrid_idMap?: Record<string, undefined>; _$LatestUpdatesGrid_context?: string };\r\n\t\tif (state._$LatestUpdatesGrid_context === queryKey() && state._$LatestUpdatesGrid_feed && state._$LatestUpdatesGrid_entries && state._$LatestUpdatesGrid_idMap) {\r\n\t\t\tfeed = state._$LatestUpdatesGrid_feed; entries = state._$LatestUpdatesGrid_entries; idMap = state._$LatestUpdatesGrid_idMap; isMounted = true;\r\n\t\t} else void load();\r\n\t});\r\n\tconst load = async (n?: number) => {\r\n\t\tif (isLoading || !reads.isLive()) return false;\r\n\t\tconst key = queryKey(), intent = reads.capture(() => queryKey() === key), href = window.location.href;\r\n\t\tisLoading = true; error = undefined; if (!entries.length) isMounted = false;\r\n\t\ttry {\r\n\t\t\tconst body = requireReadAcknowledgement(await API.Chapter.LatestUpdates(query(n)));\r\n\t\t\tif (!intent.alive()) { if (intent.current()) error = readFailure(new HTTPError(409, \"The route or requested filters changed while loading. Retry the current updates.\", \"Updates load interrupted\")); return false; }\r\n\t\t\tif (!Array.isArray(body.data)) throw new HTTPError(502, \"The server returned an invalid update feed. Retry after checking service availability.\");\r\n\t\t\tfeed = body;\r\n\t\t\tfor (const chapter of body.data) { const id = chapter.relationships.manga.id; if (!(id in idMap)) { idMap[id] = undefined; entries.push(chapter); } }\r\n\t\t\tentries = entries;\r\n\t\t\treplaceState(href, { ...page.state, _$LatestUpdatesGrid_feed: $state.snapshot(feed), _$LatestUpdatesGrid_entries: $state.snapshot(entries), _$LatestUpdatesGrid_idMap: $state.snapshot(idMap), _$LatestUpdatesGrid_context: key });\r\n\t\t\treturn true;\r\n\t\t} catch (cause) {\r\n\t\t\tif (intent.current()) error = intent.alive() ? readFailure(cause) : readFailure(new HTTPError(409, \"The update request no longer matches the current route or filters. Retry the current updates.\"));\r\n\t\t\treturn false;\r\n\t\t} finally { if (intent.current()) { isLoading = false; isMounted = true; } }\r\n\t};\r\n\tconst retryUpdates = () => load(entries.length && feed ? feed.page + 1 : undefined);\r\n\t$effect(() => {\r\n\t\tconst target = footerRef;\r\n\t\tif (!target || isCollapsed || error) return;\r\n\t\tlet disposed = false;\r\n\t\tconst observer = new IntersectionObserver(async ([entry]) => {\r\n\t\t\tif (disposed || !entry.isIntersecting || !feed || isCollapsed || isLoading || error || feed.limit * feed.page >= feed.total) return;\r\n\t\t\tconst success = await load(feed.page + 1);\r\n\t\t\tif (disposed || !success || !feed?.data?.length) observer.disconnect();\r\n\t\t}, { rootMargin: window.innerHeight + \"px\" });\r\n\t\tobserver.observe(target); return () => { disposed = true; observer.disconnect(); };\r\n\t});\r\n</script>"
  },
  {
    "path": "src/lib/components/LatestUpdatesGrid.svelte",
    "sha256": "a8c52cc549ac4d7b32521104fdbeb27f53d61f55323bc047365f2056e2ebe481",
    "count": 1,
    "label": "UI14 approved public widget error keeps acknowledged rows and reachable retry",
    "before": "\t<ErrorComponent {error} />\r\n{:else if isMounted}",
    "after": "\t<ErrorComponent {error} />\r\n\t<Button variant=\"outline\" disabled={isLoading} onclick={retryUpdates}>Retry updates</Button>\r\n{/if}\r\n{#if isMounted}",
    "approved_widget_error_exception": "public_updates_error_retry"
  },
  {
    "path": "src/lib/components/LatestUpdatesGrid.svelte",
    "sha256": "a8c52cc549ac4d7b32521104fdbeb27f53d61f55323bc047365f2056e2ebe481",
    "count": 1,
    "label": "UI14 approved failed grid read cannot render a false successful empty state",
    "before": "\t{:else}\r\n\t\t<Empty.Root>",
    "after": "\t{:else if !error}\r\n\t\t<Empty.Root>",
    "approved_widget_error_exception": "public_updates_truthful_empty_guard"
  },
  {
    "path": "src/lib/components/MyUpdatesCarousel.svelte",
    "sha256": "9335b54eab4aef35bd6a42dab0f2157e898a05eb85d0b207169c1d6db7ea2ccd",
    "count": 1,
    "label": "UI14 mounted personal carousel settles failures and coalesces only current owned reads",
    "before": "<script lang=\"ts\">\r\n\tlet feed = $state<MappedChapterFeedResponse>();\r\n\tlet entries = $state<Chapter[]>([]);\r\n\tlet idMap = $state<Record<string, undefined>>({});\r\n\timport { API, Constants } from \"$lib\";\r\n\timport { Button, buttonVariants } from \"$lib/components/ui/button\";\r\n\timport Carousel from \"$lib/components/ui/Carousel.svelte\";\r\n\timport { PersistentStore } from \"$lib/hooks\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { cn } from \"$lib/Utils\";\r\n\timport { ArrowRightIcon, ChevronUp } from \"@lucide/svelte\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport LatestUpdatesGridEntry from \"./LatestUpdatesGridEntry.svelte\";\r\n\timport LatestUpdatesGridSkeleton from \"./LatestUpdatesGridSkeleton.svelte\";\r\n\timport MangaSearchEntryCard from \"./MangaSearchEntryCard.svelte\";\r\n\timport MangaSearchEntrySkeleton from \"./MangaSearchEntrySkeleton.svelte\";\r\n\r\n\tlet isMounted = $state(false);\r\n\tconst isCollapsed = new PersistentStore(\"ivtnx7u8\", false);\r\n\r\n\tonMount(async () => {\r\n\t\tif (!feed?.data?.length) {\r\n\t\t\tconst q = new URLSearchParams();\r\n\t\t\tprefs.applyMangaPreferences(q);\r\n\t\t\tprefs.applyChapterPreferences(q);\r\n\t\t\tq.set(\"limit\", \"10\");\r\n\r\n\t\t\tconst res = await API.MangaFollow.LatestUpdates(q);\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tfeed = res.body;\r\n\t\t\t\tif (feed?.data?.length) {\r\n\t\t\t\t\tfor (const chapter of feed.data) {\r\n\t\t\t\t\t\tconst id = chapter.relationships.manga.id;\r\n\t\t\t\t\t\tif (!(id in idMap)) {\r\n\t\t\t\t\t\t\tidMap[id] = undefined;\r\n\t\t\t\t\t\t\tentries.push(chapter);\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t}\r\n\t\t\t\t\tentries = entries;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { API, Constants } from \"$lib\";\r\n\timport { Button, buttonVariants } from \"$lib/components/ui/button\";\r\n\timport Carousel from \"$lib/components/ui/Carousel.svelte\";\r\n\timport { PersistentStore } from \"$lib/hooks\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { cn } from \"$lib/Utils\";\r\n\timport { ArrowRightIcon, ChevronUp } from \"@lucide/svelte\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { HTTPError } from \"$lib/HTTP\";\r\n\timport { libraryIdentity, libraryViewKey, captureLibraryView } from \"$localLibrary/library\";\r\n\timport { captureNativeAuthority, nativeAuthority, sameNativeReadAuthority } from \"$platform/mutation-authority\";\r\n\timport { createSafeReadOwner } from \"$platform/safe-read-owner\";\r\n\timport { requireReadAcknowledgement, readFailure } from \"$platform/mutation-feedback\";\r\n\timport ErrorComponent from \"./ErrorComponent.svelte\";\r\n\timport LatestUpdatesGridEntry from \"./LatestUpdatesGridEntry.svelte\";\r\n\timport LatestUpdatesGridSkeleton from \"./LatestUpdatesGridSkeleton.svelte\";\r\n\timport MangaSearchEntryCard from \"./MangaSearchEntryCard.svelte\";\r\n\timport MangaSearchEntrySkeleton from \"./MangaSearchEntrySkeleton.svelte\";\r\n\tlet feed = $state<MappedChapterFeedResponse>(), entries = $state<Chapter[]>([]), idMap = $state<Record<string, undefined>>({});\r\n\tlet isMounted = $state(false), isLoading = $state(false), mountedLifecycle = $state(false), error = $state<Constants.Error.Error>();\r\n\tconst isCollapsed = new PersistentStore(\"ivtnx7u8\", false), reads = createSafeReadOwner();\r\n\tlet pending = false, draining = false, observed = \"\";\r\n\tconst query = () => { const q = new URLSearchParams(); prefs.applyMangaPreferences(q); prefs.applyChapterPreferences(q); q.delete(\"tagx\"); q.delete(\"txmod\"); q.set(\"limit\", \"10\"); return q; };\r\n\tconst queryKey = () => JSON.stringify([...query().entries()]);\r\n\tfunction queueUpdates() {\r\n\t\tif (!reads.isLive()) return;\r\n\t\treads.invalidate(); pending = true; feed = undefined; entries = []; idMap = {}; error = undefined; isMounted = false;\r\n\t\tvoid Promise.resolve().then(drain);\r\n\t}\r\n\tasync function drain() {\r\n\t\tif (draining || !reads.isLive()) return;\r\n\t\tdraining = true; isLoading = true;\r\n\t\ttry {\r\n\t\t\twhile (pending && reads.isLive()) {\r\n\t\t\t\tpending = false;\r\n\t\t\t\tconst view = captureLibraryView(), account = $libraryIdentity.mode === \"account\", authority = captureNativeAuthority(), key = queryKey();\r\n\t\t\t\tconst intent = reads.capture(() => captureLibraryView() === view && queryKey() === key && (!account || sameNativeReadAuthority(authority)));\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst body = requireReadAcknowledgement(await API.MangaFollow.LatestUpdates(query()));\r\n\t\t\t\t\tif (!intent.alive()) { if (intent.current()) error = readFailure(new HTTPError(409, \"The selected library or requested filters changed while loading. Retry the current updates.\")); continue; }\r\n\t\t\t\t\tif (!Array.isArray(body.data)) throw new HTTPError(502, \"The server returned an invalid personal update feed.\");\r\n\t\t\t\t\tfeed = body;\r\n\t\t\t\t\tfor (const chapter of body.data) { const id = chapter.relationships.manga.id; if (!(id in idMap)) { idMap[id] = undefined; entries.push(chapter); } }\r\n\t\t\t\t\tentries = entries;\r\n\t\t\t\t} catch (cause) { if (intent.current()) error = intent.alive() ? readFailure(cause) : readFailure(new HTTPError(409, \"The previous personal update request no longer belongs to this library. Retry the current updates.\")); }\r\n\t\t\t\tfinally { if (intent.current()) isMounted = true; }\r\n\t\t\t}\r\n\t\t} finally { draining = false; if (reads.isLive()) isLoading = false; }\r\n\t}\r\n\tonMount(() => { mountedLifecycle = true; });\r\n\t$effect.pre(() => {\r\n\t\tif (!mountedLifecycle || !reads.isLive()) return;\r\n\t\tconst view = $libraryViewKey, mode = $libraryIdentity.mode, authority = $nativeAuthority, preferences = $prefs;\r\n\t\tconst key = JSON.stringify([view, mode === \"account\" ? authority.epoch : null, queryKey()]);\r\n\t\tif (key === observed) return; observed = key; queueUpdates();\r\n\t});\r\n</script>"
  },
  {
    "path": "src/lib/components/MyUpdatesCarousel.svelte",
    "sha256": "9335b54eab4aef35bd6a42dab0f2157e898a05eb85d0b207169c1d6db7ea2ccd",
    "count": 1,
    "label": "UI14 approved personal-widget error remains visible instead of hiding or skeleton loop",
    "before": "{#if !isMounted || entries.length}",
    "after": "{#if !isMounted || entries.length || error}",
    "approved_widget_error_exception": "personal_updates_error_visibility_guard"
  },
  {
    "path": "src/lib/components/MyUpdatesCarousel.svelte",
    "sha256": "9335b54eab4aef35bd6a42dab0f2157e898a05eb85d0b207169c1d6db7ea2ccd",
    "count": 1,
    "label": "UI14 approved personal-widget error has explicit bounded retry",
    "before": "\t\t\t{#if isMounted && entries.length}",
    "after": "\t\t\t{#if error}\r\n\t\t\t\t<ErrorComponent {error} />\r\n\t\t\t\t<Button variant=\"outline\" disabled={isLoading} onclick={queueUpdates}>Retry updates</Button>\r\n\t\t\t{:else if isMounted && entries.length}",
    "approved_widget_error_exception": "personal_updates_error_retry"
  },
  {
    "path": "src/lib/components/ChapterListFilter.svelte",
    "sha256": "2f38ac6741bb02cff665ec0885929ce721082eae1afbc05ce8a3f1bd6f3db6fa",
    "count": 1,
    "label": "UI15 title chapter filter applied baseline follows actual title query navigation and back-forward",
    "before": "\tconst initialSearch = {\r\n\t\ttlang: page.url.searchParams.getAll(\"tlang\") ?? [],\r\n\t\tgroup: page.url.searchParams.getAll(\"group\") ?? [],\r\n\t\torder: page.url.searchParams.get(\"order\") ?? \"desc\"\r\n\t};\r\n\tlet search = $state<ListChaptersRequest>(structuredClone(initialSearch));",
    "after": "\tconst readAppliedSearch = (): ListChaptersRequest => ({\r\n\t\ttlang: page.url.searchParams.getAll(\"tlang\"),\r\n\t\tgroup: page.url.searchParams.getAll(\"group\"),\r\n\t\torder: page.url.searchParams.get(\"order\") ?? \"desc\"\r\n\t});\r\n\tlet initialSearch = $state.raw<ListChaptersRequest>(readAppliedSearch());\r\n\tlet search = $state<ListChaptersRequest>(structuredClone(initialSearch));\r\n\t// Actual navigation changes the applied baseline. Editing the draft alone\r\n\t// never resets it; Apply compares against the current URL, not mount time.\r\n\tconst appliedKey = () => JSON.stringify([manga.id, page.url.pathname, [...page.url.searchParams.entries()]]);\r\n\tlet appliedContext = $state(appliedKey());\r\n\t$effect.pre(() => {\r\n\t\tconst context = appliedKey();\r\n\t\tif (context === appliedContext) return;\r\n\t\tappliedContext = context;\r\n\t\tconst applied = readAppliedSearch();\r\n\t\tinitialSearch = applied;\r\n\t\tsearch = structuredClone(applied);\r\n\t});"
  },
  {
    "path": "src/lib/components/reader/reader-page-selector.svelte",
    "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
    "count": 1,
    "label": "UI15 reader selector validates displayed spread ordinal and maps to actual first page",
    "before": "\tconst readerPrefs = reader.prefs;",
    "after": "\tconst readerPrefs = reader.prefs;\r\n\tconst selectReaderPage = (value: string) => {\r\n\t\tif (!/^(0|[1-9][0-9]*)$/.test(value)) return;\r\n\t\tconst selected = Number(value);\r\n\t\tif (!Number.isSafeInteger(selected)) return;\r\n\t\tconst index = $readerPrefs.double ? reader.spreads[selected]?.[0]?.index : selected;\r\n\t\tif (index === undefined || index < 0 || index >= reader.pages.length) return;\r\n\t\tvoid reader.changePage(index);\r\n\t};"
  },
  {
    "path": "src/lib/components/reader/reader-page-selector.svelte",
    "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
    "count": 1,
    "label": "UI15 approved existing reader selector handler uses canonical page mapping",
    "before": "onValueChange={value => reader.changePage(Number(value))}",
    "after": "onValueChange={selectReaderPage}",
    "approved_reader_selector_exception": "existing_page_selector_handler"
  },
  {
    "path": "src/routes/(app)/title/MangaRecommendationsSection.svelte",
    "sha256": "26b2d29077bf3a8b05593bf8a9abe0d4e30920b3824f42ef9a0c41af816bc7e4",
    "count": 1,
    "label": "UI15 recommendations owns latest read and acknowledges real empty feed instead of perpetual loading",
    "before": "<script lang=\"ts\">\r\n\timport { replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { API, Constants } from \"$lib\";\r\n\timport ErrorComponent from \"$lib/components/ErrorComponent.svelte\";\r\n\timport MangaSearchEntryCard from \"$lib/components/MangaSearchEntryCard.svelte\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport Grid from \"$lib/components/ui/Grid.svelte\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport ViewModeToggles from \"$lib/components/ViewModeToggles.svelte\";\r\n\timport { getManga } from \"$lib/Context\";\r\n\timport { mangaView } from \"$lib/stores\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\r\n\tconst manga = $derived(getManga()());\r\n\tlet feed = $state<MangaFeedResponse>();\r\n\r\n\tlet isGrid = $derived($mangaView === \"grid\");\r\n\tlet isMounted = $state(false);\r\n\tlet error = $state<Constants.Error.Error>();\r\n\r\n\tonMount(async () => {\r\n\t\tif ((page.state as any)?._$Recommendations_feed) {\r\n\t\t\tfeed = (page.state as any)._$Recommendations_feed as typeof feed;\r\n\t\t} else {\r\n\t\t\tconst { href } = window.location;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Manga.Recommendations(manga.id);\r\n\t\t\t\tif (window.location.href !== href) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tfeed = res.body;\r\n\t\t\t\t\treplaceState(page.url.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$Recommendations_feed: $state.snapshot(feed)\r\n\t\t\t\t\t});\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { API, Constants } from \"$lib\";\r\n\timport ErrorComponent from \"$lib/components/ErrorComponent.svelte\";\r\n\timport MangaSearchEntryCard from \"$lib/components/MangaSearchEntryCard.svelte\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport Grid from \"$lib/components/ui/Grid.svelte\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport ViewModeToggles from \"$lib/components/ViewModeToggles.svelte\";\r\n\timport { getManga } from \"$lib/Context\";\r\n\timport { mangaView } from \"$lib/stores\";\r\n\timport { HTTPError } from \"$lib/HTTP\";\r\n\timport { createSafeReadOwner } from \"$platform/safe-read-owner\";\r\n\timport { onMount } from \"svelte\";\r\n\tconst manga = $derived(getManga()());\r\n\tlet feed = $state<MangaFeedResponse>(), isMounted = $state(false), isLoading = $state(false), mountedLifecycle = $state(false);\r\n\tlet isGrid = $derived($mangaView === \"grid\"), error = $state<Constants.Error.Error>();\r\n\tconst reads = createSafeReadOwner();\r\n\tlet observed = \"\", pending = false, draining = false;\r\n\tconst contextKey = () => JSON.stringify([manga.id, page.url.href]);\r\n\tfunction queueRecommendations() {\r\n\t\tif (!reads.isLive()) return;\r\n\t\treads.invalidate(); pending = true; feed = undefined; error = undefined; isMounted = false;\r\n\t\tvoid Promise.resolve().then(drain);\r\n\t}\r\n\tasync function drain() {\r\n\t\tif (draining || !reads.isLive()) return;\r\n\t\tdraining = true; isLoading = true;\r\n\t\ttry {\r\n\t\t\twhile (pending && reads.isLive()) {\r\n\t\t\t\tpending = false;\r\n\t\t\t\tconst id = manga.id, key = contextKey(), intent = reads.capture(() => contextKey() === key);\r\n\t\t\t\ttry {\r\n\t\t\t\t\t// A fresh read rechecks public availability, including cached-empty\r\n\t\t\t\t\t// history states; old history DTOs never replace the current reply.\r\n\t\t\t\t\tconst res = await API.Manga.Recommendations(id);\r\n\t\t\t\t\tif (!intent.alive()) { if (intent.current()) error = new Constants.Error.Error(\"Recommendations unavailable\", \"Navigation interrupted this read. Retry the current title.\"); continue; }\r\n\t\t\t\t\tif (res?.status !== 200 || !res.body || !Array.isArray(res.body.data)) throw new HTTPError(res?.status ?? 0, `Recommendations could not be loaded${res?.status ? ` (HTTP ${res.status})` : \" because the connection ended\"}. No empty result was inferred. Retry when the current title is available.`);\r\n\t\t\t\t\tfeed = res.body;\r\n\t\t\t\t\treplaceState(page.url.href, { ...page.state, _$Recommendations_feed: $state.snapshot(feed), _$Recommendations_context: key });\r\n\t\t\t\t} catch (cause) {\r\n\t\t\t\t\tif (intent.current()) error = new Constants.Error.Error(\"Recommendations unavailable\", cause instanceof HTTPError ? cause.body : \"Recommendations could not be loaded. Retry the current title when the service is available.\");\r\n\t\t\t\t} finally { if (intent.current()) isMounted = true; }\r\n\t\t\t}\r\n\t\t} finally { draining = false; if (reads.isLive()) isLoading = false; }\r\n\t}\r\n\tonMount(() => { mountedLifecycle = true; });\r\n\t$effect.pre(() => {\r\n\t\tif (!mountedLifecycle || !reads.isLive()) return;\r\n\t\tconst key = contextKey(); if (key === observed) return;\r\n\t\tobserved = key; queueRecommendations();\r\n\t});\r\n</script>"
  },
  {
    "path": "src/routes/(app)/title/MangaRecommendationsSection.svelte",
    "sha256": "26b2d29077bf3a8b05593bf8a9abe0d4e30920b3824f42ef9a0c41af816bc7e4",
    "count": 1,
    "label": "UI15 approved recommendations error retry control",
    "before": "\t<ErrorComponent {error} />",
    "after": "\t<ErrorComponent {error} />\r\n\t<Button variant=\"outline\" disabled={isLoading} onclick={queueRecommendations}>Retry recommendations</Button>",
    "approved_recommendation_exception": "truthful_recommendations_retry"
  },
  {
    "path": "src/routes/(app)/title/MangaRecommendationsSection.svelte",
    "sha256": "26b2d29077bf3a8b05593bf8a9abe0d4e30920b3824f42ef9a0c41af816bc7e4",
    "count": 1,
    "label": "UI15 approved acknowledged empty recommendations branch",
    "before": "{:else}\r\n\t<Empty.Root>\r\n\t\t<Spinner class=\"size-10\" />",
    "after": "{:else if isMounted}\r\n\t<Empty.Root>\r\n\t\t<Empty.Title>No recommendations found</Empty.Title>\r\n\t</Empty.Root>\r\n{:else}\r\n\t<Empty.Root>\r\n\t\t<Spinner class=\"size-10\" />",
    "approved_recommendation_exception": "acknowledged_recommendations_empty"
  },
  {
    "path": "src/lib/components/MangaSearchPagination.svelte",
    "sha256": "fdcdc1c9e185a494b55b1fc2af721f54546f698b72432478f31ee30163684111",
    "count": 1,
    "label": "UI16 existing search count matches retained result cap and path page math",
    "before": "count={isFollows ? feed.total : Math.min(feed.total, 10000 - feed.limit)}",
    "after": "count={isFollows ? feed.total : Math.min(feed.total, 10000)}",
    "approved_search_pagination_exception": "consistent_existing_search_count"
  },
  {
    "path": "src/routes/(app)/title/MangaCoversSection.svelte",
    "sha256": "04250e110b499b5652b06005380993043455c02ba6bbe60888b91efea2ea371b",
    "count": 1,
    "label": "UI16 cover read imports owned current-target controller and existing Retry button",
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onMount } from \"svelte\";\r\n\timport { page } from \"$app/state\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport { HTTPError } from \"$lib/HTTP\";\r\n\timport { createSafeReadOwner } from \"$platform/safe-read-owner\";"
  },
  {
    "path": "src/routes/(app)/title/MangaCoversSection.svelte",
    "sha256": "04250e110b499b5652b06005380993043455c02ba6bbe60888b91efea2ea371b",
    "count": 1,
    "label": "UI16 cover read settles actual empty and only latest target navigation context",
    "before": "\tonMount(async () => {\r\n\t\tconst { href } = window.location;\r\n\t\ttry {\r\n\t\t\tconst res = await API.Cover.List(manga.id);\r\n\t\t\tif (window.location.href !== href) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res?.status === StatusCodes.OK) {\r\n\t\t\t\tcovers = res.body ?? [];\r\n\t\t\t\tif (res.body) {\r\n\t\t\t\t\tavailableLocales = Array.from(new Set(res.body.map(v => v.language)));\r\n\t\t\t\t}\r\n\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});",
    "after": "\tconst reads = createSafeReadOwner();\r\n\tlet isLoading = $state(false), mountedLifecycle = $state(false), observed = \"\", pending = false, draining = false;\r\n\tconst contextKey = () => JSON.stringify([manga.id, page.url.href]);\r\n\tfunction queueCovers() {\r\n\t\tif (!reads.isLive()) return;\r\n\t\treads.invalidate(); pending = true; covers = undefined; availableLocales = []; error = undefined; isMounted = false;\r\n\t\tvoid Promise.resolve().then(drain);\r\n\t}\r\n\tasync function drain() {\r\n\t\tif (draining || !reads.isLive()) return;\r\n\t\tdraining = true; isLoading = true;\r\n\t\ttry {\r\n\t\t\twhile (pending && reads.isLive()) {\r\n\t\t\t\tpending = false;\r\n\t\t\t\tconst id = manga.id, key = contextKey(), intent = reads.capture(() => contextKey() === key);\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst res = await API.Cover.List(id);\r\n\t\t\t\t\tif (!intent.alive()) { if (intent.current()) error = new Constants.Error.Error(\"Covers unavailable\", \"Navigation interrupted this read. Retry the current title.\"); continue; }\r\n\t\t\t\t\tif (res?.status !== StatusCodes.OK || !Array.isArray(res.body)) throw new HTTPError(res?.status ?? 0, `Covers could not be loaded${res?.status ? ` (HTTP ${res.status})` : \" because the connection ended\"}. No empty result was inferred. Retry when the current title is available.`);\r\n\t\t\t\t\tcovers = res.body;\r\n\t\t\t\t\tavailableLocales = Array.from(new Set(covers.map(cover => cover.language)));\r\n\t\t\t\t\tselectedLocales = selectedLocales.filter(locale => availableLocales.includes(locale));\r\n\t\t\t\t} catch (cause) {\r\n\t\t\t\t\tif (intent.current()) error = new Constants.Error.Error(\"Covers unavailable\", cause instanceof HTTPError ? cause.body : \"Covers could not be loaded. Retry the current title when the service is available.\");\r\n\t\t\t\t} finally { if (intent.current()) isMounted = true; }\r\n\t\t\t}\r\n\t\t} finally { draining = false; if (reads.isLive()) isLoading = false; }\r\n\t}\r\n\tonMount(() => { mountedLifecycle = true; });\r\n\t$effect.pre(() => {\r\n\t\tif (!mountedLifecycle || !reads.isLive()) return;\r\n\t\tconst key = contextKey(); if (key === observed) return;\r\n\t\tobserved = key; queueCovers();\r\n\t});"
  },
  {
    "path": "src/routes/(app)/title/MangaCoversSection.svelte",
    "sha256": "04250e110b499b5652b06005380993043455c02ba6bbe60888b91efea2ea371b",
    "count": 1,
    "label": "UI16 approved cover read error retry control",
    "before": "\t<ErrorComponent {error} />",
    "after": "\t<ErrorComponent {error} />\r\n\t<Button variant=\"outline\" disabled={isLoading} onclick={queueCovers}>Retry covers</Button>",
    "approved_cover_read_exception": "truthful_covers_retry"
  },
  {
    "path": "src/routes/(app)/title/MangaCoversSection.svelte",
    "sha256": "04250e110b499b5652b06005380993043455c02ba6bbe60888b91efea2ea371b",
    "count": 1,
    "label": "UI16 approved acknowledged empty covers branch",
    "before": "{:else}\r\n\t<Empty.Root>\r\n\t\t<Spinner class=\"size-10\" />",
    "after": "{:else if isMounted}\r\n\t<Empty.Root>\r\n\t\t<Empty.Title>No covers found</Empty.Title>\r\n\t</Empty.Root>\r\n{:else}\r\n\t<Empty.Root>\r\n\t\t<Spinner class=\"size-10\" />",
    "approved_cover_read_exception": "acknowledged_covers_empty"
  },
  {
    "path": "src/lib/components/MostReadCarousel.svelte",
    "sha256": "2749143dcd46dc9b7292aea1713d0752a0e75ca5ebbcc43e1a570aa03e07f86c",
    "count": 1,
    "label": "UI16 MostReadCarousel has no unscoped module feed cache",
    "before": "<script lang=\"ts\" module>\r\n\tlet feed = $state<MangaFeedResponse | undefined>();\r\n</script>",
    "after": "<script lang=\"ts\" module>\r\n\t// Public feed state belongs to this mounted query, never a module cache.\r\n</script>"
  },
  {
    "path": "src/lib/components/MostReadCarousel.svelte",
    "sha256": "2749143dcd46dc9b7292aea1713d0752a0e75ca5ebbcc43e1a570aa03e07f86c",
    "count": 1,
    "label": "UI16 MostReadCarousel shares latest-query public read lifecycle and actual configured ratings",
    "before": "<script lang=\"ts\">\r\n\timport { API, Constants } from \"$lib\";\r\n\timport { buttonVariants } from \"$lib/components/ui/button\";\r\n\timport Carousel from \"$lib/components/ui/Carousel.svelte\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { ArrowRightIcon } from \"@lucide/svelte\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport MostReadCarouselEntry from \"./MostReadCarouselEntry.svelte\";\r\n\timport { AspectRatio } from \"./ui/aspect-ratio\";\r\n\timport Skeleton from \"./ui/skeleton/skeleton.svelte\";\r\n\r\n\tlet isMounted = $state(false);\r\n\tonMount(async () => {\r\n\t\tif (!feed) {\r\n\t\t\ttry {\r\n\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\tq.set(\"rank\", \"read\");\r\n\t\t\t\tq.set(\"time\", \"30d\");\r\n\r\n\t\t\t\tq.append(\"contentRating\", Constants.Manga.ContentRating.Safe);\r\n\t\t\t\tif ($prefs.show_suggestive) q.append(\"contentRating\", Constants.Manga.ContentRating.Suggestive);\r\n\t\t\t\tif ($prefs.show_erotic) q.append(\"contentRating\", Constants.Manga.ContentRating.Erotica);\r\n\t\t\t\tif ($prefs.show_hentai) q.append(\"contentRating\", Constants.Manga.ContentRating.Pornographic);\r\n\r\n\t\t\t\tconst res = await API.Manga.Trending(q);\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tfeed = res.body;\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { page } from \"$app/state\";\r\n\timport { API, Constants } from \"$lib\";\r\n\timport { Button, buttonVariants } from \"$lib/components/ui/button\";\r\n\timport Carousel from \"$lib/components/ui/Carousel.svelte\";\r\n\timport ErrorComponent from \"$lib/components/ErrorComponent.svelte\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { ArrowRightIcon } from \"@lucide/svelte\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { createPublicMangaWidgetRead } from \"$platform/public-manga-widget-read\";\r\n\timport MostReadCarouselEntry from \"./MostReadCarouselEntry.svelte\";\r\n\timport { AspectRatio } from \"./ui/aspect-ratio\";\r\n\timport Skeleton from \"./ui/skeleton/skeleton.svelte\";\r\n\tconst widget = createPublicMangaWidgetRead(() => {\r\n\t\tconst q = new URLSearchParams();\r\n\t\tprefs.applyMangaPreferences(q);\r\n\t\tq.set(\"rank\", \"read\"); q.set(\"time\", \"30d\");\r\n\t\treturn q;\r\n\t}, q => API.Manga.Trending(q));\r\n\tconst widgetState = widget.state;\r\n\tlet feed = $derived($widgetState.feed), isMounted = $derived($widgetState.isMounted), isLoading = $derived($widgetState.isLoading), error = $derived($widgetState.error);\r\n\tlet mountedLifecycle = $state(false);\r\n\tonMount(() => { mountedLifecycle = true; });\r\n\t$effect.pre(() => {\r\n\t\tconst preferences = $prefs, route = page.url.href;\r\n\t\tif (mountedLifecycle) widget.synchronize();\r\n\t});\r\n</script>"
  },
  {
    "path": "src/lib/components/MostReadCarousel.svelte",
    "sha256": "2749143dcd46dc9b7292aea1713d0752a0e75ca5ebbcc43e1a570aa03e07f86c",
    "count": 1,
    "label": "UI16 approved MostReadCarousel failed section stays visible",
    "before": "{#if !isMounted || feed?.data?.length}",
    "after": "{#if !isMounted || feed?.data?.length || error}",
    "approved_public_carousel_exception": "public_carousel_error_visibility"
  },
  {
    "path": "src/lib/components/MostReadCarousel.svelte",
    "sha256": "2749143dcd46dc9b7292aea1713d0752a0e75ca5ebbcc43e1a570aa03e07f86c",
    "count": 1,
    "label": "UI16 approved MostReadCarousel explicit current public-feed retry",
    "before": "\t\t{#if isMounted && feed?.data?.length}",
    "after": "\t\t{#if error}\r\n\t\t\t<ErrorComponent {error} />\r\n\t\t\t<Button variant=\"outline\" disabled={isLoading} onclick={widget.retry}>Retry titles</Button>\r\n\t\t{:else if isMounted && feed?.data?.length}",
    "approved_public_carousel_exception": "public_carousel_error_retry"
  },
  {
    "path": "src/lib/components/RecentlyAddedCarousel.svelte",
    "sha256": "88b01e9d3bd52280ebfe6fdb01d5bd533f71d7fc0198c1d20f482eada24653cf",
    "count": 1,
    "label": "UI16 RecentlyAddedCarousel has no unscoped module feed cache",
    "before": "<script lang=\"ts\" module>\r\n\tlet feed = $state<MangaFeedResponse>();\r\n</script>",
    "after": "<script lang=\"ts\" module>\r\n\t// Public feed state belongs to this mounted query, never a module cache.\r\n</script>"
  },
  {
    "path": "src/lib/components/RecentlyAddedCarousel.svelte",
    "sha256": "88b01e9d3bd52280ebfe6fdb01d5bd533f71d7fc0198c1d20f482eada24653cf",
    "count": 1,
    "label": "UI16 RecentlyAddedCarousel shares latest-query public read lifecycle and actual configured ratings",
    "before": "<script lang=\"ts\">\r\n\timport { API, Constants } from \"$lib\";\r\n\timport { buttonVariants } from \"$lib/components/ui/button\";\r\n\timport Carousel from \"$lib/components/ui/Carousel.svelte\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { ArrowRightIcon } from \"@lucide/svelte\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport MangaSearchEntryCard from \"./MangaSearchEntryCard.svelte\";\r\n\timport MangaSearchEntrySkeleton from \"./MangaSearchEntrySkeleton.svelte\";\r\n\r\n\tlet isMounted = $state(false);\r\n\r\n\tonMount(async () => {\r\n\t\tif (!feed?.data) {\r\n\t\t\ttry {\r\n\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\tprefs.applyMangaPreferences(q);\r\n\r\n\t\t\t\tq.set(\"sort\", Constants.Manga.SortBy.CreatedAt);\r\n\t\t\t\tq.set(\"limit\", \"10\");\r\n\r\n\t\t\t\tconst res = await API.Manga.Search(q);\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tfeed = res.body;\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { page } from \"$app/state\";\r\n\timport { API, Constants } from \"$lib\";\r\n\timport { Button, buttonVariants } from \"$lib/components/ui/button\";\r\n\timport Carousel from \"$lib/components/ui/Carousel.svelte\";\r\n\timport ErrorComponent from \"$lib/components/ErrorComponent.svelte\";\r\n\timport { prefs } from \"$lib/stores\";\r\n\timport { ArrowRightIcon } from \"@lucide/svelte\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { createPublicMangaWidgetRead } from \"$platform/public-manga-widget-read\";\r\n\timport MangaSearchEntryCard from \"./MangaSearchEntryCard.svelte\";\r\n\timport MangaSearchEntrySkeleton from \"./MangaSearchEntrySkeleton.svelte\";\r\n\tconst widget = createPublicMangaWidgetRead(() => {\r\n\t\tconst q = new URLSearchParams();\r\n\t\tprefs.applyMangaPreferences(q);\r\n\t\tq.set(\"sort\", Constants.Manga.SortBy.CreatedAt); q.set(\"limit\", \"10\");\r\n\t\treturn q;\r\n\t}, q => API.Manga.Search(q));\r\n\tconst widgetState = widget.state;\r\n\tlet feed = $derived($widgetState.feed), isMounted = $derived($widgetState.isMounted), isLoading = $derived($widgetState.isLoading), error = $derived($widgetState.error);\r\n\tlet mountedLifecycle = $state(false);\r\n\tonMount(() => { mountedLifecycle = true; });\r\n\t$effect.pre(() => {\r\n\t\tconst preferences = $prefs, route = page.url.href;\r\n\t\tif (mountedLifecycle) widget.synchronize();\r\n\t});\r\n</script>"
  },
  {
    "path": "src/lib/components/RecentlyAddedCarousel.svelte",
    "sha256": "88b01e9d3bd52280ebfe6fdb01d5bd533f71d7fc0198c1d20f482eada24653cf",
    "count": 1,
    "label": "UI16 approved RecentlyAddedCarousel failed section stays visible",
    "before": "{#if !isMounted || feed?.data?.length}",
    "after": "{#if !isMounted || feed?.data?.length || error}",
    "approved_public_carousel_exception": "public_carousel_error_visibility"
  },
  {
    "path": "src/lib/components/RecentlyAddedCarousel.svelte",
    "sha256": "88b01e9d3bd52280ebfe6fdb01d5bd533f71d7fc0198c1d20f482eada24653cf",
    "count": 1,
    "label": "UI16 approved RecentlyAddedCarousel explicit current public-feed retry",
    "before": "\t\t<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>\r\n\t\t\t{#if isMounted && feed?.data?.length}\r\n\t\t\t\t{#each feed.data as manga (manga)}\r\n\t\t\t\t\t<article class=\"swiper-slide relative\">\r\n\t\t\t\t\t\t<MangaSearchEntryCard {manga} isGrid />\r\n\t\t\t\t\t</article>\r\n\t\t\t\t{/each}\r\n\t\t\t{:else}\r\n\t\t\t\t<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>\r\n\t\t\t\t\t{#each Array.from({ length: 10 })}\r\n\t\t\t\t\t\t<article class=\"swiper-slide relative\">\r\n\t\t\t\t\t\t\t<MangaSearchEntrySkeleton isGrid />\r\n\t\t\t\t\t\t</article>\r\n\t\t\t\t\t{/each}\r\n\t\t\t\t</Carousel>\r\n\t\t\t{/if}\r\n\t\t</Carousel>",
    "after": "\t\t{#if error}\r\n\t\t\t<ErrorComponent {error} />\r\n\t\t\t<Button variant=\"outline\" disabled={isLoading} onclick={widget.retry}>Retry titles</Button>\r\n\t\t{:else}\r\n\t\t<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>\r\n\t\t\t{#if isMounted && feed?.data?.length}\r\n\t\t\t\t{#each feed.data as manga (manga)}\r\n\t\t\t\t\t<article class=\"swiper-slide relative\">\r\n\t\t\t\t\t\t<MangaSearchEntryCard {manga} isGrid />\r\n\t\t\t\t\t</article>\r\n\t\t\t\t{/each}\r\n\t\t\t{:else}\r\n\t\t\t\t<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>\r\n\t\t\t\t\t{#each Array.from({ length: 10 })}\r\n\t\t\t\t\t\t<article class=\"swiper-slide relative\">\r\n\t\t\t\t\t\t\t<MangaSearchEntrySkeleton isGrid />\r\n\t\t\t\t\t\t</article>\r\n\t\t\t\t\t{/each}\r\n\t\t\t\t</Carousel>\r\n\t\t\t{/if}\r\n\t\t</Carousel>\r\n\t\t{/if}",
    "approved_public_carousel_exception": "public_carousel_error_retry"
  }
];
const hash = value => createHash('sha256').update(value).digest('hex');
export function createLocalLibraryOverlays(frontendRoot) {
  const byPath = new Map();
  for (const patch of patches) {
    const absolute = resolve(frontendRoot, patch.path);
    if (hash(readFileSync(absolute)) !== patch.sha256) throw Error(`Local-library overlay source changed: ${patch.path}`);
    const entries = byPath.get(absolute) ?? []; entries.push(patch); byPath.set(absolute, entries);
  }
  return { name: 'weebdex-local-library-overlays', enforce: 'pre', transform: { order: 'pre', handler(code, id) {
    if (id.includes('?')) return null;
    const entries = byPath.get(resolve(id)); if (!entries) return null;
    // Another independent reviewed plugin may precede this one. Source-file hash
    // is checked at construction; every exact original anchor still must match.
    let result = code;
    for (const patch of entries) {
      if (result.split(patch.before).length - 1 !== patch.count) throw Error(`Local-library anchor changed: ${patch.label}`);
      result = result.split(patch.before).join(patch.after);
    }
    return { code: result, map: null };
  } } };
}
export function createLocalLibraryResolver(frontendRoot, integrationRoot) {
  const api = resolve(frontendRoot, 'src/lib/API');
  return { name: 'weebdex-local-library-api-resolver', enforce: 'pre', resolveId(source, importer) {
    for (const name of ['MangaFollow', 'ChapterHistory']) {
      if ((source === './' + name && importer?.split('?')[0] === resolve(api, 'index.ts')) || source === '$lib/API/' + name || source === '$lib/API/' + name + '.ts' || source === resolve(api, name) || source === resolve(api, name + '.ts')) return resolve(integrationRoot, name + '.ts');
    }
    return null;
  } };
}
export const localLibraryOverlayManifest = patches.map(({ before, after, ...metadata }) => metadata);
export const localLibraryApprovedMarkupExceptions = patches.filter(patch => patch.approved_library_exception).map(({ path, sha256, count, before, after, approved_library_exception }) => ({ path, sha256, count, before, after, approved_library_exception }));

export const localLibraryApprovedFeedPaginationExceptions = patches.filter(patch => patch.approved_feed_pagination_exception).map(({path,sha256,count,before,after,approved_feed_pagination_exception}) => ({path,sha256,count,before,after,approved_feed_pagination_exception}));

export const localLibraryApprovedWidgetErrorExceptions = patches.filter(patch => patch.approved_widget_error_exception).map(({path,sha256,count,before,after,approved_widget_error_exception}) => ({path,sha256,count,before,after,approved_widget_error_exception}));

export const localLibraryApprovedReaderSelectorExceptions = patches.filter(patch => patch.approved_reader_selector_exception).map(({path,sha256,count,before,after,approved_reader_selector_exception}) => ({path,sha256,count,before,after,approved_reader_selector_exception}));

export const localLibraryApprovedRecommendationExceptions = patches.filter(patch => patch.approved_recommendation_exception).map(({path,sha256,count,before,after,approved_recommendation_exception}) => ({path,sha256,count,before,after,approved_recommendation_exception}));

export const localLibraryApprovedSearchPaginationExceptions = patches.filter(patch => patch.approved_search_pagination_exception).map(({path,sha256,count,before,after,approved_search_pagination_exception}) => ({path,sha256,count,before,after,approved_search_pagination_exception}));

export const localLibraryApprovedCoverReadExceptions = patches.filter(patch => patch.approved_cover_read_exception).map(({path,sha256,count,before,after,approved_cover_read_exception}) => ({path,sha256,count,before,after,approved_cover_read_exception}));

export const localLibraryApprovedPublicCarouselExceptions = patches.filter(patch => patch.approved_public_carousel_exception).map(({path,sha256,count,before,after,approved_public_carousel_exception}) => ({path,sha256,count,before,after,approved_public_carousel_exception}));
