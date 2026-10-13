<script lang="ts">
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import Grid from "$lib/components/ui/Grid.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import { mangaView, prefs } from "$lib/stores";
	import { onMount } from "svelte";
	import ErrorComponent from "./ErrorComponent.svelte";
	import MangaSearchEntryCard from "./MangaSearchEntryCard.svelte";
	import MangaSearchEntrySkeleton from "./MangaSearchEntrySkeleton.svelte";

	let {
		feed = $bindable(),
		isRelationSearch,
		onSearch,
		onClick
	}: {
		feed: MangaFeedResponse | MangaFollowFeedResponse | undefined;
		isRelationSearch?: boolean;
		onSearch: () => Promise<void>;
		onClick?: (manga: Manga) => void;
	} = $props();

	let isGrid = $derived(isRelationSearch || $mangaView === "grid");
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const state = page.state as {
			_$MangaSearch_feed: typeof feed;
		};
		if (state._$MangaSearch_feed) {
			feed = state._$MangaSearch_feed;
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
{:else if isMounted && feed}
	{#if feed.data?.length}
		<Grid class="gap-3" breakpoints={isGrid ? Constants.MangaGridBreakpoints : { 0: 2, 768: 1 }}>
			{#each feed.data as manga (manga)}
				<article>
					<MangaSearchEntryCard {manga} {isGrid} isEdit={isRelationSearch} {onClick} />
				</article>
			{/each}
		</Grid>
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No results found</Empty.Title>
				<Empty.Description>
					{#if window.location.search}
						Try adjusting your search or using different keywords.
					{:else if window.location.pathname.startsWith("/library")}
						<p>The database may still be updating, or you haven't added anything to your library yet.</p>
						<br />
						<p>Any titles you add to your library will appear here shortly.</p>
					{:else}
						The database may still be updating, or no entries matching your filters were found.
					{/if}
				</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<Grid class="gap-3" breakpoints={isGrid ? Constants.MangaGridBreakpoints : { 0: 2, 768: 1 }}>
		{#each Array.from({ length: feed?.limit ?? Number($prefs.items_per_page) })}
			<MangaSearchEntrySkeleton {isGrid} />
		{/each}
	</Grid>
{/if}
