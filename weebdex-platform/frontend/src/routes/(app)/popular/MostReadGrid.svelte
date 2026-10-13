<script lang="ts">
	import { API, Constants } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import MangaSearchEntryCard from "$lib/components/MangaSearchEntryCard.svelte";
	import MangaSearchEntrySkeleton from "$lib/components/MangaSearchEntrySkeleton.svelte";
	import Carousel from "$lib/components/ui/Carousel.svelte";
	import Grid from "$lib/components/ui/Grid.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import { prefs } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	let { time = "30d" } = $props();
	let feed = $state<MangaFeedResponse>();

	let isMounted = $state(false);
	let isLoading = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		await load();
		isMounted = true;
	});

	const load = async () => {
		if (isLoading) {
			return;
		}

		const { pathname } = window.location;
		isLoading = true;
		try {
			const q = new URLSearchParams();
			q.set("rank", "read");
			q.set("time", time);

			q.append("contentRating", Constants.Manga.ContentRating.Safe);
			if ($prefs.show_suggestive) q.append("contentRating", Constants.Manga.ContentRating.Suggestive);
			if ($prefs.show_erotic) q.append("contentRating", Constants.Manga.ContentRating.Erotica);
			if ($prefs.show_hentai) q.append("contentRating", Constants.Manga.ContentRating.Pornographic);

			const res = await API.Manga.Trending(q);
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				feed = res.body;
			}
		} catch (e) {
			if (isMounted) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} else {
				if (e instanceof Constants.Error.Error) error = e;
				else {
					console.error(e);
					error = Constants.Error.Unknown;
				}
			}
		} finally {
			isLoading = false;
		}
	};
</script>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted}
	{#if feed?.data?.length}
		<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>
			{#each feed.data as manga (manga)}
				<article class="swiper-slide relative">
					<MangaSearchEntryCard {manga} isGrid />
				</article>
			{/each}
		</Carousel>
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No updates found</Empty.Title>
				<Empty.Description>
					The database may still be updating, or no entries matching your filters were found.
				</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<Grid class="gap-3" breakpoints={Constants.MangaGridBreakpoints}>
		{#each Array.from({ length: 21 })}
			<article>
				<MangaSearchEntrySkeleton isGrid />
			</article>
		{/each}
	</Grid>
{/if}
