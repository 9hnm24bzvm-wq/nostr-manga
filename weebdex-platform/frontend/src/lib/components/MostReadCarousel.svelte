<script lang="ts" module>
	let feed = $state<MangaFeedResponse | undefined>();
</script>

<script lang="ts">
	import { API, Constants } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import Carousel from "$lib/components/ui/Carousel.svelte";
	import { prefs } from "$lib/stores";
	import { ArrowRightIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import MostReadCarouselEntry from "./MostReadCarouselEntry.svelte";
	import { AspectRatio } from "./ui/aspect-ratio";
	import Skeleton from "./ui/skeleton/skeleton.svelte";

	let isMounted = $state(false);
	onMount(async () => {
		if (!feed) {
			try {
				const q = new URLSearchParams();
				q.set("rank", "read");
				q.set("time", "30d");

				q.append("contentRating", Constants.Manga.ContentRating.Safe);
				if ($prefs.show_suggestive) q.append("contentRating", Constants.Manga.ContentRating.Suggestive);
				if ($prefs.show_erotic) q.append("contentRating", Constants.Manga.ContentRating.Erotica);
				if ($prefs.show_hentai) q.append("contentRating", Constants.Manga.ContentRating.Pornographic);

				const res = await API.Manga.Trending(q);
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
			<h2 class="w-0 flex-auto text-xl font-medium">Most Read Titles</h2>
			<div class="flex shrink-0 items-center gap-2">
				<a class={buttonVariants({ variant: "ghost", size: "icon" })} href="/popular">
					<ArrowRightIcon class="size-6" />
				</a>
			</div>
		</header>
		{#if isMounted && feed?.data?.length}
			<Carousel class="!m-0 !p-0">
				{#each feed.data as manga, i (manga)}
					<MostReadCarouselEntry {manga} index={i} />
				{/each}
			</Carousel>
		{:else}
			<div class="relative p-5">
				<figure class="w-[35%] shrink-0 sm:w-50">
					<AspectRatio ratio={10 / 14} />
				</figure>
				<Skeleton class="bg-card absolute inset-0"></Skeleton>
			</div>
		{/if}
	</section>
{/if}
