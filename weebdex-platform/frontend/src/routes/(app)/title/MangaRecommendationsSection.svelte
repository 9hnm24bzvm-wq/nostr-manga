<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import MangaSearchEntryCard from "$lib/components/MangaSearchEntryCard.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import Grid from "$lib/components/ui/Grid.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import ViewModeToggles from "$lib/components/ViewModeToggles.svelte";
	import { getManga } from "$lib/Context";
	import { mangaView } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";

	const manga = $derived(getManga()());
	let feed = $state<MangaFeedResponse>();

	let isGrid = $derived($mangaView === "grid");
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		if ((page.state as any)?._$Recommendations_feed) {
			feed = (page.state as any)._$Recommendations_feed as typeof feed;
		} else {
			const { href } = window.location;
			try {
				const res = await API.Manga.Recommendations(manga.id);
				if (window.location.href !== href) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					feed = res.body;
					replaceState(page.url.href, {
						...page.state,
						_$Recommendations_feed: $state.snapshot(feed)
					});
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
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
{:else if isMounted && feed?.data?.length}
	<section class="space-y-2">
		<header class="flex items-center gap-2">
			<ViewModeToggles />
		</header>
		<Grid class="gap-3" breakpoints={isGrid ? Constants.MangaPageGridBreakpoints : { 0: 2, 768: 1 }}>
			{#each feed.data as manga (manga)}
				<article>
					<MangaSearchEntryCard {manga} {isGrid} />
				</article>
			{/each}
		</Grid>
	</section>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
