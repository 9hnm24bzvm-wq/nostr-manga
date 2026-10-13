<!-- svelte-ignore state_referenced_locally -->
<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Utils } from "$lib";
	import Panel from "$lib/components/reader/reader-panel.svelte";
	import * as Sidebar from "$lib/components/ui/sidebar";
	import { useSidebar } from "$lib/components/ui/sidebar";
	import { commentsVisibility, isMobile, skipEndOfChapterPage } from "$lib/stores";
	import * as Comments from "../comments";
	import Button from "../ui/button/button.svelte";
	import { useReader } from "./context.svelte";
	import Indicator from "./reader-indicator.svelte";
	import LastPage from "./reader-last-page.svelte";
	import Pages from "./reader-pages.svelte";
	import Pagination from "./reader-pagination.svelte";

	const reader = useReader();
	const sidebar = useSidebar();
	const readerPrefs = reader.prefs;
</script>

<svelte:head>
	{#if reader.chapter}
		{#if reader.currentPage}
			<title
				>Page {reader.currentPage.index + 1}: {Utils.chapterHeadline(reader.chapter, false)} - {reader.chapter
					.relationships.manga.title} - {PUBLIC_SITE_NAME}</title
			>
		{:else}
			<title
				>Read: {Utils.chapterHeadline(reader.chapter, false)} - {reader.chapter.relationships.manga.title} - {PUBLIC_SITE_NAME}</title
			>
		{/if}
		{@html `<script id="chapter-data" type="application/json">${JSON.stringify({
			id: reader.chapter.id,
			manga_id: reader.chapter.relationships.manga.id,
			manga_title: reader.chapter.relationships.manga.title,
			chapter: reader.chapter.chapter,
			volume: reader.chapter.volume,
			title: reader.chapter.title,
			links: reader.chapter.relationships.manga.relationships.links ?? {}
		})}</script>`}
	{/if}
</svelte:head>

<Indicator />

<Sidebar.Outer>
	{#if $readerPrefs.panel_position === "left"}
		<Panel />
	{/if}

	<div class="flex w-0 flex-auto flex-col">
		{#if (!isMobile.current && !sidebar.open) || (isMobile.current && !sidebar.openMobile)}
			<Pagination sticky />
		{/if}

		<main
			class="m-auto flex min-h-[100vh] w-full flex-auto cursor-pointer flex-col select-none"
			style="gap: {$readerPrefs.gap}px"
			bind:this={reader.mainRef}
		>
			{#if reader.pages.length}
				{#if reader.isViewingLastPage && !$skipEndOfChapterPage && $readerPrefs.layout !== Constants.Reader.Layout.Scroll}
					<LastPage />
				{:else}
					<Pages />
				{/if}
			{:else if reader.chapter.is_unavailable}
				<p class="m-auto text-xl">
					{#if reader.chapter.deleted_at}
						This chapter has been removed.
					{:else}
						This chapter is not available yet.
					{/if}
				</p>
			{/if}
		</main>

		{#if (!isMobile.current && !sidebar.open) || (isMobile.current && !sidebar.openMobile)}
			<Pagination />
		{/if}
	</div>

	{#if $readerPrefs.panel_position === "right"}
		<Panel />
	{/if}
</Sidebar.Outer>

{#if $readerPrefs.layout === Constants.Reader.Layout.Scroll}
	<LastPage />
{:else if $commentsVisibility === Constants.Reader.CommentsVisibility.Collapsed && !reader.isViewingLastPage}
	<Button
		class="w-full border-0 border-t border-dashed"
		variant="outline"
		size="lg"
		onclick={() => (reader.isShowComments = !reader.isShowComments)}
	>
		{#if reader.isShowComments}
			Hide Comments
		{:else}
			Show Comments
		{/if}
	</Button>
{/if}

{#if $commentsVisibility === Constants.Reader.CommentsVisibility.Always || ($commentsVisibility === Constants.Reader.CommentsVisibility.Collapsed && (reader.isViewingLastPage || reader.isShowComments || $readerPrefs.layout === Constants.Reader.Layout.Scroll))}
	<div
		class="border-t border-dashed p-2"
		class:pl-3={$readerPrefs.panel_position === "right" && !isMobile.current}
		class:pr-3={$readerPrefs.panel_position === "left" && !isMobile.current}
	>
		<Comments.Comments class="m-auto max-w-300" />
	</div>
{/if}
