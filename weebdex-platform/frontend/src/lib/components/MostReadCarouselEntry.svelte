<script lang="ts">
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { Constants, Typography, Utils } from "$lib";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import ImageLoader from "./ui/ImageLoader.svelte";
	import ScrollArea from "./ui/scroll-area/scroll-area.svelte";

	const { manga, index }: { manga: Manga; index: number } = $props();
	// svelte-ignore state_referenced_locally
	const cover = manga.relationships.cover
		? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.512.webp`
		: "/images/cover.webp";
	// svelte-ignore state_referenced_locally
	const fullCover = manga.relationships.cover
		? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.webp`
		: "/images/cover.webp";
</script>

<article class="bg-card swiper-slide relative">
	<a href="/title/{manga.id}{Utils.makeSlug(manga.title)}">
		<div class="absolute inset-0 overflow-hidden">
			<ImageLoader class="size-full object-cover object-center" src={fullCover} alt={manga.title} />
			<div class="absolute inset-0 bg-linear-to-t from-black/50 to-black/0"></div>
		</div>
		<div class="relative flex h-full gap-5 overflow-hidden bg-black/50 p-5 backdrop-blur-xs">
			<figure class="w-[35%] shrink-0 sm:w-50">
				<AspectRatio ratio={10 / 14} class="bg-card overflow-hidden rounded-md">
					<ImageLoader class="size-full object-cover object-center" src={cover} alt={manga.title} />
				</AspectRatio>
			</figure>
			<div class="flex w-0 flex-auto flex-col text-white">
				<h3 class="line-clamp-2 shrink-0 text-xl/6 font-semibold sm:text-2xl/7 md:text-3xl/8">{manga.title}</h3>
				<div class="flex h-0 flex-auto flex-col space-y-2.5">
					{#if manga.relationships.authors?.length || manga.relationships.artists?.length}
						<em class="mb-4 block text-lg font-medium">
							{#each Array.from(new Set( [...(manga.relationships.authors?.map(a => a.name) ?? []), ...(manga.relationships.artists?.map(a => a.name) ?? [])] )) as author, i}
								{#if i > 0}
									{", "}
								{/if}
								{author}
							{/each}
						</em>
					{/if}
					<div class="flex flex-wrap gap-1">
						<span
							class="rounded px-1 font-medium"
							class:!bg-green-700={manga.status === Constants.Manga.PublicationStatus.Ongoing}
							class:!bg-sky-700={manga.status === Constants.Manga.PublicationStatus.Completed}
							class:!bg-orange-700={manga.status === Constants.Manga.PublicationStatus.Hiatus}
							class:!bg-red-700={manga.status === Constants.Manga.PublicationStatus.Cancelled}
						>
							{Constants.Manga.PublicationStatusText[manga.status]}
						</span>
						{#if manga.content_rating != Constants.Manga.ContentRating.Safe}
							<span
								class="rounded px-1 font-medium"
								class:!bg-yellow-700={manga.content_rating === Constants.Manga.ContentRating.Suggestive}
								class:!bg-orange-700={manga.content_rating === Constants.Manga.ContentRating.Erotica}
								class:!bg-red-700={manga.content_rating === Constants.Manga.ContentRating.Pornographic}
							>
								{Constants.Manga.ContentRatingText[manga.content_rating]}
							</span>
						{/if}
						{#if manga.relationships.tags?.length}
							{#each manga.relationships.tags
								?.filter(tag => [Constants.Tag.Group.Content, Constants.Tag.Group.Genre].includes(tag.group))
								.slice(0, 5) as tag}
								<span class="rounded bg-neutral-900/75 px-1 font-medium">{tag.name}</span>
							{/each}
						{/if}
					</div>
					{#if manga.description}
						<ScrollArea class="h-0 flex-auto">
							<div class="typography space-y-2" {@attach Typography.exec}>
								{@html Utils.parseDescriptionPlain(manga.description)}
							</div>
						</ScrollArea>
					{/if}
					<strong class="-mr-2.5 -mb-2.5 block text-right text-2xl font-semibold">
						#{index + 1}
					</strong>
				</div>
			</div>
		</div>
	</a>
</article>

<style lang="scss">
	div :global(hr) {
		border-color: rgb(150, 150, 150) !important;
	}
</style>
