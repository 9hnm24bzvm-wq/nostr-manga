<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import API from "$lib/API";
	import MangaChapterList from "$lib/components/ChapterList.svelte";
	import ChapterListFilter from "$lib/components/ChapterListFilter.svelte";
	import GenericPagination from "$lib/components/GenericPagination.svelte";
	import { getManga } from "$lib/Context";
	import { prefs } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";

	const manga = $derived(getManga()());
	let feed = $state<ChapterFeedResponse>();
	const onSearch = async () => {
		const { href } = window.location;
		const q = new URLSearchParams();

		if (page.url.searchParams.has("tlang")) page.url.searchParams.getAll("tlang").forEach(v => q.append("tlang", v));
		else if ($prefs.translated_languages.length) $prefs.translated_languages.forEach(v => q.append("tlang", v));
		if (page.url.searchParams.has("group")) page.url.searchParams.getAll("group").forEach(v => q.append("group", v));
		else if ($prefs.groups.length) $prefs.groups.forEach(v => q.append("groupx", v.id));

		if (page.url.searchParams.has("page")) {
			const p = page.url.searchParams.get("page");
			if (Number(p) > 1) q.set("page", p!);
		}
		if (page.url.searchParams.has("order")) {
			q.set("order", page.url.searchParams.get("order")!);
		}
		q.set("limit", "100");

		const res = await API.Chapter.List(manga.id, q);
		if (window.location.href !== href) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			feed = res.body;
			replaceState(window.location.href, {
				...page.state,
				_$ChapterList_feed: $state.snapshot(feed)
			});
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

<section class="space-y-5">
	{#if manga.relationships.available_languages || manga.relationships.available_groups}
		<header class="space-y-5">
			<ChapterListFilter bind:feed />
			{#if feed && feed.total > feed.limit}
				<GenericPagination {feed} />
			{/if}
		</header>
	{/if}
	{#key page.url.search}
		<MangaChapterList bind:feed {onSearch} />
	{/key}
	{#if feed && feed.total > feed.limit}
		<footer>
			<GenericPagination {feed} />
		</footer>
	{/if}
</section>
