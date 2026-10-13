<script lang="ts">
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Mutex, Permission } from "$lib";
	import type { UpdateSettingsRequest } from "$lib/API/Settings";
	import { useComments } from "$lib/components/comments/context.svelte";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import { Button } from "$lib/components/ui/button";
	import { buttonVariants } from "$lib/components/ui/button/button.svelte";
	import * as Dialog from "$lib/components/ui/dialog";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import { setChaptersCount, setGroup } from "$lib/Context";
	import { Session } from "$lib/stores";
	import { prefs } from "$lib/stores/prefs.js";
	import { cn } from "$lib/Utils";
	import {
		BanIcon,
		CrownIcon,
		FilesIcon,
		FlagIcon,
		InfoIcon,
		MessagesSquare,
		SettingsIcon,
		UserMinusIcon,
		UserPlusIcon
	} from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import { writable } from "svelte/store";
	import GroupClaimDialog from "./GroupClaimDialog.svelte";

	const {
		data,
		children
	}: {
		data: ScanlationGroup;
		children: any;
	} = $props();

	const mutex = new Mutex();
	const group = writable<ScanlationGroup>(data);
	const comments = useComments();

	let isGroupClaim = $state(false);
	let isFollowed = $state(false);
	let isBlocked = $derived($prefs.groups.some(e => e.id === $group.id));
	let chaptersCount = $state<number>();

	setGroup(group);
	setChaptersCount(() => chaptersCount);

	const tabs: Record<
		string,
		{
			name: string;
			icon: any;
			hidden?: boolean;
		}
	> = $derived({
		home: {
			name: "Overview",
			icon: InfoIcon
		},
		uploads: {
			name:
				(chaptersCount ?? $group.relationships?.stats?.chapters)
					? `Uploads (${(chaptersCount ?? $group.relationships?.stats?.chapters)!.toLocaleString()})`
					: `Uploads`,
			icon: FilesIcon
		},
		comments: {
			name: comments.stats?.replies ? `Comments (${comments.stats.replies.toLocaleString()})` : `Comments`,
			icon: MessagesSquare
		},
		settings: {
			name: "Settings",
			icon: SettingsIcon,
			hidden: !$Session || !Permission.Group.canUpdate($group)
		}
	});

	onMount(async () => {
		if ($group) {
			mutex.exec(async () => {
				try {
					await Promise.all([getFollowStatus(), getChaptersCount()]);
				} catch (e) {
					console.error(e);
				}
			});
		}
	});

	const getFollowStatus = async () => {
		if (!(await Session.isValid())) {
			return;
		}

		const res = await API.Group.GetFollowStatus($group.id);
		isFollowed = res?.status === StatusCodes.NO_CONTENT;
	};

	const getChaptersCount = async () => {
		const q = new URLSearchParams();
		prefs.applyMangaPreferences(q);
		prefs.applyChapterPreferences(q);
		q.set("group", $group.id);
		q.set("limit", "0");

		const res = await API.Chapter.Search(q);
		if (res?.status === StatusCodes.OK && res.body) {
			chaptersCount = res.body.total;
		}
	};

	let isUpdatingFollowStatus = $state(false);
	const updateFollowStatus = () =>
		mutex.exec(async () => {
			if (!(await Session.isValid())) {
				return;
			}

			isUpdatingFollowStatus = true;
			try {
				const res = await (isFollowed ? API.Group.Unfollow : API.Group.Follow)($group.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					isFollowed = !isFollowed;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isUpdatingFollowStatus = false;
			}
		});

	let isUpdatingBlockStatus = $state(false);
	const updateBlockStatus = () =>
		mutex.exec(async () => {
			if (!(await Session.isValid())) {
				return;
			}

			isUpdatingBlockStatus = true;
			try {
				const idx = $prefs.groups.findIndex(({ id }) => id === $group.id);
				if (idx >= 0) $prefs.groups.splice(idx, 1);
				else $prefs.groups.push({ id: $group.id, name: $group.name, version: $group.version });

				if ($Session) {
					const payload: UpdateSettingsRequest = {
						version: $prefs.version,
						groups: $prefs.groups.map(({ id }) => id)
					};
					if (Object.keys(payload).length <= 1) {
						return;
					}

					const res = await API.Settings.Update(payload);
					if (res?.status === StatusCodes.OK && res.body) {
						$prefs = $Session.preferences = res.body;
					} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
						throw Constants.Error.TooManyRequests;
					} else throw Constants.Error.Unknown;
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isUpdatingBlockStatus = false;
			}
		});
</script>

<svelte:head>
	<title>{$group.name} - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="space-y-2">
	<header class="space-y-5">
		<div class="flex gap-4 md:items-center">
			<div class="w-20 shrink-0">
				<AspectRatio ratio={1 / 1} class="bg-card overflow-hidden rounded-full">
					{#snippet child({ props })}
						<figure {...props}>
							<ImageLoader class="size-full object-cover" src="/images/avatar.webp" />
						</figure>
					{/snippet}
				</AspectRatio>
			</div>
			<div class="flex flex-auto flex-col gap-4 md:w-0 md:flex-row md:items-center">
				<div class="flex-auto md:w-0">
					<h1 class="truncate text-2xl font-semibold">{$group.name}</h1>
					<strong class="text-muted-foreground truncate font-medium">@{$group.id}</strong>
				</div>
				<div class="flex shrink-0 flex-wrap items-center gap-1">
					{#if !$group.relationships?.members?.some(member => member.is_leader)}
						<Dialog.Root bind:open={isGroupClaim}>
							<Dialog.Trigger class={buttonVariants({ class: "border-none", variant: "outline", size: "sm" })}>
								<CrownIcon />
								Claim
							</Dialog.Trigger>
							<Dialog.Content
								class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col overflow-auto"
							>
								<GroupClaimDialog bind:open={isGroupClaim} group={$group} />
							</Dialog.Content>
						</Dialog.Root>
					{/if}
					{#if $Session}
						{#if !isBlocked}
							<Button
								class="border-none text-green-300 hover:text-green-400"
								variant="outline"
								size="sm"
								onclick={updateFollowStatus}
								disabled={$mutex}
							>
								{#if isUpdatingFollowStatus}
									<Spinner />
								{:else if isFollowed}
									<UserMinusIcon />
								{:else}
									<UserPlusIcon />
								{/if}
								{#if isFollowed}
									Unfollow
								{:else}
									Follow
								{/if}
							</Button>
						{/if}
						{#if !isFollowed}
							<Button
								class="border-none text-red-300 hover:text-red-400"
								variant="outline"
								size="sm"
								disabled={$mutex}
								onclick={updateBlockStatus}
							>
								<BanIcon />
								{#if isBlocked}
									Unblock
								{:else}
									Block
								{/if}
							</Button>
						{/if}
					{/if}
					<Button
						class="border-none"
						variant="outline"
						size="sm"
						onclick={() => {
							ReportDialog.set($group, Constants.EntityType.ScanlationGroup);
							ReportDialog.open();
						}}
					>
						<FlagIcon />
						Report
					</Button>
				</div>
			</div>
		</div>
		<div class="border-b-2">
			<nav class="text-muted-foreground/80 -mb-0.5 flex flex-nowrap overflow-auto whitespace-nowrap">
				{#each Object.entries(tabs) as [key, item]}
					{#if !item.hidden}
						{@const pathname = key === "home" ? `/group/${$group.id}` : `/group/${$group.id}/${key}`}
						<a
							class={buttonVariants({
								class: cn("border-b-2 border-b-transparent !bg-transparent", {
									"border-accent": page.url.pathname === pathname
								}),
								variant: "ghost"
							})}
							class:!text-accent-foreground={page.url.pathname === pathname}
							href={pathname}
						>
							<item.icon />
							{item.name}
						</a>
					{/if}
				{/each}
			</nav>
		</div>
	</header>
	<div class="space-y-5">
		{@render children?.()}
	</div>
</section>
