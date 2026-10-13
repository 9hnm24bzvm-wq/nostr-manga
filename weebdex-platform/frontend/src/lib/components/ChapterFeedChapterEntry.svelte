<script lang="ts">
	import { API, Constants, Locale, Permission, Utils } from "$lib";
	import { getManga, getReadHistory } from "$lib/Context";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Eye, EyeOff, MessagesSquareIcon, Pencil } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import type { HTMLAttributes } from "svelte/elements";
	import { Button, buttonVariants } from "./ui/button";
	import { Spinner } from "./ui/spinner";

	const {
		class: className,
		chapter,
		...restProps
	}: HTMLAttributes<HTMLElement> & {
		chapter: Chapter;
		depth?: number;
	} = $props();

	const manga = $derived(getManga()());
	const readHistory = getReadHistory();
	const isRead = $derived(
		$readHistory && manga.id in $readHistory ? chapter.id in ($readHistory[manga.id] ?? {}) : false
	);

	// svelte-ignore state_referenced_locally
	const title = Utils.chapterHeadlineWithoutVolume(chapter);
	// svelte-ignore state_referenced_locally
	const role = chapter.relationships.uploader?.roles
		? chapter.relationships.uploader.roles
				.slice()
				.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b))[0]
		: undefined;

	let isUpdatingReadHistory = $state(false);
	const updateReadHistory = async () => {
		if (isUpdatingReadHistory) return;
		isUpdatingReadHistory = true;

		try {
			const res = await API.ChapterHistory.UpdateBatch(
				manga.id,
				isRead ? { unread_ids: [chapter.id] } : { read_ids: [chapter.id] }
			);
			if (res?.status === StatusCodes.OK && res?.body) {
				if (isRead) {
					if ($readHistory && manga.id in $readHistory) {
						delete $readHistory[manga.id][chapter.id];
					}
				} else {
					if (!$readHistory) $readHistory = {};
					($readHistory[manga.id] ??= {})[chapter.id] = null;
				}
				$readHistory = $readHistory;
			}
		} finally {
			isUpdatingReadHistory = false;
		}
	};
</script>

<article
	class={cn("relative border-l-2 border-l-transparent px-2.5 py-1.5", className)}
	class:!border-l-accent={$Session && !isRead}
	{...restProps}
>
	<a class="absolute inset-0" href="/chapter/{chapter.id}" {title}></a>
	<div class="flex items-center gap-2">
		<div class="flex w-0 flex-auto items-center gap-2">
			{#if $Session}
				<Button
					class="relative -mt-0.5 -mr-1 -mb-0.5 -ml-1 size-6 !bg-transparent"
					variant="ghost"
					size="icon-sm"
					onclick={updateReadHistory}
					disabled={isUpdatingReadHistory}
					title={isRead ? "Mark as unread" : "Mark as unread"}
				>
					{#if isUpdatingReadHistory}
						<Spinner />
					{:else if isRead}
						<EyeOff />
					{:else}
						<Eye />
					{/if}
				</Button>
			{/if}
			<img
				class="size-4 shrink-0"
				src="/images/flags/{Locale.to3166(chapter.language)}.svg"
				alt={Locale.toLanguage(chapter.language)}
			/>
			<h3 class="truncate font-semibold">
				{title}
			</h3>
			{#if $Session && Permission.Chapter.canUpdate(chapter)}
				<a
					class={buttonVariants({
						class: "relative -mt-0.5 -mr-1 -mb-0.5 -ml-1 size-6 !bg-transparent",
						variant: "ghost",
						size: "icon-sm"
					})}
					href="/chapter/{chapter.id}/edit"
					title="Edit"
				>
					<Pencil />
				</a>
			{/if}
		</div>
	</div>
	<div class="text-foreground/75 flex items-center gap-5">
		<div class="flex w-0 flex-auto items-center gap-1">
			{#if chapter.read_at}
				<div class="shrink-0">
					Read
					<time datetime={chapter.read_at}>
						{chapter.read_at.ago()}
					</time>
				</div>
			{:else}
				<time class="shrink-0" datetime={chapter.published_at ?? chapter.created_at}>
					{(chapter.published_at ?? chapter.created_at)?.ago()}
				</time>
			{/if}
			&bull;
			<div class="truncate">
				{#if chapter.relationships.groups?.length}
					{#each chapter.relationships.groups as group, i}
						{#if i}
							{#if i === chapter.relationships.groups.length - 1}
								{" & "}
							{:else}
								{", "}
							{/if}
						{/if}
						<a class="hover:text-accent-foreground relative" href="/group/{group.id}" title={group.name}>
							{group.name}
						</a>
					{/each}
				{:else}
					<em>No Group</em>
				{/if}
			</div>
			{#if chapter.relationships.uploader && chapter.relationships.uploader.name !== "system"}
				&bull;
				<div class="shrink-0">
					<a
						class="hover:text-accent-foreground relative"
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
						href="/user/{chapter.relationships.uploader.id}"
						title={chapter.relationships.uploader.name}
					>
						{chapter.relationships.uploader.name}
					</a>
				</div>
			{/if}
		</div>
		<a
			class="hover:text-accent-foreground relative flex shrink-0 items-center gap-1.5"
			href="/forums/threads/{chapter.relationships.thread.id}"
			title="Go to comments"
		>
			<MessagesSquareIcon class="shrink-0" size="16" />
			{#if chapter.relationships.thread.relationships.stats.replies}
				{chapter.relationships.thread.relationships.stats.replies.short()}
			{/if}
		</a>
	</div>
</article>
