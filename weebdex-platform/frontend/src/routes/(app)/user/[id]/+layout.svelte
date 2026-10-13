<script lang="ts">
	import * as Comments from "$lib/components/comments";
	import NotFound from "$lib/components/ui/NotFound.svelte";
	import UserComponent from "./User.svelte";

	const { data, children } = $props();
</script>

{#key data.user}
	{#if data.user}
		<Comments.Provider thread={data.user.relationships!.thread!}>
			<UserComponent data={data.user}>
				{@render children?.()}
			</UserComponent>
		</Comments.Provider>
	{:else}
		<NotFound />
	{/if}
{/key}
