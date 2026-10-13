<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants } from "$lib";
	import GenericPagination from "$lib/components/GenericPagination.svelte";
	import GenericSearchFilter from "$lib/components/GenericSearchFilter.svelte";
	import UserSearch from "$lib/components/UserSearch.svelte";
	import { StatusCodes } from "http-status-codes";

	let feed = $state<UserFeedResponse>();
	const onSearch = async () => {
		const { href } = window.location;
		const q = new URLSearchParams();

		if (page.url.searchParams.has("name")) q.set("name", page.url.searchParams.get("name")!);
		if (page.url.searchParams.has("page")) {
			const p = page.url.searchParams.get("page");
			if (Number(p) > 1) q.set("page", p!);
		}
		q.set("limit", "100");

		const res = await API.User.Search(q);
		if (window.location.href !== href) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			feed = res.body;
			replaceState(window.location.href, {
				...page.state,
				_$UserSearch_feed: $state.snapshot(feed)
			});
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

<svelte:head>
	{#if feed && feed.page > 1}
		<title>Users - Page {feed.page} - {PUBLIC_SITE_NAME}</title>
	{:else}
		<title>Users - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

<section class="flex flex-auto flex-col space-y-5">
	<header class="space-y-5">
		<h2 class="text-xl font-medium">Users</h2>
		{#key page.url.search}
			<GenericSearchFilter bind:feed />
		{/key}
	</header>
	{#key page.url.search}
		<UserSearch bind:feed {onSearch} />
	{/key}
	{#if feed && feed.total > feed.limit}
		<footer>
			<GenericPagination {feed} />
		</footer>
	{/if}
</section>
