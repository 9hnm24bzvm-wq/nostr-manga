<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { libraryIdentity, localLibraryState, openLibrary } from './library';
  import {cataloguePreviewLibraryUnavailable, CATALOGUE_PREVIEW_LIBRARY_FENCE, publicAccountLibraryUnavailable} from '../public-api/library-staging';
  import LoginRequired from '$lib/components/ui/LoginRequired.svelte';
  const { children, isAction }: { children?: Snippet; isAction?: boolean } = $props();
  onMount(() => { void openLibrary().catch(() => {}); });
</script>
{#if $libraryIdentity.canUse}
  {@render children?.()}
{:else if cataloguePreviewLibraryUnavailable}
  <p role="alert">{CATALOGUE_PREVIEW_LIBRARY_FENCE} <a class="underline" href="/settings/library">Manage your library or export a backup</a>.</p>
{:else if $localLibraryState.error}
  <p role="alert">{$localLibraryState.error}</p>
{:else if publicAccountLibraryUnavailable && $localLibraryState.ready && $localLibraryState.mode === 'account'}
  <p role="alert">Choose your device library to save titles and progress in this catalogue. <a class="underline" href="/settings/library">Use device library</a>.</p>
{:else if $localLibraryState.ready && $localLibraryState.mode === 'account'}
  <LoginRequired {isAction} />
{:else if $localLibraryState.ready && $localLibraryState.mode === 'device' && $localLibraryState.scopeStatus !== 'verified'}
  <p role="alert">{$localLibraryState.scopeReason} Your saved records are preserved. <a class="underline" href="/settings/library">Manage your library or export a backup</a>.</p>
{:else}
  <p role="status">Opening this device's library…</p>
{/if}
