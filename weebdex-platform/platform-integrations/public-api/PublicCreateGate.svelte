<script lang="ts">
 import { page } from '$app/state';
 import type { Snippet } from 'svelte';
 import ProtectedVerified from '$lib/components/ui/ProtectedVerified.svelte';
 let {children}:{children?:Snippet}=$props();
</script>
{#if page.url.pathname==='/create/title'}
 {@render children?.()}
{:else}
 <ProtectedVerified>{@render children?.()}</ProtectedVerified>
{/if}
