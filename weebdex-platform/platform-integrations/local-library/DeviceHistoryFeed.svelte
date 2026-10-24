<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { page } from '$app/state';
  import { prefs } from '$lib/stores';
  import { writable } from 'svelte/store';
  import { setReadHistory } from '$lib/Context';
  import Grid from '$lib/components/ui/Grid.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as Empty from '$lib/components/ui/empty';
  import ChapterFeedMangaEntry from '$lib/components/ChapterFeedMangaEntry.svelte';
  import { libraryViewKey, libraryIdentity, message } from './library';
  import { openDeviceHistoryFeed } from './history-feed';
  import { HistoryCancelled, HistoryTransportUnsettled, HistoryCapacityError, type HistoryPager, type HistoryPage } from './history-pagination';

  const readHistory = writable<Record<string, Record<string, null>>>({});
  setReadHistory(readHistory);
  let result = $state<HistoryPage<ChapterHistory>>();
  let entries = $state<Manga[]>([]);
  let error = $state('');
  let stopped = $state(false);
  let loading = $state(false);
  let target = $state(1);
  let pager: HistoryPager<ChapterHistory> | undefined;
  let generation = 0, destroyed = false, mounted = $state(false);
  const identity = () => JSON.stringify([$libraryViewKey, $libraryIdentity.accountId, $prefs, page.url.href]);
  let ownedIdentity = '';
  function assertOwner(epoch: number, key: string) {
    if (destroyed || epoch !== generation || key !== identity()) throw new HistoryCancelled();
  }
  async function load(n = target) {
    if (loading || destroyed) return;
    const epoch = generation, key = ownedIdentity;
    const changedPage = n !== target;
    if (changedPage) { result = undefined; entries = []; $readHistory = {}; }
    target = n; loading = true; error = ''; stopped = false;
    try {
      if (changedPage) { await tick(); assertOwner(epoch, key); pager?.releaseDisplayedPage(); }
      assertOwner(epoch, key);
      if (!pager) {
        const query = new URLSearchParams(); prefs.applyMangaPreferences(query); prefs.applyChapterPreferences(query); query.set('limit', '100');
        const opened = await openDeviceHistoryFeed(query, () => assertOwner(epoch, key));
        try { assertOwner(epoch, key); } catch (cause) { opened.dispose(); throw cause; }
        pager = opened;
      }
      const next = await pager.load(n); assertOwner(epoch, key);
      result = next; target = next.page;
      const grouped = new Map<string, Manga>(), history: Record<string, Record<string, null>> = {};
      for (const chapter of next.data) {
        const parent = chapter.relationships.manga;
        let manga = grouped.get(parent.id);
        if (!manga) { manga = { ...parent, relationships: { ...parent.relationships, _chapters: [] } }; grouped.set(parent.id, manga); }
        manga.relationships._chapters!.push(chapter); (history[parent.id] ??= {})[chapter.id] = null;
      }
      entries = [...grouped.values()]; $readHistory = history;
    } catch (cause) {
      if (!destroyed && epoch === generation && key === identity()) { error = message(cause); stopped = cause instanceof HistoryTransportUnsettled || cause instanceof HistoryCapacityError; }
    } finally { if (!destroyed && epoch === generation) loading = false; }
  }
  function cancel() {
    if (pager) pager.cancel();
    else { generation++; loading = false; error = 'History loading was cancelled. Retry to resume.'; }
  }
  function reset() {
    generation++; pager?.dispose(); pager = undefined; result = undefined; entries = []; $readHistory = {}; loading = false; error = ''; stopped = false;
    ownedIdentity = identity();
    const requested = Number(page.url.searchParams.get('page') ?? 1); target = Number.isSafeInteger(requested) && requested > 0 ? requested : 1;
    void load(target);
  }
  onMount(() => { mounted = true; });
  onDestroy(() => { destroyed = true; generation++; pager?.dispose(); });
  $effect.pre(() => { const key = identity(); if (mounted && key !== ownedIdentity) reset(); });
</script>

{#if entries.length}
  <Grid class="gap-x-4" breakpoints={{ 0: 3, 1400: 2, 1080: 1 }}>
    {#each entries as manga (manga)}<ChapterFeedMangaEntry {manga} />{/each}
  </Grid>
{:else if result?.complete && !error}
  <Empty.Root><Empty.Header><Empty.Title>No results found</Empty.Title><Empty.Description>{#if result.total === 0}No read chapters match these preferences in this snapshot.{:else}This page is past the end of the matching history. Return to the first page.{/if}</Empty.Description></Empty.Header></Empty.Root>
{:else if !loading && result}
  <p role="status">No matching chapters found yet. More saved history remains to be checked.</p>
{/if}
{#if error}<p role="alert">{error}</p>{/if}
{#if loading}
  <div class="flex items-center gap-2" role="status"><Spinner class="size-6" /> Loading history…</div>
  <Button variant="outline" onclick={cancel}>Cancel loading</Button>
{:else}
  <div class="flex flex-wrap items-center gap-2">
    {#if stopped}<Button variant="outline" onclick={reset}>Reopen history</Button>
    {:else if error}<Button variant="outline" onclick={() => load()}>Retry history</Button>
    {:else if !result?.ready}<Button variant="outline" onclick={() => load()}>Continue scanning</Button>
    {:else if result.continuation}<Button variant="outline" onclick={() => load(result!.page + 1)}>Continue loading</Button>{/if}
    {#if result?.total && Math.ceil(result.total / result.limit) !== target}<Button variant="outline" onclick={() => load(Math.ceil(result!.total! / result!.limit))}>Last page</Button>{/if}
    {#if target > 1}<Button variant="outline" onclick={() => load(target - 1)}>Previous page</Button><Button variant="outline" onclick={() => load(1)}>First page</Button>{/if}
  </div>
{/if}
{#if result}
  {#if result.notice}<p role="status">{result.notice}</p>{/if}
  <p class="text-muted-foreground text-sm" role="status">
    Page {target}. Checked {result.scanned} of {result.candidates} saved chapters.
    {#if result.total === null}Filtered total is not known yet.{:else}{result.total} matching chapters in this scan.{/if}
    {#if !result.ready}Continue scanning to finish this page.{/if}
  </p>
  <p class="text-muted-foreground text-sm">Only a few pages are kept in memory. Older pages are scanned again when revisited.</p>
{/if}
