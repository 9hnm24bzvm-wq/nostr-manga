<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { libraryIdentity, localLibraryState, openLibrary } from './library';
  import LoginRequired from '$lib/components/ui/LoginRequired.svelte';
  const { children, isAction }: { children?: Snippet; isAction?: boolean } = $props();
  onMount(() => { void openLibrary().catch(() => {}); });
</script>
{#if $libraryIdentity.canUse}
  {@render children?.()}
{:else if $localLibraryState.error}
  <p role="alert">{$localLibraryState.error}</p>
{:else if $localLibraryState.ready && $localLibraryState.mode === 'account'}
  <LoginRequired {isAction} />
{:else if $localLibraryState.ready && $localLibraryState.mode === 'device' && $localLibraryState.scopeStatus !== 'verified'}
  <p role="alert">{$localLibraryState.scopeReason} Your saved records are preserved. <a class="underline" href="/settings/preferences#local-library">Manage your library or export a backup</a>.</p>
{:else}
  <p role="status">Opening this device's library…</p>
{/if}
