<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_COVER_URL, PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, ExternalLink, Typography, Utils } from "$lib";
	import * as Comments from "$lib/components/comments";
	import { useComments } from "$lib/components/comments/context.svelte";
	import { Badge, badgeVariants } from "$lib/components/ui/badge";
	import * as Breadcrumb from "$lib/components/ui/breadcrumb/index.js";
	import { buttonVariants } from "$lib/components/ui/button";
	import Collapsible from "$lib/components/ui/Collapsible.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { useLightbox } from "$lib/components/ui/lightbox";
	import Scrollable from "$lib/components/ui/Scrollable.svelte";
	import { setManga, setReadHistory } from "$lib/Context";
	import { isMobile } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import {
		Bookmark,
		Expand,
		Eye,
		FilesIcon,
		Images,
		LinkIcon,
		MessageSquare,
		MessagesSquare,
		Star,
		StarIcon
	} from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import pluralize from "pluralize";
	import { onMount, tick } from "svelte";
	import { writable } from "svelte/store";
	import Analytics from "./Analytics.svelte";
	import MangaActions from "./MangaActions.svelte";
	import MangaChapterListSection from "./MangaChapterListSection.svelte";
	import MangaCoversSection from "./MangaCoversSection.svelte";
	import MangaRecommendationsSection from "./MangaRecommendationsSection.svelte";
	import MangaRelatedSection from "./MangaRelatedSection.svelte";

	const { manga }: { manga: Manga } = $props();
	const comments = useComments();
	const lightbox = useLightbox();

	const isPublished = $derived(manga.state === Constants.State.Published);
	const slug = $derived(Utils.makeSlug(manga.title));

	// svelte-ignore state_referenced_locally
	let mangaStats = $state<MangaStats | undefined>(manga.relationships.stats);
	const readHistory = writable<Record<string, Record<string, null>> | undefined>();

	setManga(() => manga);
	setReadHistory(readHistory);

	let figureRef = $state<HTMLElement>();
	// svelte-ignore state_referenced_locally
	const coverUrl = manga.relationships.cover
		? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.${manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256"}.webp`
		: "/images/cover.webp";
	let coverWidth = $derived(figureRef?.clientWidth ?? 256);
	const coverHeight = $derived(
		manga.relationships.cover && figureRef
			? Math.floor((manga.relationships.cover.dimensions[1] * coverWidth) / manga.relationships.cover.dimensions[0])
			: 364
	);

	const tabs: Record<
		string,
		{
			name: string;
			icon: any;
			component: any;
			hidden?: boolean;
		}
	> = $derived({
		chapters: {
			name: "Chapters",
			icon: FilesIcon,
			component: MangaChapterListSection
		},
		comments: {
			name: comments?.stats?.replies ? `Comments (${comments.stats.replies.short()})` : "Comments",
			icon: MessagesSquare,
			component: Comments.Comments,
			hidden: !isPublished || !manga.relationships.thread
		},
		covers: {
			name: "Covers",
			icon: Images,
			component: MangaCoversSection,
			hidden: !manga.relationships.cover
		},
		related: {
			name: "Related",
			icon: LinkIcon,
			component: MangaRelatedSection,
			hidden: !manga.relationships.relations?.length
		},
		recommendations: {
			name: "Recommendations",
			icon: StarIcon,
			component: MangaRecommendationsSection,
			hidden: !isPublished
		}
	});
	let tab = $derived.by(() => {
		let key = page.url.searchParams.get("tab") ?? "chapters";
		return key in tabs ? tabs[key] : undefined;
	});

	onMount(async () => {
		try {
			await Promise.all([getReadHistory(), getMangaStats()]);
		} catch (e) {
			console.error(e);
		}
	});

	$effect(() => {
		if (!tab || tab.hidden) {
			const q = new URLSearchParams();
			if (!isPublished) q.set("draft", "true");

			tick().then(() => {
				replaceState(window.location.pathname + q.toString(), page.state);
				tab = tabs["chapters"];
			});
		}
	});

	const getReadHistory = async () => {
		if (!isPublished) return;
		const res = await API.ChapterHistory.List(manga.id);
		if (res?.status === StatusCodes.OK && res.body) {
			$readHistory = res.body;
		}
	};

	const getMangaStats = async () => {
		if (!isPublished) return;
		const res = await API.Statistics.Manga(manga.id);
		if (res?.status === StatusCodes.OK && res.body) {
			mangaStats = res.body;
			manga.relationships.stats = res.body;
		}
	};
</script>

<svelte:window
	onresize={() => (coverWidth = figureRef?.clientWidth ?? 0)}
	ontransitionend={() => (coverWidth = figureRef?.clientWidth ?? 0)}
/>

<svelte:head>
	{#if manga}
		<title>{manga.title} - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

{#snippet cover()}
	<figure
		class="group bg-card relative overflow-hidden rounded-md"
		style="height: {coverHeight}px"
		bind:this={figureRef}
	>
		{#if manga.relationships.cover}
			{@const url = `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}${manga.relationships.cover.ext}`}
			<a
				href={url}
				onclick={e => {
					if (!e.ctrlKey) {
						e.preventDefault();
						e.stopPropagation();
						e.stopImmediatePropagation();

						lightbox.set(url);
						lightbox.open();
					}
				}}
			>
				<ImageLoader src={coverUrl} alt={manga.title} />
				<div
					class="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100"
				>
					<Expand class="text-white/85" size="40" />
				</div>
			</a>
		{:else}
			<ImageLoader src={coverUrl} alt={manga.title} />
		{/if}
	</figure>
{/snippet}

{#snippet header()}
	<Breadcrumb.Root class="mb-1">
		<Breadcrumb.List class="!gap-1 font-medium">
			<Breadcrumb.Item>
				<Breadcrumb.Link href="/search{new URLSearchParams({ lang: manga.language })}" class="type">
					{#if manga.language === "ja"}
						Manga
					{:else if manga.language === "ko"}
						Manhwa
					{:else if manga.language === "zh"}
						Manhua
					{:else}
						Comic
					{/if}
				</Breadcrumb.Link>
			</Breadcrumb.Item>
			{#if manga.year}
				<Breadcrumb.Separator />
				<Breadcrumb.Item>
					<Breadcrumb.Link
						href="/search{new URLSearchParams({
							yearFrom: manga.year.toString(),
							yearTo: manga.year.toString()
						}).toString()}"
					>
						{manga.year}
					</Breadcrumb.Link>
				</Breadcrumb.Item>
			{/if}
			<Breadcrumb.Separator />
			<Breadcrumb.Item class="status">
				<Breadcrumb.Link
					class={cn({
						"text-green-300": manga.status === Constants.Manga.PublicationStatus.Ongoing,
						"text-sky-300": manga.status === Constants.Manga.PublicationStatus.Completed,
						"text-orange-300": manga.status === Constants.Manga.PublicationStatus.Hiatus,
						"text-red-300": manga.status === Constants.Manga.PublicationStatus.Cancelled
					})}
					href="/search{new URLSearchParams({
						status: manga.status
					})}"
				>
					{Constants.Manga.PublicationStatusText[manga.status]}
				</Breadcrumb.Link>
			</Breadcrumb.Item>

			{#if manga.demographic}
				<Breadcrumb.Separator />
				<Breadcrumb.Item class="demographic">
					<Breadcrumb.Link
						href="/search{new URLSearchParams({
							demographic: manga.demographic
						})}"
					>
						{Constants.Manga.DemographicText[manga.demographic]}
					</Breadcrumb.Link>
				</Breadcrumb.Item>
			{/if}
			{#if manga.content_rating}
				<Breadcrumb.Separator />
				<Breadcrumb.Item class="contentRating">
					<Breadcrumb.Link
						class={cn({
							"text-green-300": manga.content_rating === Constants.Manga.ContentRating.Safe,
							"text-yellow-300": manga.content_rating === Constants.Manga.ContentRating.Suggestive,
							"text-orange-300": manga.content_rating === Constants.Manga.ContentRating.Erotica,
							"text-red-300": manga.content_rating === Constants.Manga.ContentRating.Pornographic
						})}
						href="/search{new URLSearchParams({
							contentRating: manga.content_rating
						})}"
					>
						{Constants.Manga.ContentRatingText[manga.content_rating]}
					</Breadcrumb.Link>
				</Breadcrumb.Item>
			{/if}
		</Breadcrumb.List>
	</Breadcrumb.Root>

	<div class="mb-5">
		<h1 class="text-xl/6 font-bold text-balance break-words sm:text-2xl/7 xl:text-3xl/8">{manga.title}</h1>
		{#if manga.alt_titles}
			<Scrollable
				class="text-muted-foreground mt-1 max-h-9 items-center overflow-auto text-[0.8125rem]/4.5 font-medium text-balance break-words"
			>
				{#each Object.values(manga.alt_titles).flat() as title, i}
					{#if i}
						<span class="mx-1 inline-block">/</span>
					{/if}
					<span class="hover:text-foreground focus:text-foreground">
						{title}
					</span>
				{/each}
			</Scrollable>
		{/if}
	</div>

	<MangaActions />

	{#if mangaStats && comments.stats && isPublished}
		<div class="mt-2.5 flex items-center gap-2.5 text-base">
			<span class="flex items-center gap-1 opacity-50">
				<Star size="20" />
				N/A
			</span>
			<span class="flex items-center gap-1">
				<Bookmark size="20" />
				{mangaStats.follows.short()}
			</span>
			<span class="flex items-center gap-1">
				<MessageSquare size="20" />
				{comments.stats.replies.short()}
			</span>
			<span class="flex items-center gap-1" class:opacity-50={!mangaStats.views}>
				<Eye size="20" />
				{#if mangaStats.views}
					{mangaStats.views.short()}
				{:else}
					N/A
				{/if}
			</span>
		</div>
	{/if}
{/snippet}

{#snippet synopsis()}
	<div>
		<strong class="font-semibold">Synopsis</strong>
		<div class="mt-1">
			{#if manga.description}
				<Collapsible class="max-h-25 overflow-hidden md:max-h-50">
					<div class="typography text-foreground/85 max-w-200 space-y-2" {@attach Typography.exec}>
						{@html Utils.parseDescription(manga.description)}
					</div>
				</Collapsible>
			{:else}
				<div class="typography text-foreground/85 max-w-200 space-y-2">
					<p>Oops! This manga doesn't have a description yet.</p>
				</div>
			{/if}
		</div>
	</div>
{/snippet}

{#snippet tags()}
	{#if manga.relationships.tags?.length || manga.relationships.authors?.length || manga.relationships.artists?.length || manga.last_chapter || manga.last_volume || manga.relationships.links}
		<ul class="space-y-1 md:space-y-2">
			{#if manga.relationships.authors}
				<li class="authors flex gap-2 md:flex-col md:gap-1">
					<strong class="w-25 shrink-0 font-semibold">{pluralize("Authors", manga.relationships.authors.length)}</strong
					>
					<div class="flex flex-wrap items-center gap-1">
						{#each manga.relationships.authors as author}
							<a
								class={badgeVariants({
									class: "bg-muted text-foreground/85 border-none text-sm font-normal",
									variant: "outline"
								})}
								href="/author/{author.id}">{author.name}</a
							>
						{/each}
					</div>
				</li>
			{/if}
			{#if manga.relationships.artists}
				<li class="artists flex gap-2 md:flex-col md:gap-1">
					<strong class="w-25 shrink-0 font-semibold">{pluralize("Artists", manga.relationships.artists.length)}</strong
					>
					<div class="flex flex-wrap items-center gap-1">
						{#each manga.relationships.artists as artist}
							<a
								class={badgeVariants({
									class: "bg-muted text-foreground/85 border-none text-sm font-normal",
									variant: "outline"
								})}
								href="/author/{artist.id}">{artist.name}</a
							>
						{/each}
					</div>
				</li>
			{/if}

			{#if manga.relationships.tags}
				{#each Object.entries(Utils.groupTags(manga.relationships.tags)) as [group, tags]}
					<li class="tags flex gap-2 md:flex-col md:gap-1">
						<strong class="w-25 shrink-0 font-semibold">{pluralize(Constants.Tag.GroupText[group], tags.length)}</strong
						>
						<div class="flex flex-wrap items-center gap-1">
							{#each tags as tag}
								<a
									class={badgeVariants({
										class: "bg-muted text-foreground/85 border-none text-sm font-normal",
										variant: "outline"
									})}
									href="/search{new URLSearchParams({ tag: tag.id })}">{tag.name}</a
								>
							{/each}
						</div>
					</li>
				{/each}
			{/if}

			{#if manga.relationships.links}
				{#each Object.entries(ExternalLink.group(manga.relationships.links)) as [name, group]}
					<li class="flex gap-2 md:flex-col md:gap-1">
						<strong class="w-25 shrink-0 font-semibold">
							{#if name === "1"}
								Track
							{:else}
								Read or Buy
							{/if}
						</strong>
						<ul class="flex flex-wrap items-center gap-1">
							{#each Object.entries(group) as [code, link]}
								{#if code in ExternalLink.names}
									{@const url = ExternalLink.build(code, link)}
									{#if Utils.isValidURL(url)}
										{@const name = ExternalLink.names[code]}
										<li>
											<a
												class={badgeVariants({
													class: "bg-muted text-foreground/85 border-none text-sm font-normal",
													variant: "outline"
												})}
												href={url}
												rel="nofollow noreferrer noopener"
												target="_blank"
											>
												<div class="size-3">
													<ImageLoader
														class="rounded-full"
														src={`https://www.google.com/s2/favicons?sz=14&domain_url=${ExternalLink.base(url)}`}
														alt={name}
													/>
												</div>
												{name}
											</a>
										</li>
									{/if}
								{/if}
							{/each}
						</ul>
					</li>
				{/each}
			{/if}

			{#if manga.last_chapter || manga.last_volume}
				<li class="final-chapter flex gap-2 md:flex-col md:gap-1">
					<strong class="w-25 shrink-0 font-semibold">Final Chapter</strong>
					<div class="">
						<Badge>
							{#if manga.last_volume}Volume {manga.last_volume},{/if}
							Chapter {manga.last_chapter}
						</Badge>
					</div>
				</li>
			{/if}
		</ul>
	{/if}
{/snippet}

{#snippet navs()}
	<div class="border-b-2">
		<nav class="text-muted-foreground/80 -mb-0.5 flex flex-nowrap overflow-auto whitespace-nowrap">
			{#each Object.entries(tabs) as [key, item]}
				{#if !item.hidden}
					<a
						class={buttonVariants({
							class: cn("border-b-2 border-b-transparent !bg-transparent", {
								"border-accent": tab === item
							}),
							variant: "ghost"
						})}
						class:!text-accent-foreground={tab === item}
						href="/title/{manga.id}{slug}{new URLSearchParams({
							...Object.entries(page.url.searchParams.entries()),
							...(key === 'chapters' ? {} : { tab: key }),
							...(page.url.searchParams.has('draft') ? { draft: 'true' } : {})
						})}"
					>
						<item.icon />
						{item.name}
					</a>
				{/if}
			{/each}
		</nav>
	</div>
{/snippet}

{#if isMobile.current}
	<section class="space-y-2">
		<header class="space-y-5">
			<div class="flex gap-5">
				<div class="w-[25%] max-w-64 shrink-0">
					{@render cover()}
				</div>
				<div>
					{@render header()}
				</div>
			</div>
			<Collapsible class="max-h-100 space-y-5 overflow-hidden">
				{@render synopsis()}
				{@render tags()}
			</Collapsible>
			{@render navs()}
		</header>
		<div>
			{#if tab}
				<tab.component />
			{/if}
		</div>
	</section>
{:else}
	<section class="flex gap-5">
		<div class="w-[24%] max-w-64 shrink-0">
			<div class="space-y-5">
				{@render cover()}
				{@render tags()}
			</div>
		</div>
		<div class="w-0 flex-auto space-y-5">
			<div class="space-y-2">
				<header class="space-y-5">
					<div>
						<div class="mb-4">
							{@render header()}
						</div>
						{@render synopsis()}
					</div>
					{@render navs()}
				</header>
				<div>
					{#if tab}
						<tab.component />
					{/if}
				</div>
			</div>
		</div>
	</section>
{/if}
<Analytics />
