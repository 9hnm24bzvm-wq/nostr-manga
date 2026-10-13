<script lang="ts">
	import * as Comments from "$lib/components/comments";
	import NotFound from "$lib/components/ui/NotFound.svelte";
	import Thread from "./Thread.svelte";

	const { data } = $props();
</script>

{#if data.thread}
	<Comments.Provider thread={data.thread!}>
		<section class="m-auto w-full max-w-300 flex-auto space-y-5">
			<Thread />
			<Comments.Comments />
		</section>
	</Comments.Provider>
{:else}
	<NotFound />
{/if}
