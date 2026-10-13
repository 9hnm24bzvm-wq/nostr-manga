<script lang="ts">
	import * as Comments from "$lib/components/comments";
	import * as Reader from "$lib/components/reader";
	import NotFound from "$lib/components/ui/NotFound.svelte";

	const { data } = $props();
</script>

{#key data.chapter}
	{#if data.chapter}
		<Comments.Provider thread={data.chapter.relationships.thread}>
			<Reader.Provider chapter={data.chapter}>
				<Reader.Reader />
			</Reader.Provider>
		</Comments.Provider>
	{:else}
		<NotFound />
	{/if}
{/key}
