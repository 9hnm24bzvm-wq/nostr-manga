<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { page } from '$app/state';
  import { prefs } from '$lib/stores';
  import { Constants } from '$lib';
  import Grid from '$lib/components/ui/Grid.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import { writable } from 'svelte/store';
  import { setReadHistory } from '$lib/Context';
  import ChapterFeedMangaEntry from '$lib/components/ChapterFeedMangaEntry.svelte';
  import Carousel from '$lib/components/ui/Carousel.svelte';
  import LatestUpdatesGridEntry from '$lib/components/LatestUpdatesGridEntry.svelte';
  import MangaSearchEntryCard from '$lib/components/MangaSearchEntryCard.svelte';
  import { libraryViewKey, libraryIdentity, localLibraryState, message } from './library';
  import { nativeAuthority } from '$platform/mutation-authority';
  import { captureNavigationEpoch } from '$platform/navigation-authority';
  import { openDeviceUpdatesFeed } from './updates-feed';
  import type { UpdateRow } from './updates-pagination';
  import { UpdatesCancelled, type UpdatesPager, type UpdatePage } from './updates-pagination';
  import { TitleIndexStopped } from './title-index';
  let { latestOnly = false }: { latestOnly?: boolean } = $props();
  const readHistory = writable<Record<string, Record<string, null>>>({}); setReadHistory(readHistory);
  let entries = $state<Manga[]>([]);
  import { HistoryTransportUnsettled } from './history-pagination';
  let result = $state<UpdatePage<Chapter & UpdateRow>>(), error = $state(''), stopped = $state(false), loading = $state(false), mounted = $state(false), target = $state(1);
  let pager: UpdatesPager<Chapter & UpdateRow> | undefined;
  let generation = 0, destroyed = false, ownedIdentity = '';
  const identity = () => JSON.stringify([$libraryViewKey, $libraryIdentity.accountId, $nativeAuthority, captureNavigationEpoch(), $prefs, page.url.href, latestOnly]);
  function assertOwner(epoch: number, key: string) { if (destroyed || epoch !== generation || key !== identity()) throw new UpdatesCancelled(); }
  async function load(n = target) {
    if (loading || destroyed) return;
    const epoch = generation, key = ownedIdentity;
    target = n; result = undefined; entries = []; $readHistory = {}; loading = true; error = ''; stopped = false;
    try {
      // Release the previous rendered DTO page before admitting another one.
      await tick(); assertOwner(epoch, key); pager?.releaseDisplayedPage();
      if (!pager) {
        const query = new URLSearchParams(); prefs.applyMangaPreferences(query); prefs.applyChapterPreferences(query);
        query.set('limit', latestOnly ? '10' : '100');
        const opened = await openDeviceUpdatesFeed(query, latestOnly, () => assertOwner(epoch, key));
        try { assertOwner(epoch, key); } catch (cause) { await opened.dispose(); throw cause; }
        pager = opened;
      }
      let next = await pager.load(n); assertOwner(epoch, key);
      // Indexing and hydration have independent bounded budgets. Once all titles
      // are checked, display one page automatically; never auto-loop scans.
      if (next.indexed && !next.ready && !next.notice) {
        next = await pager.load(n); assertOwner(epoch, key);
      }
      result = next; target = next.page;
      const grouped = new Map<string, Manga>(), selected = new Set(next.data.map(row => row.id));
      const history: Record<string, Record<string, null>> = {};
      for (const saved of $localLibraryState.records) if (saved.family === 'chapters' && saved.completed && saved.id && saved.mangaId && selected.has(saved.id)) (history[saved.mangaId] ??= {})[saved.id] = null;
      for (const chapter of next.data) { const parent = chapter.relationships.manga; let manga = grouped.get(parent.id); if (!manga) { manga = { ...parent, relationships: { ...parent.relationships, _chapters: [] } }; grouped.set(parent.id,manga); } manga.relationships._chapters!.push(chapter); }
      entries = [...grouped.values()]; $readHistory = history;
    } catch (cause) {
      if (!destroyed && epoch === generation && key === identity()) { error = message(cause); stopped = cause instanceof TitleIndexStopped || cause instanceof HistoryTransportUnsettled; }
    } finally { if (!destroyed && epoch === generation) loading = false; }
  }
  function cancel() {
    if (pager) pager.cancel();
    else { generation++; loading = false; error = 'Update loading was cancelled. Retry to resume.'; }
  }
  function reset() {
    generation++; const previous = pager; pager = undefined; result = undefined; entries = []; $readHistory = {}; error = ''; loading = false; stopped = false;
    ownedIdentity = identity();
    const epoch = generation, key = ownedIdentity;
    const requested = Number(page.url.searchParams.get('page') ?? 1); target = Number.isSafeInteger(requested) && requested > 0 ? requested : 1;
    if (!previous) { void load(target); return; }
    loading = true;
    void previous.dispose().then(() => {
      if (destroyed || epoch !== generation || key !== identity()) return;
      loading = false; void load(target);
    }).catch(cause => {
      if (!destroyed && epoch === generation && key === identity()) { loading = false; stopped = true; error = message(cause); }
    });
  }
  onMount(() => { mounted = true; });
  onDestroy(() => { destroyed = true; generation++; void pager?.dispose().catch(() => { /* The storage owner remains poisoned until actual settlement; a reopened view reports it. */ }); });
  $effect.pre(() => { const key = identity(); if (mounted && key !== ownedIdentity) reset(); });
</script>

{#if result?.data.length}
  {#if latestOnly}
    <Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>
      {#each result.data as chapter (chapter.id)}<article class="swiper-slide relative"><MangaSearchEntryCard manga={chapter.relationships.manga} isGrid /><LatestUpdatesGridEntry {chapter} /></article>{/each}
    </Carousel>
  {:else}
    <Grid class="gap-x-4" breakpoints={{ 0: 3, 1400: 2, 1080: 1 }}>
      {#each entries as manga (manga.id)}<ChapterFeedMangaEntry {manga} />{/each}
    </Grid>
  {/if}
{:else if result?.ready && !error}
  <p role="status">{result.total === 0 ? 'No new chapters found for your followed titles.' : 'There are no more updates on this page.'}</p>
{/if}
{#if error}<p role="alert">{error}</p>{/if}
{#if loading}
  <div class="flex items-center gap-2" role="status"><Spinner class="size-6" /> Loading updates…</div>
  <Button variant="outline" onclick={cancel}>Cancel loading</Button>
{:else}
  <div class="flex flex-wrap items-center gap-2">
    {#if stopped}<Button variant="outline" onclick={reset}>Refresh updates</Button>
    {:else if error}<Button variant="outline" onclick={() => load()}>Retry updates</Button>
    {:else if !result?.ready}<Button variant="outline" onclick={() => load()}>Continue loading</Button>
    {:else if result.total !== null && target * result.limit < result.total}<Button variant="outline" onclick={() => load(target + 1)}>Next page</Button>{/if}
    {#if result && !stopped}
      {#if target > 1}<Button variant="outline" onclick={() => load(target - 1)}>Previous page</Button><Button variant="outline" onclick={() => load(1)}>First page</Button>{/if}
      {#if result.total && Math.ceil(result.total / result.limit) !== target}<Button variant="outline" onclick={() => load(Math.ceil(result!.total! / result!.limit))}>Last page</Button>{/if}
      <Button variant="outline" onclick={reset}>Refresh updates</Button>
    {/if}
  </div>
{/if}
{#if result}
  {#if result.notice}<p role="status">There’s more to check. Continue loading to pick up where you left off.</p>{/if}
  <p class="text-muted-foreground text-sm" role="status">
    {#if !result.ready}Checking your followed titles: {result.scanned} of {result.parents}. Updates will appear when the check is complete.
    {:else if result.total}{result.total} {latestOnly ? 'updated titles' : 'chapters'} · Page {target}{/if}
  </p>
{/if}
