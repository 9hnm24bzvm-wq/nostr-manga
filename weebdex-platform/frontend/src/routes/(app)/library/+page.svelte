<script lang="ts" module>
	let isShowFilter = $state(true);
</script>

<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants } from "$lib";
	import MangaSearch from "$lib/components/MangaSearch.svelte";
	import MangaSearchFilter from "$lib/components/MangaSearchFilter.svelte";
	import MangaSearchPagination from "$lib/components/MangaSearchPagination.svelte";
	import { prefs } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";

	let feed = $state<MangaFollowFeedResponse>();
	const onSearch = async () => {
		const { href } = window.location;
		const q = new URLSearchParams(window.location.search);

		if (!q.has("limit")) q.set("limit", $prefs.items_per_page);

		const res = await API.MangaFollow.Search(q);
		if (window.location.href !== href) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			feed = res.body;
			replaceState(window.location.href, {
				...page.state,
				_$MangaSearch_feed: $state.snapshot(feed)
			});
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

<svelte:head>
	{#if feed && feed.page > 1}
		<title>My Library - Page {feed.page} - {PUBLIC_SITE_NAME}</title>
	{:else}
		<title>My Library - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

<section class="flex flex-auto flex-col space-y-5" id="search">
	<header class="space-y-5">
		<h2 class="text-xl font-medium">My Library</h2>
		{#key page.url.search}
			<MangaSearchFilter bind:feed bind:isShowFilter />
		{/key}
	</header>
	{#key page.url.search}
		<MangaSearch bind:feed {onSearch} />
	{/key}
	{#if feed && feed.total > feed.limit}
		<footer class="mt-5">
			<MangaSearchPagination {feed} />
		</footer>
	{/if}
</section>
