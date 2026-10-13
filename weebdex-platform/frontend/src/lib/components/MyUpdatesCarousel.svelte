<script lang="ts" module>
	let feed = $state<MappedChapterFeedResponse>();
	let entries = $state<Chapter[]>([]);
	let idMap = $state<Record<string, undefined>>({});
</script>

<script lang="ts">
	import { API, Constants } from "$lib";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import Carousel from "$lib/components/ui/Carousel.svelte";
	import { PersistentStore } from "$lib/hooks";
	import { prefs } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { ArrowRightIcon, ChevronUp } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import LatestUpdatesGridEntry from "./LatestUpdatesGridEntry.svelte";
	import LatestUpdatesGridSkeleton from "./LatestUpdatesGridSkeleton.svelte";
	import MangaSearchEntryCard from "./MangaSearchEntryCard.svelte";
	import MangaSearchEntrySkeleton from "./MangaSearchEntrySkeleton.svelte";

	let isMounted = $state(false);
	const isCollapsed = new PersistentStore("ivtnx7u8", false);

	onMount(async () => {
		if (!feed?.data?.length) {
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);
			prefs.applyChapterPreferences(q);
			q.set("limit", "10");

			const res = await API.MangaFollow.LatestUpdates(q);
			if (res?.status === StatusCodes.OK && res.body) {
				feed = res.body;
				if (feed?.data?.length) {
					for (const chapter of feed.data) {
						const id = chapter.relationships.manga.id;
						if (!(id in idMap)) {
							idMap[id] = undefined;
							entries.push(chapter);
						}
					}
					entries = entries;
				}
			}
		}
		isMounted = true;
	});
</script>

{#if !isMounted || entries.length}
	<section>
		<header class="flex items-center gap-2" class:mb-3={!$isCollapsed}>
			<div class="flex w-0 flex-auto items-center gap-2">
				<h2 class="text-xl font-medium">My Updates</h2>
				<Button
					class="relative"
					type="button"
					variant="ghost"
					size="icon"
					onclick={() => ($isCollapsed = !$isCollapsed)}
				>
					<ChevronUp class={cn("size-6 transition-all", $isCollapsed && "-rotate-180")} />
				</Button>
			</div>
			<div class="flex shrink-0 items-center gap-2">
				<a class={buttonVariants({ variant: "ghost", size: "icon" })} href="/library/updates">
					<ArrowRightIcon class="size-6" />
				</a>
			</div>
		</header>
		<div class:hidden={$isCollapsed}>
			{#if isMounted && entries.length}
				<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>
					{#each entries as chapter (chapter)}
						<article class="swiper-slide relative">
							<MangaSearchEntryCard manga={chapter.relationships.manga} isGrid />
							<LatestUpdatesGridEntry {chapter} />
						</article>
					{/each}
				</Carousel>
			{:else}
				<Carousel slidesPerView={2} breakpoints={Constants.MangaSliderBreakpoints} spaceBetween={12}>
					{#each Array.from({ length: 10 })}
						<article class="swiper-slide relative">
							<MangaSearchEntrySkeleton isGrid />
							<LatestUpdatesGridSkeleton />
						</article>
					{/each}
				</Carousel>
			{/if}
		</div>
	</section>
{/if}
