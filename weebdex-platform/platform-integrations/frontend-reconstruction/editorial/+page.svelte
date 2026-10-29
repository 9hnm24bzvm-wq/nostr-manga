<script lang="ts">
 import {page} from '$app/state';
 import {env} from '$env/dynamic/public';
 import PublicEditorialTools from '$publicApi/PublicEditorialTools.svelte';
 import {publicUuid} from '$publicApi/validation';
 const kind=$derived(page.url.searchParams.get('kind')==='group'?'group':'author');
 const target=$derived(page.url.searchParams.has('id')?publicUuid(page.url.searchParams.get('id')):undefined);
</script>
<svelte:head><title>Catalogue tools</title></svelte:head>
{#if env.PUBLIC_MANGADEX_CATALOGUE==='1'}{#key `${kind}:${target ?? "new"}`}<PublicEditorialTools {kind} {target}/>{/key}{:else}<p>Catalogue tools are unavailable in this deployment.</p>{/if}
