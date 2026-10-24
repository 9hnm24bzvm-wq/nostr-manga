<script lang="ts">
 import type { Snippet } from 'svelte';
 import { page } from '$app/state';
 import ProtectedVerified from '$lib/components/ui/ProtectedVerified.svelte';
 let {children}:{children?:Snippet}=$props();
</script>
{#if page.url.pathname==='/drafts/title'}{@render children?.()}{:else}<ProtectedVerified>{@render children?.()}</ProtectedVerified>{/if}
