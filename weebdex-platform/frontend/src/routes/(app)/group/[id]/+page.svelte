<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants, ExternalLink, GroupLink, Typography, Utils } from "$lib";
	import MangaSearchEntryCard from "$lib/components/MangaSearchEntryCard.svelte";
	import MangaSearchPagination from "$lib/components/MangaSearchPagination.svelte";
	import { badgeVariants } from "$lib/components/ui/badge/badge.svelte";
	import { buttonVariants } from "$lib/components/ui/button";
	import Grid from "$lib/components/ui/Grid.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import ViewModeToggles from "$lib/components/ViewModeToggles.svelte";
	import { getGroup } from "$lib/Context";
	import { mangaView, prefs } from "$lib/stores";
	import { ChessRook, Crown, Mail } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const group = getGroup();
	let feed = $state<MangaFeedResponse>();
	let isGrid = $derived($mangaView === "grid");
	let isMounted = $state(false);

	onMount(async () => {
		const state = page.state as {
			_$GroupPage_feed: typeof feed;
		};
		if (state._$GroupPage_feed) {
			feed = state._$GroupPage_feed;
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

			q.set("group", $group.id);
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
					_$GroupPage_feed: $state.snapshot(feed)
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
	{@const keys = Object.keys(GroupLink.names)}
	{#if keys.some(key => key in $group)}
		<section class="space-y-1">
			<header>
				<h2 class="text-base font-semibold">Where to find</h2>
			</header>
			<ul class="flex flex-wrap items-center gap-1">
				{#each keys as key}
					{#if key in $group}
						{@const url = GroupLink.build(key, <string>$group[<keyof typeof $group>key])}
						{#if key === "contact_email" || Utils.isValidURL(url)}
							{@const name = GroupLink.names[key]}
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

{#if !$group.relationships?.members?.some(member => member.is_leader)}
	<section class="rounded-md border border-dashed border-green-500/50 bg-green-500/15 p-3 text-center text-green-300">
		<p>This group is unclaimed and currently has no WeebDex user set as its leader.</p>
		<p>
			If this group belongs to you, click the <strong class="underline">Claim</strong> button and fill out the form.
		</p>
	</section>
{/if}

<section class="space-y-1">
	<header>
		<h2 class="text-base font-semibold">Information</h2>
	</header>
	<div class="typography max-w-200 space-y-2" {@attach Typography.exec}>
		{#if $group.description}
			{@html Utils.parseDescription($group.description)}
		{:else}
			<p>No biography has been provided.</p>
		{/if}
	</div>
</section>

{@render links()}

{#if $group.relationships?.members?.length}
	<section class="space-y-1">
		<header>
			<h2 class="text-base font-semibold">Group Members</h2>
		</header>
		<div class="flex flex-wrap items-center gap-1">
			{#each $group.relationships.members.slice().sort((a, b) => {
				if (a.is_leader) return -1;
				if (b.is_leader) return 1;
				if (a.is_officer) return -1;
				if (b.is_officer) return 1;
				return 0;
			}) as member (member)}
				<a
					class={buttonVariants({
						class: "w-0 min-w-[40%] flex-auto justify-start border-none px-3 py-4",
						variant: "outline"
					})}
					href="/user/{member.id}"
					title={member.name}
				>
					<figure class="bg-card size-5 shrink-0 overflow-hidden rounded-full">
						<ImageLoader class="size-full object-cover" src="/images/avatar.webp" />
					</figure>
					<span class="w-0 flex-auto truncate">
						{member.name}
					</span>
					{#if member.is_leader}
						<span class="shrink-0">
							<Crown />
						</span>
					{:else if member.is_officer}
						<span class="shrink-0">
							<ChessRook />
						</span>
					{/if}
				</a>
			{/each}
		</div>
	</section>
{/if}

<section class="space-y-1">
	<header>
		<h2 class="text-base font-semibold">Upload Permissions</h2>
	</header>
	<div class="flex items-center gap-2 px-2">
		<div
			class="size-2 overflow-hidden rounded-full"
			class:bg-green-500={!$group.locked}
			class:bg-red-500={$group.locked}
		></div>
		<span>
			{#if $group.locked}
				Only group members
			{:else}
				Anyone
			{/if}
		</span>
	</div>
</section>

{#if isMounted && feed?.total}
	<section class="space-y-5">
		<header class="space-y-1">
			<h2 class="text-base font-semibold">Titles ({feed.total})</h2>
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
