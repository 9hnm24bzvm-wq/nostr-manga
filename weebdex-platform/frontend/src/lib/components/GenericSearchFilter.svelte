<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { SearchIcon } from "@lucide/svelte";
	import { Button } from "./ui/button";

	let {
		feed = $bindable(),
		showCount
	}: {
		feed: FeedResponse<any> | undefined;
		showCount?: boolean;
	} = $props();

	let query = $state<string>(page.url.searchParams.get("name") ?? "");

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();

		const q = new URLSearchParams();
		if (query.length) q.set("name", query);
		q.delete("page");

		const target = window.location.pathname + q.toString();
		if (target !== window.location.pathname + window.location.search) {
			feed = undefined;
			goto(target);
		}
	};
</script>

<form class="space-y-2" onsubmit={onSubmit}>
	<InputGroup.Root class="w-full border-none py-5 !ring-0 !ring-transparent">
		<InputGroup.Input class="truncate" placeholder="Search..." autocomplete="off" bind:value={query} />
		<InputGroup.Addon>
			<SearchIcon />
		</InputGroup.Addon>
		{#if feed?.total && showCount}
			<InputGroup.Addon align="inline-end">{feed.total.short()}</InputGroup.Addon>
		{/if}
	</InputGroup.Root>
	<div class="flex items-center justify-end">
		<Button variant="default" type="submit">
			<SearchIcon />
			Search
		</Button>
	</div>
</form>
