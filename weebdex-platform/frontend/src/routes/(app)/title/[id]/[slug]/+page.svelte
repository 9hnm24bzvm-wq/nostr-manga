<script lang="ts">
	import * as Comments from "$lib/components/comments";
	import NotFound from "$lib/components/ui/NotFound.svelte";
	import Manga from "../../Manga.svelte";

	const { data } = $props();
</script>

{#key data.manga}
	{#if data.manga}
		{#if data.manga.relationships.thread}
			<Comments.Provider thread={data.manga.relationships.thread}>
				<Manga manga={data.manga} />
			</Comments.Provider>
		{:else}
			<Manga manga={data.manga} />
		{/if}
	{:else}
		<NotFound />
	{/if}
{/key}
