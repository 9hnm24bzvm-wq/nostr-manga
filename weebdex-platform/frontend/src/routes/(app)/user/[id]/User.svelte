<script lang="ts">
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Mutex, Permission } from "$lib";
	import type { UpdateSettingsRequest } from "$lib/API/Settings";
	import { useComments } from "$lib/components/comments/context.svelte";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import Badge from "$lib/components/ui/badge/badge.svelte";
	import { Button } from "$lib/components/ui/button";
	import { buttonVariants } from "$lib/components/ui/button/button.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import { setChaptersCount, setUser } from "$lib/Context";
	import { Session } from "$lib/stores";
	import { prefs } from "$lib/stores/prefs.js";
	import { cn } from "$lib/Utils";
	import {
		BanIcon,
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

	const {
		data,
		children
	}: {
		data: User;
		children: any;
	} = $props();

	const mutex = new Mutex();
	const user = writable<User>(data);
	const comments = useComments();

	let isFollowed = $state(false);
	let isBlocked = $derived($prefs.uploaders.some(e => e.id === $user.id));
	let chaptersCount = $state<number>();

	setUser(user);
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
				(chaptersCount ?? $user.relationships?.stats?.chapters)
					? `Uploads (${(chaptersCount ?? $user.relationships?.stats?.chapters)!.toLocaleString()})`
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
			hidden: !$Session || !Permission.User.canUpdate($user)
		}
	});

	onMount(async () => {
		if ($user) {
			mutex.exec(async () => {
				await Promise.all([getFollowStatus(), getChaptersCount()]);
			});
		}
	});

	const getFollowStatus = async () => {
		if (!(await Session.isValid())) {
			return;
		}

		try {
			const res = await API.User.GetFollowStatus($user.id);
			isFollowed = res?.status === StatusCodes.NO_CONTENT;
		} catch (e) {
			console.error(e);
		}
	};

	const getChaptersCount = async () => {
		try {
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);
			prefs.applyChapterPreferences(q);
			q.set("uploader", $user.id);
			q.set("limit", "0");

			const res = await API.Chapter.Search(q);
			if (res?.status === StatusCodes.OK && res.body) {
				chaptersCount = res.body.total;
			}
		} catch (e) {
			console.error(e);
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
				const res = await (isFollowed ? API.User.Unfollow : API.User.Follow)($user.id);
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
				const idx = $prefs.uploaders.findIndex(({ id }) => id === $user.id);
				if (idx >= 0) $prefs.uploaders.splice(idx, 1);
				else $prefs.uploaders.push({ id: $user.id, name: $user.name, version: $user.version });

				if ($Session) {
					const payload: UpdateSettingsRequest = {
						version: $prefs.version,
						uploaders: $prefs.uploaders.map(({ id }) => id)
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
	<title>{$user.name} - {PUBLIC_SITE_NAME}</title>
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
					<h1 class="truncate text-xl font-semibold">{$user.name}</h1>
					<strong class="text-muted-foreground truncate font-medium">@{$user.id}</strong>
					{#if $user.roles?.length}
						<div class="mt-2 flex flex-wrap items-center gap-1">
							{#each $user.roles
								.slice()
								.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b)) as role}
								<Badge
									class={cn("bg-input/30 border-none", {
										"text-orange-600":
											role === Constants.User.Role.Sysop ||
											role === Constants.User.Role.Staff ||
											role === Constants.User.Role.Moderator,
										"text-green-600": role === Constants.User.Role.PowerUploader,
										"text-emerald-600": role === Constants.User.Role.Contributor,
										"text-sky-600": role === Constants.User.Role.GroupLeader,
										"text-cyan-600": role === Constants.User.Role.GroupOfficer
									})}
									variant="outline"
								>
									{Constants.User.RoleText[role]}
								</Badge>
							{/each}
						</div>
					{/if}
				</div>
				<div class="flex shrink-0 flex-wrap items-center gap-1">
					{#if $Session && $user.id !== $Session.identity.id}
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
							ReportDialog.set($user, Constants.EntityType.User);
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
						{@const pathname = key === "home" ? `/user/${$user.id}` : `/user/${$user.id}/${key}`}
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
