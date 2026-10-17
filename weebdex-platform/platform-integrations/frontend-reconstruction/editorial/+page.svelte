<script lang="ts">
 import {page} from '$app/state';
 import {env} from '$env/dynamic/public';
 import PublicEditorialTools from '$publicApi/PublicEditorialTools.svelte';
 import {publicUuid} from '$publicApi/validation';
 const kind=$derived(page.url.searchParams.get('kind')==='group'?'group':'author');
 const target=$derived(page.url.searchParams.has('id')?publicUuid(page.url.searchParams.get('id')):undefined);
</script>
{#if env.PUBLIC_MANGADEX_CATALOGUE==='1'}{#key page.url.href}<PublicEditorialTools {kind} {target}/>{/key}{:else}<p>Signed catalogue tools are unavailable in this deployment.</p>{/if}
