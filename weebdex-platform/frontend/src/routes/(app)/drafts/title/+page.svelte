<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants } from "$lib";
	import DraftSearch from "$lib/components/draft/manga/DraftSearch.svelte";
	import DraftSearchFilter from "$lib/components/draft/manga/DraftSearchFilter.svelte";
	import GenericPagination from "$lib/components/GenericPagination.svelte";
	import { buttonVariants } from "$lib/components/ui/button";
	import { PlusIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";

	let feed = $state<MangaFeedResponse>();
	const onSearch = async () => {
		const { href } = window.location;
		const q = new URLSearchParams(window.location.search);
		q.set("limit", "36");

		const res = await API.Manga.SearchDrafts(q);
		if (window.location.href !== href) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			feed = res.body;
			replaceState(window.location.href, {
				...page.state,
				_$DraftSearch_feed: $state.snapshot(feed)
			});
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

<svelte:head>
	<title>Title: Drafts - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="flex flex-auto flex-col space-y-5">
	<header class="space-y-5">
		<h2 class="text-xl font-medium">My Title Drafts</h2>
	</header>
	<div class="space-y-5">
		<div class="space-y-2">
			<div
				class="typography space-y-2 rounded-md border border-dashed border-yellow-500/50 bg-yellow-500/15 p-3 text-yellow-300"
			>
				<p>
					The title draft system lets you preview and edit a title before it goes live, while also helping us maintain
					data quality and prevent abuse or spam.
				</p>
				<p>
					Once you're satisfied, you can submit the draft for publication. Most submissions are queued for manual review
					by the moderation team (usually within 48 hours). However, Contributors and Power Uploaders have their drafts
					approved automatically. This privilege may be revoked in cases of abuse.
				</p>
				<p>
					If your submission is rejected, check whether it violates site rules or duplicates an existing title. Rejected
					submissions can be edited and resubmitted. If the reason is still unclear, contact us via Discord or email.
				</p>
			</div>
			<a
				class={buttonVariants({
					class:
						"w-full border border-dashed border-green-500/50 bg-green-500/15 !p-3 text-green-300 hover:!bg-green-500/20",
					variant: "ghost"
				})}
				href="/create/title"
			>
				<PlusIcon />
				Create New Draft
			</a>
			{#key page.url.search}
				<DraftSearchFilter bind:feed />
			{/key}
		</div>
		{#key page.url.search}
			<DraftSearch bind:feed {onSearch} />
		{/key}
	</div>
	{#if feed && feed.total > feed.limit}
		<footer>
			<GenericPagination {feed} />
		</footer>
	{/if}
</section>
