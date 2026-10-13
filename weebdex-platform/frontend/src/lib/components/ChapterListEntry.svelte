<script lang="ts">
	import { API, Constants, Locale, Permission, Utils } from "$lib";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import { Spinner } from "$lib/components/ui/spinner";
	import { getManga, getReadHistory } from "$lib/Context";
	import { isMobile, Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Clock, Eye, EyeOff, Flag, MessagesSquareIcon, Pencil, User, Users } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import type { HTMLAttributes } from "svelte/elements";

	let {
		class: className,
		chapter,
		depth = 0,
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
	class={cn("relative flex items-center border-l-2 border-l-transparent p-1.5", className)}
	class:!border-l-green-800={$Session && !isRead}
	{...restProps}
>
	<a class="absolute inset-0" href="/chapter/{chapter.id}" {title}></a>
	<div class="w-0 flex-auto">
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
					{#if depth > 0 && chapter.title}
						{chapter.title ?? title}
					{:else}
						{title}
					{/if}
				</h3>
				{#if ((manga.last_volume && chapter.volume === manga.last_volume) || !manga.last_volume) && manga.last_chapter && chapter.chapter === manga.last_chapter}
					<strong class="rounded-md bg-sky-600 px-1 text-xs font-medium text-white">END</strong>
				{/if}
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
			{#if chapter.published_at || chapter.relationships.stats}
				<div class="flex w-52.5 shrink-0 items-center justify-end gap-2">
					{#if chapter.published_at}
						<div
							class="ml-[0.0625rem] flex items-center gap-1.5"
							class:flex-auto={chapter.relationships.stats}
							class:w-0={chapter.relationships.stats}
						>
							<Clock class="shrink-0" size="14" />
							<time class="truncate">{chapter.published_at?.ago()}</time>
						</div>
					{/if}
					{#if chapter.relationships.stats}
						<div class="flex w-18 shrink-0 items-center gap-1.5">
							<Eye class="shrink-0" size="16" />
							{#if chapter.relationships.stats.views}
								<span class="truncate">{chapter.relationships.stats.views.short()}</span>
							{:else}
								<span class="text-muted-foreground truncate">N/A</span>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>
		<div class="flex items-center gap-2">
			<div class="w-0 flex-auto">
				<div class="flex items-center gap-2">
					<Users class="ml-0.5 shrink-0" size="16" />
					{#if chapter.relationships.groups?.length}
						{#each chapter.relationships.groups as group}
							<a class="hover:text-accent-foreground relative truncate" href="/group/{group.id}" title={group.name}>
								{group.name}
							</a>
						{/each}
					{:else}
						<em>No Group</em>
					{/if}
				</div>
			</div>
			{#if (chapter.relationships.uploader && chapter.relationships.uploader.name !== "system") || chapter.relationships.stats}
				<div class="flex w-52.5 shrink-0 items-center justify-end gap-2">
					{#if chapter.relationships.uploader && chapter.relationships.uploader.name !== "system"}
						<div class="flex w-0 flex-auto items-center gap-1">
							<User class="shrink-0" size="16" />
							<a
								class="hover:text-accent-foreground relative w-0 flex-auto truncate"
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
					<a
						class="hover:text-accent-foreground relative flex w-18 shrink-0 items-center gap-1.5"
						href="/forums/threads/{chapter.relationships.thread.id}"
						title="Go to comments"
					>
						<MessagesSquareIcon class="shrink-0" size="16" />
						{#if chapter.relationships.thread.relationships.stats.replies}
							<span class="truncate">{chapter.relationships.thread.relationships.stats.replies.short()}</span>
						{/if}
					</a>
				</div>
			{/if}
		</div>
	</div>
	{#if !isMobile.current && chapter.version >= 0}
		<Button
			class="relative shrink-0 border-none"
			type="button"
			variant="outline"
			size="icon"
			onclick={() => {
				ReportDialog.set(chapter, Constants.EntityType.Chapter);
				ReportDialog.open();
			}}
		>
			<Flag />
		</Button>
	{/if}
</article>
