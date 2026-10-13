<script lang="ts">
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { Constants, Utils } from "$lib";
	import ImageLoader from "./ui/ImageLoader.svelte";
	import { badgeVariants } from "./ui/badge";

	const { manga }: { manga: Manga } = $props();
	// svelte-ignore state_referenced_locally
	const cover = manga.relationships.cover
		? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.${manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256"}.webp`
		: "/images/cover.webp";
</script>

<article>
	<a
		class="bg-input/30 hover:bg-input/50 flex gap-2 overflow-hidden rounded-md p-2"
		href="/title/{manga.id}{Utils.makeSlug(manga.title)}"
	>
		<div class="w-14 shrink-0">
			<figure class="relative overflow-hidden rounded-md pt-[140%]">
				<ImageLoader class="absolute inset-0 size-full object-cover object-center" src={cover} alt={manga.title} />
			</figure>
		</div>
		<div class="w-0 flex-auto">
			<h3 class="mb-1 line-clamp-2 text-lg/6 font-semibold text-balance break-words" title={manga.title}>
				{manga.title}
			</h3>
			<div class="flex flex-wrap items-center gap-1 text-xs">
				<span
					class={badgeVariants({ class: "border-none px-1 py-0 text-white", variant: "outline" })}
					class:!bg-green-700={manga.status === Constants.Manga.PublicationStatus.Ongoing}
					class:!bg-sky-700={manga.status === Constants.Manga.PublicationStatus.Completed}
					class:!bg-orange-700={manga.status === Constants.Manga.PublicationStatus.Hiatus}
					class:!bg-red-700={manga.status === Constants.Manga.PublicationStatus.Cancelled}
				>
					{Constants.Manga.PublicationStatusText[manga.status]}</span
				>
				{#if manga.content_rating != Constants.Manga.ContentRating.Safe}
					<span
						class={badgeVariants({ class: "border-none px-1 py-0 text-white", variant: "outline" })}
						class:!bg-yellow-700={manga.content_rating === Constants.Manga.ContentRating.Suggestive}
						class:!bg-orange-700={manga.content_rating === Constants.Manga.ContentRating.Erotica}
						class:!bg-red-700={manga.content_rating === Constants.Manga.ContentRating.Pornographic}
					>
						{Constants.Manga.ContentRatingText[manga.content_rating]}
					</span>
				{/if}
				{#if manga.demographic}
					<span
						class={badgeVariants({
							class: "bg-input/75 text-foreground/85 border-none px-1 py-0",
							variant: "outline"
						})}
					>
						{Constants.Manga.DemographicText[manga.demographic]}
					</span>
				{/if}
				{#if manga.relationships.tags?.length}
					{#each manga.relationships.tags
						?.filter(tag => [Constants.Tag.Group.Content, Constants.Tag.Group.Genre].includes(tag.group))
						.slice(0, 5) as tag}
						<span
							class={badgeVariants({
								class: "bg-input/75 text-foreground border-none px-1 py-0",
								variant: "outline"
							})}
						>
							{tag.name}
						</span>
					{/each}
				{/if}
			</div>
		</div>
	</a>
</article>
