<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { onMount } from "svelte";
	import { setComments } from "./context.svelte";

	const {
		thread,
		children
	}: {
		thread: Thread;
		children: any;
	} = $props();

	// svelte-ignore state_referenced_locally
	let stats = $state(thread.relationships.stats);
	let voteState = $state<number>();
	let feed = $state<ReplyFeedResponse>();

	setComments({
		// svelte-ignore state_referenced_locally
		thread,
		stats: () => stats,
		setStats: (value: ThreadStats) => (stats = value),
		voteState: () => voteState,
		setVoteState: (value: number | undefined) => (voteState = value),
		feed: () => feed,
		setFeed: (value: ReplyFeedResponse | undefined) => (feed = value)
	});

	onMount(() => {
		const targetPage = (page.data as any).targetPage;
		if (targetPage) {
			const q = new URLSearchParams(window.location.search);
			q.set("page", String(targetPage));

			const target = window.location.pathname + q.toString();
			if (target !== window.location.href) {
				replaceState(target, page.state);
			}
		}
	});
</script>

{@render children()}
