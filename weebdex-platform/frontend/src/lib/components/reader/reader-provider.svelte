<script lang="ts" module>
	let lastMangaId = $state<string>();
	let aggregated = $state<ChapterAggregateResponse>();

	const makeSpreads = (pages: ReaderPage[], offset: number = 0): Array<ReaderPage[]> => {
		const result: Array<ReaderPage[]> = [];
		for (let i = 0; i < pages.length; i++) {
			const current = pages[i];

			// If it's a wide (landscape) page, show it as a single spread
			if (i < offset || current.data.dimensions[0] > current.data.dimensions[1]) {
				result.push([current]);
				continue;
			}

			// If next page exists and both are not wide, make a double spread
			const next = pages[i + 1];
			if (next?.data.dimensions[0] <= next?.data.dimensions[1]) {
				result.push([current, next]);
				i++; // Skip the next page since it's already paired
			} else {
				// Otherwise, single page
				result.push([current]);
			}
		}
		return result;
	};
</script>

<!-- svelte-ignore state_referenced_locally -->
<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import API from "$lib/API";
	import { dataSaver, mangaPrefs, webtoonPrefs } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount, tick, type Snippet } from "svelte";
	import { writable } from "svelte/store";
	import { setReader } from "./context.svelte";
	import Analytics from "./reader-analytics.svelte";
	import History from "./reader-history.svelte";
	import Keyboard from "./reader-keyboard-listener.svelte";
	import Queue from "./reader-queue.svelte";
	import Scroll from "./reader-scroll-listener.svelte";

	const { chapter, children }: { chapter: Chapter; children: Snippet } = $props();
	const reader = setReader({
		prefs:
			chapter.relationships.manga.language != "ja" ||
			chapter.relationships.manga.relationships.tags?.some(
				e => e.group === Constants.Tag.Group.Format && e.name === "Long Strip"
			)
				? webtoonPrefs
				: mangaPrefs,
		chapter: chapter,
		aggregated: () => aggregated
	});
	const readerPrefs = $derived(reader.prefs);

	onMount(async () => {
		if (reader.isViewingLastPage) {
			await tick().then(async () => {
				const lastPage = Math.max(reader.pages.length - 1, 0);
				reader.currentPage = reader.pages[lastPage];

				replaceState(`/chapter/${chapter.id}/${lastPage}`, { ...page.state, page: lastPage });
			});
		} else if (!page.params.page) {
			await tick().then(() => replaceState(`/chapter/${chapter.id}/1`, { ...page.state, page: 1 }));
		}

		if (lastMangaId !== chapter.relationships.manga.id || !aggregated) {
			try {
				const q = new URLSearchParams();
				q.set("tlang", chapter.language);

				const res = await API.Chapter.Aggregate(chapter.relationships.manga.id, q);
				if (res?.status === StatusCodes.OK && res.body) {
					lastMangaId = chapter.relationships.manga.id;
					aggregated = res.body;
				}
			} catch (e) {
				console.error(e);
			}
		}
	});

	let previousOffset: number;
	let previousDataSaver: boolean;
	$effect(() => {
		let isPreviousDataSaverChanged = previousDataSaver !== $dataSaver;
		if (isPreviousDataSaverChanged) previousDataSaver = $dataSaver;
		let isPreviousOffsetChanged = previousOffset !== $readerPrefs.offset;
		if (isPreviousOffsetChanged) previousOffset = $readerPrefs.offset;

		if (isPreviousDataSaverChanged) {
			reader.pages = (($dataSaver && chapter.data_optimized ? chapter.data_optimized : chapter.data) ?? [])?.map(
				(e, i) => ({
					data: e,
					index: i,
					ref: writable<HTMLDivElement | undefined>()
				})
			);
			reader.currentPage =
				reader.pages[
					Math.min(Math.max(Number((page.state as any).page ?? page.params.page ?? 1), 1), reader.pages.length) - 1
				];
		}
		if (isPreviousOffsetChanged || isPreviousDataSaverChanged) {
			reader.spreads = makeSpreads(reader.pages, $readerPrefs.offset);
		}
	});
</script>

{#key reader.key}
	<Queue />
{/key}

<Scroll />
<Keyboard />

{#if reader.pages.length}
	<Analytics />
	<History />
{/if}

{@render children()}
