<script lang="ts">
	import NotFound from "$lib/components/ui/NotFound.svelte";
	import Author from "./Author.svelte";
	const { data, children } = $props();
</script>

{#key data.author}
	{#if data.author}
		<Author data={data.author}>
			{@render children?.()}
		</Author>
	{:else}
		<NotFound />
	{/if}
{/key}
