<script lang="ts">
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import { onMount } from "svelte";
	import ErrorComponent from "./ErrorComponent.svelte";
	import ReportSearchEntry from "./ReportSearchEntry.svelte";
	import Grid from "./ui/Grid.svelte";
	import { Spinner } from "./ui/spinner";

	let {
		feed = $bindable(),
		onSearch
	}: {
		feed: ReportFeedResponse | undefined;
		onSearch: () => Promise<void>;
	} = $props();

	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const state = page.state as {
			_$ReportSearch_feed: typeof feed;
		};
		if (state._$ReportSearch_feed) {
			feed = state._$ReportSearch_feed;
		} else {
			try {
				await onSearch();
			} catch (e) {
				if (e instanceof Constants.Error.Error) error = e;
				else {
					console.error(e);
					error = Constants.Error.Unknown;
				}
			}
		}
		isMounted = true;
	});
</script>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted}
	{#if feed?.data?.length}
		<Grid class="gap-1" breakpoints={{ 0: 3, 1400: 2, 920: 1 }}>
			{#each feed.data as report, i (report)}
				<ReportSearchEntry bind:entries={feed.data} bind:report={feed.data[i]} />
			{/each}
		</Grid>
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No results found</Empty.Title>
				{#if window.location.search}
					<Empty.Description>Try adjusting your search or using different keywords.</Empty.Description>
				{:else}
					<Empty.Description>You haven't submitted any reports yet.</Empty.Description>
				{/if}
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
