<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { page } from '$app/state';
  import { prefs, mangaView } from '$lib/stores';
  import { Constants } from '$lib';
  import Grid from '$lib/components/ui/Grid.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import MangaSearchEntryCard from '$lib/components/MangaSearchEntryCard.svelte';
  import { libraryViewKey, libraryIdentity, localLibraryState, isCurrentCatalogueId, message } from './library';
  import { libraryDiscovery, unfilteredLibraryHref } from './library-discovery';
  import { nativeAuthority } from '$platform/mutation-authority';
  import { captureNavigationEpoch } from '$platform/navigation-authority';
  import { openDeviceTitleFeed } from './title-feed';
  import { TitleCancelled, TitleIndexStopped, type TitlePager, type TitlePage } from './title-pagination';
  import { HistoryTransportUnsettled } from './history-pagination';
  let result = $state<TitlePage<Manga>>(), error = $state(''), stopped = $state(false), loading = $state(false), mounted = $state(false), target = $state(1);
  let pager: TitlePager<Manga> | undefined;
  const selectedList = $derived(page.url.searchParams.get('list'));
  const discovery = $derived(libraryDiscovery($localLibraryState.records, selectedList, isCurrentCatalogueId));
  let generation = 0, destroyed = false, ownedIdentity = '';
  const identity = () => JSON.stringify([$libraryViewKey, $libraryIdentity.accountId, $nativeAuthority, captureNavigationEpoch(), $prefs, page.url.href]);
  function assertOwner(epoch: number, key: string) { if (destroyed || epoch !== generation || key !== identity()) throw new TitleCancelled(); }
  async function load(n = target) {
    if (loading || destroyed) return;
    const epoch = generation, key = ownedIdentity;
    target = n; result = undefined; loading = true; error = ''; stopped = false;
    try {
      // Release the previous rendered DTO page before admitting another one.
      await tick(); assertOwner(epoch, key); pager?.releaseDisplayedPage();
      if (!pager) {
        const query = new URLSearchParams(page.url.searchParams);
        // Retain library semantics: explicit URL filters; absent ratings mean all saved titles.
        if (!query.has('limit')) query.set('limit', String($prefs.items_per_page));
        const opened = await openDeviceTitleFeed(query, () => assertOwner(epoch, key));
        try { assertOwner(epoch, key); } catch (cause) { await opened.dispose(); throw cause; }
        pager = opened;
      }
      let next = await pager.load(n); assertOwner(epoch, key);
      // A finished index needs one bounded page read, not another user action.
      if (next.indexed && !next.ready && !next.notice) { pager.releaseDisplayedPage(); next = await pager.load(n); assertOwner(epoch, key); }
      result = next; target = next.page;
    } catch (cause) {
      if (!destroyed && epoch === generation && key === identity()) { error = message(cause); stopped = cause instanceof TitleIndexStopped || cause instanceof HistoryTransportUnsettled; }
    } finally { if (!destroyed && epoch === generation) loading = false; }
  }
  function cancel() {
    if (pager) pager.cancel();
    else { generation++; loading = false; error = 'Title loading was cancelled. Retry to resume.'; }
  }
  function reset() {
    generation++; const previous = pager; pager = undefined; result = undefined; error = ''; loading = false; stopped = false;
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
  <Grid class="gap-3" breakpoints={$mangaView === 'grid' ? Constants.MangaGridBreakpoints : { 0: 2, 768: 1 }}>
    {#each result.data as manga (manga.id)}<article><MangaSearchEntryCard {manga} isGrid={$mangaView === 'grid'} /></article>{/each}
  </Grid>
{:else if result?.ready && !error}
  <div class="space-y-2" role="status">
    {#if result.total !== 0}
      <p>There are no more saved titles on this page.</p>
    {:else if discovery.saved === 0}
      <p>Your library is empty. Find a title you like, then choose “Add to Library” to save it here.</p>
      <a class="inline-block rounded border px-4 py-2" href="/search">Discover titles</a>
    {:else if selectedList && discovery.listName && discovery.listSaved === 0}
      <p>No saved titles in “{discovery.listName}” yet. Add a saved title to this list from your library.</p>
      <a class="underline" href="/library">Browse saved titles</a>
    {:else}
      <p>No saved titles match these filters. Your saved titles have not been removed.</p>
      <a class="underline" href={unfilteredLibraryHref(selectedList)}>Clear filters</a>
    {/if}
  </div>
{/if}
{#if error}<p role="alert">{error}</p>{/if}
{#if loading}
  <div class="flex items-center gap-2" role="status"><Spinner class="size-6" /> Loading titles…</div>
  <Button variant="outline" onclick={cancel}>Cancel loading</Button>
{:else}
  <div class="flex flex-wrap items-center gap-2">
    {#if stopped}<Button variant="outline" onclick={reset}>Refresh library</Button>
    {:else if error}<Button variant="outline" onclick={() => load()}>Retry titles</Button>
    {:else if !result?.ready}<Button variant="outline" onclick={() => load()}>Continue loading</Button>
    {:else if result.total === null || target * result.limit < result.total}<Button variant="outline" onclick={() => load(target + 1)}>Next page</Button>{/if}
    {#if result && !stopped}
      {#if target > 1}<Button variant="outline" onclick={() => load(target - 1)}>Previous page</Button><Button variant="outline" onclick={() => load(1)}>First page</Button>{/if}
      {#if result.total && Math.ceil(result.total / result.limit) !== target}<Button variant="outline" onclick={() => load(Math.ceil(result!.total! / result!.limit))}>Last page</Button>{/if}
      <Button variant="outline" onclick={reset}>Refresh library</Button>
    {/if}
  </div>
{/if}
{#if result}
  {#if result.notice}<p role="status">Loading paused. Choose Continue loading to keep going.</p>{/if}
  <p class="text-muted-foreground text-sm" role="status">
    Page {target}. {#if result.total === null}Checked {result.scanned} of {result.candidates} saved titles.{:else}{result.total} matching {result.total === 1 ? 'title' : 'titles'}.{/if}
  </p>
  <p class="text-muted-foreground text-sm">Refresh to check for changes to your saved titles.</p>
{/if}
