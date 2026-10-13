<script lang="ts">
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { API, Constants, Locale, Utils } from "$lib";
	import { cn } from "$lib/Utils";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import MangaSearchEntrySkeleton from "$lib/components/MangaSearchEntrySkeleton.svelte";
	import Grid from "$lib/components/ui/Grid.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import * as Empty from "$lib/components/ui/empty";
	import { prefs } from "$lib/stores";
	import { EyeIcon, MessagesSquare, ThumbsUpIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	let { time = "30d" } = $props();

	let feed = $state<MappedChapterFeedResponse>();

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
			q.set("time", time);

			q.append("contentRating", Constants.Manga.ContentRating.Safe);
			if ($prefs.show_suggestive) q.append("contentRating", Constants.Manga.ContentRating.Suggestive);
			if ($prefs.show_erotic) q.append("contentRating", Constants.Manga.ContentRating.Erotica);
			if ($prefs.show_hentai) q.append("contentRating", Constants.Manga.ContentRating.Pornographic);

			const res = await API.Chapter.Trending(q);
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
		<Grid class="gap-3" breakpoints={{ 0: 3 }}>
			{#each feed.data as chapter (chapter)}
				{@const manga = feed.map.manga[chapter.relationships.manga.id]}
				{@const cover = manga.relationships.cover
					? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.${manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256"}.webp`
					: "/images/cover.webp"}
				<article class="flex gap-2">
					<figure class="w-15 shrink-0">
						<AspectRatio ratio={10 / 14} class={cn("bg-card overflow-hidden rounded-md", {})}>
							{#snippet child({ props })}
								<a href="/title/{manga.id}{Utils.makeSlug(manga.title)}" title={manga.title} {...props}>
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
					<div class="text-muted-foreground w-0 flex-auto space-y-1">
						<div>
							<h2 class="truncate font-semibold">
								<a
									class="text-foreground hover:text-accent-foreground"
									href="/title/{manga.id}{Utils.makeSlug(manga.title)}"
									title={manga.title}
								>
									{manga.title}
								</a>
							</h2>
							<h4 class="truncate">
								<a
									class="text-foreground/85 hover:text-accent-foreground"
									href="/chapter/{chapter.id}"
									title={Utils.chapterHeadline(chapter)}
								>
									{Utils.chapterHeadline(chapter)}
								</a>
							</h4>
						</div>
						<div class="flex items-center gap-2">
							<div class="flex items-center gap-1">
								<EyeIcon size="16" />
								<span>{chapter.relationships.stats?.views.short()}</span>
							</div>
							<div class="flex items-center gap-1">
								<MessagesSquare size="16" />
								<span>{chapter.relationships.thread.relationships.stats.replies.short()}</span>
							</div>
							<div class="flex items-center gap-1">
								<ThumbsUpIcon size="16" />
								<span>{chapter.relationships.thread.relationships.stats.up.short()}</span>
							</div>
						</div>
					</div>
				</article>
			{/each}
		</Grid>
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
