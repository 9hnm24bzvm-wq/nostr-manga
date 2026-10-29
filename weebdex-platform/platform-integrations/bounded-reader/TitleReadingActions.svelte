<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import { createBoundedReader, titleFilters } from './runtime.svelte';
  import { plainAction } from './presentation';
  import { localLibraryState, readerLibrarySelection } from '../local-library/library';
  import { nativeAuthority } from '../frontend-behavior/mutation-authority';
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
    {:else}
      <button type="button" class="rounded border px-4 py-2" disabled={blocked || !$nativeAuthority.actor} onclick={event => plainAction(event, () => controller.continueNative())}>Continue Reading</button>
    {/if}
      <details>
        <summary>More reading options</summary>
        <button type="button" class="underline" disabled={blocked} onclick={event => plainAction(event, startFirst)}>Start from first chapter</button>
      </details>
  </div>
  {#if $readerLibrarySelection.pending}
    <p class="text-muted-foreground text-xs">Saving your library choice…</p>
  {:else if $localLibraryState.mode === 'device'}
    {#if !deviceHistoryReady}<p role="status">Your reading history is still loading or unavailable. You can start from the first chapter instead.</p>{/if}
    <p class="text-muted-foreground text-xs">Continue picks up after your completed chapters. Finish a chapter to save it to History on this device. Your device history is never sent to the server.</p>
    {#if deviceError}<p role="alert">{deviceError}</p>{/if}
  {:else}
    <p class="text-muted-foreground text-xs">Continue uses your signed-in account’s completed chapters and resumes its saved page when available, not this device’s history.</p>
  {/if}
  <Status {controller} />
  {#if $controller.groupAnchor}<EditionPage {controller} />{/if}
</div>
