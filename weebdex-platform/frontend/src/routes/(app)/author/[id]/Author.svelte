<script lang="ts">
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Permission } from "$lib";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import { Button } from "$lib/components/ui/button";
	import { buttonVariants } from "$lib/components/ui/button/button.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { setAuthor, setChaptersCount } from "$lib/Context";
	import { Session } from "$lib/stores";
	import { prefs } from "$lib/stores/prefs.js";
	import { cn } from "$lib/Utils";
	import { FilesIcon, FlagIcon, InfoIcon, SettingsIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { writable } from "svelte/store";

	const {
		data,
		children
	}: {
		data: Author;
		children: any;
	} = $props();

	// svelte-ignore state_referenced_locally
	const author = writable<Author>(data);
	let chaptersCount = $state<number>();

	setAuthor(author);
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
			name: chaptersCount ? `Uploads (${chaptersCount.toLocaleString()})` : `Uploads`,
			icon: FilesIcon
		},
		settings: {
			name: "Settings",
			icon: SettingsIcon,
			hidden: !$Session || !Permission.Author.canUpdate($author)
		}
	});

	onMount(async () => {
		if ($author) {
			await Promise.all([getChaptersCount()]);
		}
	});

	const getChaptersCount = async () => {
		try {
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);
			prefs.applyChapterPreferences(q);
			q.set("authorOrArtist", $author.id);
			q.set("limit", "0");

			const res = await API.Chapter.Search(q);
			if (res?.status === StatusCodes.OK && res.body) {
				chaptersCount = res.body.total;
			}
		} catch (e) {
			console.error(e);
		}
	};
</script>

<svelte:head>
	<title>{$author.name} - {PUBLIC_SITE_NAME}</title>
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
					<h1 class="truncate text-2xl font-semibold">{$author.name}</h1>
					<strong class="text-muted-foreground truncate font-medium">@{$author.id}</strong>
				</div>
				<div class="flex shrink-0 flex-wrap items-center gap-1">
					<Button
						class="border-none"
						variant="outline"
						size="sm"
						onclick={() => {
							ReportDialog.set($author, Constants.EntityType.Author);
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
						{@const pathname = key === "home" ? `/author/${$author.id}` : `/author/${$author.id}/${key}`}
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
