<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, AuthorLink, Constants, ExternalLink, Typography, Utils } from "$lib";
	import MangaSearchEntryCard from "$lib/components/MangaSearchEntryCard.svelte";
	import MangaSearchPagination from "$lib/components/MangaSearchPagination.svelte";
	import { badgeVariants } from "$lib/components/ui/badge/badge.svelte";
	import Grid from "$lib/components/ui/Grid.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import ViewModeToggles from "$lib/components/ViewModeToggles.svelte";
	import { getAuthor } from "$lib/Context";
	import { mangaView, prefs } from "$lib/stores";
	import { Mail } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const author = getAuthor();
	let feed = $state<MangaFeedResponse>();
	let isGrid = $derived($mangaView === "grid");
	let isMounted = $state(false);

	onMount(async () => {
		const state = page.state as {
			_$AuthorPage_feed: typeof feed;
		};
		if (state._$AuthorPage_feed) {
			feed = state._$AuthorPage_feed;
		} else {
			await search();
		}
		isMounted = true;
	});

	const search = async () => {
		const { href } = window.location;
		try {
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);

			q.set("authorOrArtist", $author.id);
			q.set("sort", "title");
			q.set("order", "asc");

			if (page.url.searchParams.has("page")) {
				const p = page.url.searchParams.get("page");
				if (Number(p) > 1) q.set("page", p!);
			}
			q.set("limit", "100");

			const res = await API.Manga.Search(q);
			if (window.location.href !== href) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				feed = res.body;
				replaceState(window.location.href, {
					...page.state,
					_$AuthorPage_feed: $state.snapshot(feed)
				});
			} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
				throw Constants.Error.TooManyRequests;
			} else throw Constants.Error.Unknown;
		} catch (e) {
			if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
			else {
				console.error(e);
				toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		}
	};

	let previousPage = $state(page.url.searchParams.get("page"));
	$effect(() => {
		const currentPage = page.url.searchParams.get("page");
		if (isMounted && previousPage !== currentPage) {
			previousPage = currentPage;
			search();
		}
	});
</script>

{#snippet links()}
	{@const keys = Object.keys(AuthorLink.names)}
	{#if keys.some(key => key in $author)}
		<section class="space-y-1">
			<header>
				<h2 class="text-base font-semibold">Where to find</h2>
			</header>
			<ul class="flex flex-wrap items-center gap-1">
				{#each keys as key}
					{#if key in $author}
						{@const url = AuthorLink.build(key, <string>$author[<keyof typeof $author>key])}
						{#if key === "contact_email" || Utils.isValidURL(url)}
							{@const name = AuthorLink.names[key]}
							<li>
								<a
									class={badgeVariants({
										class: "bg-muted border-none text-sm font-normal",
										variant: "outline"
									})}
									href={url}
									rel="nofollow noreferrer noopener"
									target="_blank"
								>
									<div class="size-3">
										{#if key === "contact_email"}
											<Mail class="size-3" />
										{:else}
											<ImageLoader
												class="rounded-full"
												src={`https://www.google.com/s2/favicons?sz=14&domain_url=${ExternalLink.base(url)}`}
												alt={name}
											/>
										{/if}
									</div>
									{name}
								</a>
							</li>
						{/if}
					{/if}
				{/each}
			</ul>
		</section>
	{/if}
{/snippet}

<section class="space-y-1">
	<header>
		<h2 class="text-base font-semibold">Information</h2>
	</header>
	<div class="typography max-w-200 space-y-2" {@attach Typography.exec}>
		{#if $author.description}
			{@html Utils.parseDescription($author.description)}
		{:else}
			<p>No biography has been provided.</p>
		{/if}
	</div>
</section>

{@render links()}

{#if isMounted && feed?.total}
	<section class="space-y-5">
		<header class="space-y-1">
			<h2 class="text-base font-semibold">Works ({feed.total})</h2>
			<div class="flex items-center gap-2">
				<ViewModeToggles />
			</div>
		</header>
		<Grid class="gap-3" breakpoints={isGrid ? Constants.MangaGridBreakpoints : { 0: 2, 768: 1 }}>
			{#each feed.data as manga (manga)}
				<article>
					<MangaSearchEntryCard {manga} {isGrid} />
				</article>
			{/each}
		</Grid>
		{#if feed && feed.total > feed.limit}
			<footer>
				<MangaSearchPagination {feed} />
			</footer>
		{/if}
	</section>
{/if}
