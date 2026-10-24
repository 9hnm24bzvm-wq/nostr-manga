<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import { createBoundedReader, titleFilters } from './runtime.svelte';
  import { plainAction } from './presentation';
  import { localLibraryState, readerLibrarySelection } from '../local-library/library';
  import { nativeAuthority } from '../frontend-behavior/mutation-authority';
  import { publicUuidLibraryEnabled } from '../public-api/library-staging';
  import EditionPage from './EditionPage.svelte';
  import Status from './Status.svelte';
  let { mangaId, disabled = false, devicePrimary, deviceHistoryReady = false, deviceError = '', legacyPending = false,
    clearDeviceSelection = () => true, onPending = () => {} }: {
    mangaId: string; disabled?: boolean; devicePrimary?: Snippet<[boolean, () => boolean]>;
    deviceHistoryReady?: boolean; deviceError?: string; legacyPending?: boolean;
    clearDeviceSelection?: () => boolean; onPending?: (pending: boolean) => void;
  } = $props();
  const controller = createBoundedReader({ manga: () => mangaId, filters: () => titleFilters(page.url.searchParams), queryEpoch: () => page.url.href });
  const blocked = $derived(disabled || legacyPending || $controller.pending || $controller.requiresReload || $readerLibrarySelection.pending);
  const prepareDevice = () => !blocked && controller.dismissSelection();
  const startFirst = () => { if (!blocked && clearDeviceSelection()) return controller.start(); };
  $effect(() => { onPending($controller.pending); });
</script>
<div class="space-y-2" data-bounded-title-reading>
  <div class="flex flex-wrap items-center gap-2">
    {#if $localLibraryState.mode === 'device'}
      {#if devicePrimary}{@render devicePrimary(!!blocked, prepareDevice)}{:else}<button type="button" disabled>Device reading control unavailable</button>{/if}
    {:else if !publicUuidLibraryEnabled}
      <button type="button" class="rounded border px-4 py-2" disabled={blocked || !$nativeAuthority.actor} onclick={event => plainAction(event, () => controller.continueNative())}>Continue Reading</button>
    {:else}
      <button type="button" class="rounded border px-4 py-2" disabled={blocked} onclick={event => plainAction(event, startFirst)}>Start Reading</button>
    {/if}
    {#if $localLibraryState.mode === 'device' || !publicUuidLibraryEnabled}
      <details>
        <summary>More reading options</summary>
        <button type="button" class="underline" disabled={blocked} onclick={event => plainAction(event, startFirst)}>Start from first chapter (ignore saved completion)</button>
      </details>
    {/if}
  </div>
  {#if $readerLibrarySelection.pending}
    <p class="text-muted-foreground text-xs">Wait for the selected library change to finish.</p>
  {:else if $localLibraryState.mode === 'device'}
    {#if !deviceHistoryReady}<p role="status">Device completion history is unavailable or still loading. It was not treated as empty. You can explicitly start from the first chapter without history.</p>{/if}
    <p class="text-muted-foreground text-xs">Automatic device Continue uses the retained complete aggregate where supported. If its 10,000-chapter limit is exceeded, narrow language/group filters or explicitly start from the first chapter. Device history is never sent to the server.</p>
    {#if deviceError}<p role="alert">{deviceError}</p>{/if}
  {:else if publicUuidLibraryEnabled}
    <p class="text-muted-foreground text-xs">Native-account Continue is unavailable in this public catalogue. Select device mode explicitly in Library to use device completion.</p>
  {:else}
    <p class="text-muted-foreground text-xs">Continue uses this native cookie account: oldest unread group, or newest if all eligible groups were completed in that request. Device history is not substituted. A later request may observe a changed catalogue or history.</p>
  {/if}
  <Status {controller} />
  {#if $controller.groupAnchor}<EditionPage {controller} />{/if}
</div>
