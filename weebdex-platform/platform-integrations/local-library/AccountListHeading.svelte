<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { localLibraryState } from './library';
  import { nativeAuthority } from '../frontend-behavior/mutation-authority';
  import { accountLibrary } from './native-account-library';
  let { id }: { id: string } = $props();
  let live = $state(false), name = $state(''), error = $state(''), pending = $state(false), generation = 0;
  const context = () => JSON.stringify([id, $localLibraryState.mode, $nativeAuthority.actor, $nativeAuthority.epoch]);
  async function load() {
    const token = ++generation, key = context(), selected = id;
    name = ''; error = ''; pending = true;
    const check = () => { if (!live || generation !== token || context() !== key || $localLibraryState.mode !== 'account') throw Error('The selected account list changed.'); };
    try { check(); const access = await accountLibrary(check); check(); const result = await access.client.list(selected); access.check(); check(); name = result.data.name; }
    catch (cause) { if (live && generation === token && context() === key) error = cause instanceof Error ? cause.message : 'List name could not be loaded.'; }
    finally { if (live && generation === token) pending = false; }
  }
  $effect(() => { const selected = context(); if (live && id && $localLibraryState.mode === 'account' && $nativeAuthority.actor) void load(); else { generation++; name = ''; error = ''; pending = false; } });
  onMount(() => { live = true; });
  onDestroy(() => { live = false; generation++; });
</script>
{#if name}{name}{:else if pending}<span>Loading list…</span>{:else}<span>Selected list</span>{/if}
{#if error}<span class="block text-sm font-normal" role="alert">{error} <button type="button" class="underline" disabled={pending} onclick={load}>Retry list name</button></span>{/if}
