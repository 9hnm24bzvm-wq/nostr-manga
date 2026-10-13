<script lang="ts">
	import { Constants, Utils } from "$lib";
	import * as Comments from "$lib/components/comments";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { isMobile } from "$lib/stores";
	import {
		ArrowLeftIcon,
		ArrowRightIcon,
		BookIcon,
		FlagIcon,
		HomeIcon,
		MessagesSquareIcon,
		UserIcon,
		UsersIcon
	} from "@lucide/svelte";
	import pluralize from "pluralize";
	import { onMount } from "svelte";
	import { useComments } from "../comments/context.svelte";
	import { Button, buttonVariants } from "../ui/button";
	import { useReader } from "./context.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;
	const comments = useComments();

	const role = reader.chapter.relationships.uploader?.roles
		? reader.chapter.relationships.uploader.roles
				.slice()
				.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b))[0]
		: undefined;

	onMount(() => {
		reader.lastPageRef?.scrollIntoView();
	});
</script>

{#snippet standard()}
	<div class="flex flex-col items-center justify-center gap-1">
		<h4 class="text-lg font-medium">Rate this chapter</h4>
		<div class="flex items-center gap-2">
			<Button
				class="!text-destructive border-none"
				variant="outline"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();

					ReportDialog.set(reader.chapter, Constants.EntityType.Chapter);
					ReportDialog.open();
				}}
			>
				<FlagIcon class="shrink-0" />
				Report
			</Button>
			<Comments.Votes />
		</div>
	</div>
	<div class="flex flex-col items-center justify-center gap-2">
		<div class="flex flex-wrap items-center justify-center gap-2">
			<a
				class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}
				href="/"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<HomeIcon class="shrink-0" />
				Home
			</a>
			<a
				class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}
				href="/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(reader.chapter.relationships.manga.title)}"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<BookIcon class="shrink-0" />
				Title
			</a>
			<a
				class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}
				href="/forums/threads/{reader.chapter.relationships.thread.id}"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<MessagesSquareIcon class="shrink-0" />
				{#if comments.stats?.replies}
					{comments.stats.replies.toLocaleString()}
					{pluralize("Comment", comments.stats.replies)}
				{:else}
					Go to comments
				{/if}
			</a>
		</div>
		<div class="flex flex-wrap items-center justify-center gap-2">
			{#if reader.prevChapter}
				<a
					class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}
					href="/chapter/{reader.prevChapter.id}/1"
					onclick={e => {
						e.stopPropagation();
						e.stopImmediatePropagation();
					}}
				>
					<ArrowLeftIcon class="shrink-0" />
					{Utils.chapterHeadlineWithoutVolume(reader.prevChapter)}
				</a>
			{:else}
				<span
					class={buttonVariants({
						class: "pointer-events-none border-none opacity-50",
						variant: "outline",
						size: "sm"
					})}
				>
					<ArrowLeftIcon class="shrink-0" />
					N/A
				</span>
			{/if}

			{#if reader.nextChapter}
				<a
					class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}
					href="/chapter/{reader.nextChapter.id}/1"
					onclick={e => {
						e.stopPropagation();
						e.stopImmediatePropagation();
					}}
				>
					{Utils.chapterHeadlineWithoutVolume(reader.nextChapter)}
					<ArrowRightIcon class="shrink-0" />
				</a>
			{:else}
				<span
					class={buttonVariants({
						class: "pointer-events-none border-none opacity-50",
						variant: "outline",
						size: "sm"
					})}
				>
					N/A
					<ArrowRightIcon class="shrink-0" />
				</span>
			{/if}
		</div>
	</div>
{/snippet}

{#snippet longstrip()}
	<div class="flex flex-col items-center justify-center gap-1">
		<h4 class="text-lg font-medium">Rate this chapter</h4>
		<div class="flex items-center gap-2">
			<Button
				class="!text-destructive border-none"
				variant="outline"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();

					ReportDialog.set(reader.chapter, Constants.EntityType.Chapter);
					ReportDialog.open();
				}}
			>
				<FlagIcon class="shrink-0" />
				Report
			</Button>
			<Comments.Votes />
		</div>
	</div>
	<div class="flex w-full items-center justify-between gap-2">
		{#if reader.prevChapter}
			<a
				class={buttonVariants({ class: "min-w-23 border-none", variant: "outline", size: "sm" })}
				href="/chapter/{reader.prevChapter.id}/1"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<ArrowLeftIcon class="shrink-0" />
				<span class="flex-auto text-center">{Utils.chapterHeadlineWithoutVolume(reader.prevChapter)}</span>
			</a>
		{:else}
			<span
				class={buttonVariants({
					class: "pointer-events-none min-w-23 border-none opacity-50",
					variant: "outline",
					size: "sm"
				})}
			>
				<ArrowLeftIcon class="shrink-0" />
				<span class="flex-auto text-center">N/A</span>
			</span>
		{/if}

		<div>
			<a
				class={buttonVariants({
					class: "border-none",
					variant: "outline",
					size: isMobile.current ? "icon" : "sm"
				})}
				href="/"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<HomeIcon class="shrink-0" />
				{#if !isMobile.current}
					Home
				{/if}
			</a>
			<a
				class={buttonVariants({
					class: "border-none",
					variant: "outline",
					size: isMobile.current ? "icon" : "sm"
				})}
				href="/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(reader.chapter.relationships.manga.title)}"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<BookIcon class="shrink-0" />
				{#if !isMobile.current}
					Title
				{/if}
			</a>

			{#if !isMobile.current}
				<a
					class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}
					href="/forums/threads/{comments.thread.id}"
					onclick={e => {
						e.stopPropagation();
						e.stopImmediatePropagation();
					}}
				>
					<MessagesSquareIcon class="shrink-0" />
					{#if comments.stats?.replies}
						{comments.stats.replies}
						{pluralize("Comment", comments.stats.replies)}
					{:else}
						Go to comments
					{/if}
				</a>
			{/if}
		</div>

		{#if reader.nextChapter}
			<a
				class={buttonVariants({ class: "min-w-23 border-none", variant: "outline", size: "sm" })}
				href="/chapter/{reader.nextChapter.id}/1"
				onclick={e => {
					e.stopPropagation();
					e.stopImmediatePropagation();
				}}
			>
				<span class="flex-auto text-center">{Utils.chapterHeadlineWithoutVolume(reader.nextChapter)}</span>
				<ArrowRightIcon class="shrink-0" />
			</a>
		{:else}
			<span
				class={buttonVariants({
					class: "pointer-events-none min-w-23 border-none opacity-50",
					variant: "outline",
					size: "sm"
				})}
			>
				<span class="flex-auto text-center">N/A</span>
				<ArrowRightIcon class="shrink-0" />
			</span>
		{/if}
	</div>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="m-auto flex w-full flex-auto items-center border-dashed p-2"
	class:border-t={$readerPrefs.layout === Constants.Reader.Layout.Scroll}
	class:bg-card={$readerPrefs.layout === Constants.Reader.Layout.Scroll}
	onclick={reader.listener}
	bind:this={reader.lastPageRef}
>
	<div class="m-auto flex max-w-300 flex-auto flex-col items-center space-y-5">
		<div class="space-y-2 text-center">
			<h3 class="text-xl font-semibold">You have reached the end of the chapter</h3>
			<div class="bg-input/5 space-y-1 rounded-md border border-dashed px-10 py-5">
				<h4 class="font-semibold">
					<span class="inline align-middle">{Utils.chapterHeadline(reader.chapter)}</span>
					{#if ((reader.chapter.relationships.manga.last_volume && reader.chapter.volume === reader.chapter.relationships.manga.last_volume) || !reader.chapter.relationships.manga.last_volume) && reader.chapter.relationships.manga.last_chapter && reader.chapter.chapter === reader.chapter.relationships.manga.last_chapter}
						<strong class="ml-1 inline rounded-md bg-sky-600 px-1 align-middle text-xs font-medium text-white">
							END
						</strong>
					{/if}
				</h4>
				<div class="flex flex-wrap items-center justify-center gap-2">
					{#if reader.chapter.relationships.groups?.length}
						{#each reader.chapter.relationships.groups as group}
							<div class="flex items-center gap-1">
								<UsersIcon class="size-4 shrink-0" />
								<a
									class="hover:text-accent-foreground"
									href="/group/{group.id}"
									onclick={e => {
										e.stopPropagation();
										e.stopImmediatePropagation();
									}}
								>
									{group.name}
								</a>
							</div>
						{/each}
					{:else}
						<div class="flex items-center gap-1">
							<UsersIcon class="size-4 shrink-0" />
							<em>No Group</em>
						</div>
					{/if}
					{#if reader.chapter.relationships.uploader && reader.chapter.relationships.uploader.name !== "system"}
						<div class="flex items-center gap-1">
							<UserIcon class="size-4 shrink-0" />
							<a
								class:text-orange-600={role === Constants.User.Role.Sysop ||
									role === Constants.User.Role.Staff ||
									role === Constants.User.Role.Moderator}
								class:hover:!text-orange-500={role === Constants.User.Role.Sysop ||
									role === Constants.User.Role.Staff ||
									role === Constants.User.Role.Moderator}
								class:text-green-600={role === Constants.User.Role.PowerUploader}
								class:hover:!text-green-500={role === Constants.User.Role.PowerUploader}
								class:text-emerald-600={role === Constants.User.Role.Contributor}
								class:hover:!text-emerald-500={role === Constants.User.Role.Contributor}
								class:text-sky-600={role === Constants.User.Role.GroupLeader}
								class:hover:!text-sky-500={role === Constants.User.Role.GroupLeader}
								class:text-cyan-600={role === Constants.User.Role.GroupOfficer}
								class:hover:!text-cyan-500={role === Constants.User.Role.GroupOfficer}
								href="/user/{reader.chapter.relationships.uploader.id}"
								title={reader.chapter.relationships.uploader.name}
								onclick={e => {
									e.stopPropagation();
									e.stopImmediatePropagation();
								}}
							>
								{reader.chapter.relationships.uploader.name}
							</a>
						</div>
					{/if}
				</div>
			</div>
		</div>

		{#if $readerPrefs.layout === Constants.Reader.Layout.Scroll}
			{@render longstrip()}
		{:else}
			{@render standard()}
		{/if}
	</div>
</div>
