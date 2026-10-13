<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants } from "$lib";
	import API from "$lib/API";
	import GenericPagination from "$lib/components/GenericPagination.svelte";
	import ReportSearch from "$lib/components/ReportSearch.svelte";
	import ReportSearchFilter from "$lib/components/ReportSearchFilter.svelte";
	import { StatusCodes } from "http-status-codes";

	let feed = $state<ReportFeedResponse>();
	const onSearch = async () => {
		const { href } = window.location;
		const q = new URLSearchParams(window.location.search);
		q.set("limit", "36");

		const res = await API.Report.Search(q);
		if (window.location.href !== href) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			feed = res.body;
			replaceState(window.location.href, {
				...page.state,
				_$ReportSearch_feed: $state.snapshot(feed)
			});
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

<svelte:head>
	{#if feed && feed.page > 1}
		<title>My Reports - Page {feed.page} - {PUBLIC_SITE_NAME}</title>
	{:else}
		<title>My Reports - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

<section class="flex flex-auto flex-col space-y-5">
	<header class="space-y-5">
		<h2 class="text-xl font-medium">My Reports</h2>
		{#key page.url.search}
			<ReportSearchFilter bind:feed />
		{/key}
	</header>
	{#key page.url.search}
		<ReportSearch bind:feed {onSearch} />
	{/key}
	{#if feed && feed.total > feed.limit}
		<footer>
			<GenericPagination {feed} />
		</footer>
	{/if}
</section>
