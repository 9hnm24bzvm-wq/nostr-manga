<script lang="ts">
	import { Constants, Utils } from "$lib";
	import * as Comments from "$lib/components/comments";
	import ReaderChapterSelector from "$lib/components/reader/reader-chapter-selector.svelte";
	import ReaderPageSelector from "$lib/components/reader/reader-page-selector.svelte";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as Sidebar from "$lib/components/ui/sidebar";
	import { useSidebar } from "$lib/components/ui/sidebar";
	import * as Tabs from "$lib/components/ui/tabs";
	import { readerTab } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { BookIcon, FlagIcon, HomeIcon, MessagesSquareIcon, UserIcon, UsersIcon, XIcon } from "@lucide/svelte";
	import pluralize from "pluralize";
	import { onMount } from "svelte";
	import { useComments } from "../comments/context.svelte";
	import { ScrollArea } from "../ui/scroll-area";
	import { Separator } from "../ui/separator";
	import { useReader } from "./context.svelte";
	import GroupSelector from "./reader-group-selector.svelte";
	import Settings from "./reader-settings.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;
	const sidebar = useSidebar();
	const comments = useComments();

	const role = reader.chapter.relationships.uploader?.roles
		? reader.chapter.relationships.uploader.roles
				.slice()
				.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b))[0]
		: undefined;

	let isMounted = $state(false);
	onMount(() => {
		sidebar.setOpen($readerPrefs.panel);
		isMounted = true;
	});

	$effect(() => {
		if (isMounted) {
			$readerPrefs.panel = sidebar.open;
		}
	});
</script>

{#if isMounted}
	<Sidebar.Root
		class={cn("whitespace-nowrap", {
			sticky: sidebar.open
		})}
		variant="sidebar"
		side={$readerPrefs.panel_position}
	>
		<Tabs.Root class="h-full !gap-0 !space-y-0" bind:value={$readerTab}>
			<Sidebar.Header>
				<div class="flex items-center gap-2">
					<div class="w-0 flex-auto">
						<h1 class="block truncate text-base/5 font-semibold">
							<a
								class="hover:text-accent-foreground"
								href="/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(
									reader.chapter.relationships.manga.title
								)}"
							>
								{reader.chapter.relationships.manga.title}
							</a>
						</h1>
						{#if reader.chapter.title && reader.chapter.title.localeCompare( "oneshot", undefined, { sensitivity: "accent" } ) !== 0}
							<h2 class="text-muted-foreground block truncate font-medium">{reader.chapter.title}</h2>
						{/if}
					</div>
					<div class="flex shrink-0 items-center gap-1">
						<a class={buttonVariants({ class: "border-none", variant: "outline", size: "icon" })} href="/">
							<HomeIcon class="size-5" />
						</a>
						<a
							class={buttonVariants({ class: "border-none", variant: "outline", size: "icon" })}
							href="/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(
								reader.chapter.relationships.manga.title
							)}"
						>
							<BookIcon class="size-5" />
						</a>
						<Button class="border-none" type="button" variant="outline" size="icon" onclick={sidebar.toggle}>
							<XIcon class="size-5" />
						</Button>
					</div>
				</div>
			</Sidebar.Header>
			<Sidebar.Separator />
			<Sidebar.Content class="flex flex-col gap-0 overflow-hidden">
				<div class="space-y-2 p-2">
					<div class="space-y-1">
						<div class="flex items-center gap-1">
							<ReaderChapterSelector />
						</div>
						<div class="flex items-center gap-1">
							<ReaderPageSelector />
						</div>
						<div class="flex items-center gap-1">
							<GroupSelector class="w-0 flex-auto" />
							<Button
								class="!text-destructive w-0 flex-auto border-none"
								variant="outline"
								onclick={() => {
									ReportDialog.set(reader.chapter, Constants.EntityType.Chapter);
									ReportDialog.open();
								}}
							>
								<FlagIcon />
								Report
							</Button>
						</div>
						<div class="flex items-center gap-1">
							<Comments.Votes />
							<a
								class={buttonVariants({ class: "flex-auto border-none", variant: "outline" })}
								href="/forums/threads/{comments.thread.id}"
							>
								<MessagesSquareIcon />
								{#if comments.stats?.replies}
									{comments.stats.replies.toLocaleString()}
									{pluralize("Comment", comments.stats.replies)}
								{:else}
									No comments yet
								{/if}
							</a>
						</div>
					</div>
					<div class="flex flex-wrap items-center justify-center gap-2">
						{#if reader.chapter.relationships.groups?.length}
							{#each reader.chapter.relationships.groups as group}
								<div class="flex items-center gap-1">
									<UsersIcon class="size-4 shrink-0" />
									<a class="hover:text-accent-foreground" href="/group/{group.id}">{group.name}</a>
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
								>
									{reader.chapter.relationships.uploader.name}
								</a>
							</div>
						{/if}
					</div>
				</div>
				<Separator />
				<ScrollArea class="h-0 flex-auto" scrollbarXClasses="hidden" scrollbarYClasses="hidden">
					<Settings />
				</ScrollArea>
			</Sidebar.Content>
		</Tabs.Root>
	</Sidebar.Root>
{/if}
