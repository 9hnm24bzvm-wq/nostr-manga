<script lang="ts" module>
	let feed = $state<MangaFeedResponse>();
</script>

<script lang="ts">
	import { API, Constants } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import Carousel from "$lib/components/ui/Carousel.svelte";
	import { prefs } from "$lib/stores";
	import { ArrowRightIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import MangaSearchEntryCard from "./MangaSearchEntryCard.svelte";
	import MangaSearchEntrySkeleton from "./MangaSearchEntrySkeleton.svelte";

	let isMounted = $state(false);

	onMount(async () => {
		if (!feed?.data) {
			try {
				const q = new URLSearchParams();
				prefs.applyMangaPreferences(q);

				q.set("sort", Constants.Manga.SortBy.CreatedAt);
				q.set("limit", "10");

				const res = await API.Manga.Search(q);
				if (res?.status === StatusCodes.OK && res.body) {
					feed = res.body;
				}
			} catch (e) {
				console.error(e);
			}
		}
		isMounted = true;
	});
</script>

{#if !isMounted || feed?.data?.length}
	<section>
		<header class="mb-3 flex items-center gap-2">
			<div class="flex w-0 flex-auto items-center gap-2">
				<h2 class="text-xl font-medium">Recently Added Titles</h2>
			</div>
			<div class="flex shrink-0 items-center gap-2">
				<a class={buttonVariants({ variant: "ghost", size: "icon" })} href="/search?sort=createdAt">
					<ArrowRightIcon class="size-6" />
				</a>
			</div>
		</header>
		<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>
			{#if isMounted && feed?.data?.length}
				{#each feed.data as manga (manga)}
					<article class="swiper-slide relative">
						<MangaSearchEntryCard {manga} isGrid />
					</article>
				{/each}
			{:else}
				<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>
					{#each Array.from({ length: 10 })}
						<article class="swiper-slide relative">
							<MangaSearchEntrySkeleton isGrid />
						</article>
					{/each}
				</Carousel>
			{/if}
		</Carousel>
	</section>
{/if}
