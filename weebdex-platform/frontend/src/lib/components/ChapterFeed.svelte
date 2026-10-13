<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import Grid from "$lib/components/ui/Grid.svelte";
	import { setReadHistory } from "$lib/Context";
	import type { HTTPResponse } from "$lib/HTTP";
	import { Session } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import { writable } from "svelte/store";
	import ChapterFeedMangaEntry from "./ChapterFeedMangaEntry.svelte";
	import ErrorComponent from "./ErrorComponent.svelte";
	import GenericPagination from "./GenericPagination.svelte";
	import { Spinner } from "./ui/spinner";

	const {
		onSearch
	}: {
		onSearch: (n?: number) => Promise<HTTPResponse<MappedChapterFeedResponse>> | undefined;
	} = $props();

	const readHistory = writable<Record<string, Record<string, null>> | undefined>();
	setReadHistory(readHistory);

	let feed = $state<MappedChapterFeedResponse>();
	let entries = $state<Manga[]>([]);
	let idMap = $state<Record<string, undefined>>({});

	const isEntity =
		window.location.pathname.startsWith("/user") ||
		window.location.pathname.startsWith("/author") ||
		window.location.pathname.startsWith("/group");
	let isMounted = $state(false);
	let isLoading = $state(false);
	let footerRef = $state<HTMLElement>();
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const state = page.state as {
			_$ChapterFeed_history: typeof $readHistory;
			_$ChapterFeed_feed: typeof feed;
			_$ChapterFeed_entries: typeof entries;
		};

		if (state._$ChapterFeed_feed && state._$ChapterFeed_entries) {
			$readHistory = state._$ChapterFeed_history;
			feed = state._$ChapterFeed_feed;
			entries = state._$ChapterFeed_entries;
		} else {
			const p = page.url.searchParams.get("page");
			await load(p ? Number(p) : undefined);
		}
		isMounted = true;
	});

	const load = async (n?: number) => {
		if (isLoading) {
			return;
		}

		const { pathname } = window.location;
		isLoading = true;
		try {
			const res = await onSearch(n);
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				feed = res.body;
				replaceState(window.location.href, {
					...page.state,
					_$ChapterFeed_feed: $state.snapshot(feed)
				});

				if (feed.data) {
					const newEntries: Record<string, Manga> = {};
					const newIds = [];

					for (const chapter of feed.data) {
						const id = chapter.relationships.manga.id;
						let manga = newEntries[id] ?? feed.map.manga[id];

						if (!(id in newEntries)) {
							newEntries[id] = manga;
							entries.push(manga);
						}

						if (!(id in idMap)) {
							idMap[id] = undefined;
							if ($Session) newIds.push(id);
						}

						if (!manga.relationships._chapters) manga.relationships._chapters = [];
						if (!manga.relationships._chapters.some(e => e.id === chapter.id)) {
							manga.relationships._chapters.push(chapter);
						}
					}

					entries = entries;
					replaceState(window.location.href, {
						...page.state,
						_$ChapterFeed_entries: $state.snapshot(entries)
					});

					if ($Session && newIds.length) {
						const q = new URLSearchParams();
						newIds.forEach(id => q.append("id", id));
						API.ChapterHistory.Search(q)?.then(res => {
							if (window.location.pathname === pathname && res.status === StatusCodes.OK && res.body) {
								$readHistory = { ...$readHistory, ...res.body };
								replaceState(window.location.href, {
									...page.state,
									_$ChapterFeed_history: $readHistory
								});
							}
						});
					}
				}
			} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
				throw Constants.Error.TooManyRequests;
			} else throw Constants.Error.Unknown;
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

	$effect(() => {
		if (!footerRef) {
			return;
		}

		const observer = new IntersectionObserver(
			async ([entry]) => {
				if (!entry.isIntersecting || !feed || isLoading) {
					return;
				}

				await load(feed.page + 1);
				if (!feed.data?.length) {
					if (footerRef) {
						observer.unobserve(footerRef);
					}
					observer.disconnect();
				}
			},
			{ rootMargin: window.innerHeight + "px" }
		);
		observer.observe(footerRef);
		return () => {
			if (footerRef) observer.unobserve(footerRef);
			observer.disconnect();
		};
	});
</script>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted && feed}
	{#if feed.data?.length}
		<Grid class="gap-x-4" breakpoints={isEntity ? { 0: 2, 1080: 1 } : { 0: 3, 1400: 2, 1080: 1 }}>
			{#each entries as manga (manga)}
				<ChapterFeedMangaEntry {manga} />
			{/each}
		</Grid>
		{#if feed && feed.limit * feed.page < feed.total}
			{#if isEntity}
				<footer>
					<GenericPagination {feed} />
				</footer>
			{:else}
				<footer class:h-0.5={!isLoading} bind:this={footerRef}>
					{#if isLoading}
						<Empty.Root>
							<Spinner class="size-10" />
						</Empty.Root>
					{/if}
				</footer>
			{/if}
		{/if}
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No results found</Empty.Title>
				<Empty.Description>
					{#if window.location.search}
						Try adjusting your search or using different keywords.
					{:else if window.location.pathname.startsWith("/library")}
						<p>The database may still be updating, or you haven't added anything to your library yet.</p>
						<br />
						<p>Any chapters from titles you add to your library will appear here shortly.</p>
					{:else if window.location.pathname.startsWith("/history")}
						<p>The database may still be updating, or you haven't read anything yet.</p>
						<br />
						<p>Any chapters you read will appear here shortly.</p>
					{:else}
						The database may still be updating, or no entries matching your filters were found.
					{/if}
				</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
