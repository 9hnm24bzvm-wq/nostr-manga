<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants } from "$lib";
	import Grid from "$lib/components/ui/Grid.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { prefs } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import ErrorComponent from "./ErrorComponent.svelte";
	import LatestUpdatesGridEntry from "./LatestUpdatesGridEntry.svelte";
	import LatestUpdatesGridSkeleton from "./LatestUpdatesGridSkeleton.svelte";
	import MangaSearchEntryCard from "./MangaSearchEntryCard.svelte";
	import MangaSearchEntrySkeleton from "./MangaSearchEntrySkeleton.svelte";

	let feed = $state<ChapterFeedResponse>();
	let entries = $state<Chapter[]>([]);
	let idMap = $state<Record<string, undefined>>({});

	let isMounted = $state(false);
	let isLoading = $state(false);
	let isCollapsed = $state(false);
	let footerRef = $state<HTMLElement>();
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const state = page.state as {
			_$LatestUpdatesGrid_feed: typeof feed;
			_$LatestUpdatesGrid_entries: typeof entries;
			_$LatestUpdatesGrid_idMap: typeof idMap;
		};

		if (state._$LatestUpdatesGrid_feed && state._$LatestUpdatesGrid_entries && state._$LatestUpdatesGrid_idMap) {
			feed = state._$LatestUpdatesGrid_feed;
			entries = state._$LatestUpdatesGrid_entries;
			idMap = state._$LatestUpdatesGrid_idMap;
		} else {
			await load();
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
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);
			prefs.applyChapterPreferences(q);

			if (n) q.set("page", n.toString());
			q.set("limit", "70");

			const res = await API.Chapter.LatestUpdates(q);
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				feed = res.body;
				replaceState(window.location.href, {
					...page.state,
					_$LatestUpdatesGrid_feed: $state.snapshot(feed)
				});

				if (feed?.data?.length) {
					for (const chapter of feed.data) {
						const id = chapter.relationships.manga.id;
						if (!(id in idMap)) {
							idMap[id] = undefined;
							entries.push(chapter);
						}
					}

					entries = entries;
					replaceState(window.location.href, {
						...page.state,
						_$LatestUpdatesGrid_entries: $state.snapshot(entries),
						_$LatestUpdatesGrid_idMap: $state.snapshot(idMap)
					});
				}
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

	$effect(() => {
		if (!footerRef || isCollapsed) {
			return;
		}

		const observer = new IntersectionObserver(
			async ([entry]) => {
				if (!entry.isIntersecting || !feed || isCollapsed || isLoading) {
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
		return () => observer.disconnect();
	});
</script>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted}
	{#if entries.length}
		<Grid class="gap-3" breakpoints={Constants.MangaGridBreakpoints}>
			{#each entries as chapter (chapter)}
				<article>
					<MangaSearchEntryCard manga={chapter.relationships.manga} isGrid />
					<LatestUpdatesGridEntry {chapter} />
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
				<LatestUpdatesGridSkeleton />
			</article>
		{/each}
	</Grid>
{/if}

{#if !isCollapsed && feed && feed.limit * feed.page < feed.total}
	<div class:h-0.5={!isLoading} bind:this={footerRef}>
		{#if isLoading}
			<Empty.Root>
				<Spinner class="size-10" />
			</Empty.Root>
		{/if}
	</div>
{/if}
