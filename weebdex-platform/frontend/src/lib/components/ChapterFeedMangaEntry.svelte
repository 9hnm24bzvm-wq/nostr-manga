<script lang="ts">
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { Locale, Utils } from "$lib";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import { Button } from "$lib/components/ui/button";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { setManga } from "$lib/Context";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import ChapterFeedChapterEntry from "./ChapterFeedChapterEntry.svelte";

	const {
		manga
	}: {
		manga: Manga;
	} = $props();

	setManga(() => manga);

	// svelte-ignore state_referenced_locally
	const cover = manga.relationships.cover
		? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.${manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256"}.webp`
		: "/images/cover.webp";
	// svelte-ignore state_referenced_locally
	const slug = Utils.makeSlug(manga.title);

	// svelte-ignore state_referenced_locally
	let isFollowed = $state($Session?.follows && manga.id in $Session.follows);
	const isPrivate = window.location.pathname.startsWith("/library");

	let isTrimmed = $state(true);
	const chapters = $derived(isTrimmed ? manga.relationships._chapters!.slice(0, 3) : manga.relationships._chapters);
</script>

<article class="flex gap-2 border-t-2 py-2">
	<figure class="w-20 shrink-0">
		<AspectRatio
			ratio={10 / 14}
			class={cn("bg-card overflow-hidden rounded-md", {
				"ring-accent ring-2": isFollowed && !isPrivate
			})}
		>
			{#snippet child({ props })}
				<a href="/title/{manga.id}{slug}" title={manga.title} {...props}>
					<ImageLoader class="size-full object-cover object-center" src={cover} alt={manga.title} />
					<div class="gradient pointer-events-none absolute inset-0"></div>
					<ImageLoader
						class="absolute top-0 left-0 -mt-[0.25rem] size-6"
						src="/images/flags/{Locale.to3166(manga.language)}.svg"
						alt={Locale.toLanguage(manga.language)}
					/>
				</a>
			{/snippet}
		</AspectRatio>
	</figure>
	<div class="w-0 flex-auto space-y-1">
		<h2 class="truncate font-semibold">
			<a class="hover:text-accent-foreground" href="/title/{manga.id}{slug}" title={manga.title}>
				{manga.title}
			</a>
		</h2>
		<div class="space-y-0.5">
			{#each chapters as chapter}
				<ChapterFeedChapterEntry class="bg-input/30 hover:bg-input/55 rounded-md" {chapter} />
			{/each}
		</div>
		{#if manga.relationships._chapters!.length > 3}
			<Button
				class="w-full border-none text-xs tracking-wider uppercase"
				variant="outline"
				size="sm"
				onclick={() => (isTrimmed = !isTrimmed)}
			>
				{#if isTrimmed}
					Show More
				{:else}
					Show Less
				{/if}
			</Button>
		{/if}
	</div>
</article>

<style lang="scss">
	.gradient {
		background-image: linear-gradient(300deg, transparent 80%, rgba(0, 0, 0, 0.5));
	}
</style>
