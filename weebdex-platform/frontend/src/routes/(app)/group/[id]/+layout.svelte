<script lang="ts">
	import * as Comments from "$lib/components/comments";
	import NotFound from "$lib/components/ui/NotFound.svelte";
	import Group from "./Group.svelte";

	const { data, children } = $props();
</script>

{#key data.group}
	{#if data.group}
		<Comments.Provider thread={data.group.relationships!.thread!}>
			<Group data={data.group}>
				{@render children?.()}
			</Group>
		</Comments.Provider>
	{:else}
		<NotFound />
	{/if}
{/key}
