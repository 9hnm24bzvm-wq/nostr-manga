<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants, Mutex, Permission } from "$lib";
	import ChapterListEntry from "$lib/components/ChapterListEntry.svelte";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as Select from "$lib/components/ui/select";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { getManga, getReadHistory } from "$lib/Context";
	import { prefs, Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Bell, BellRing, BookOpen, ChevronDownIcon, FlagIcon, Pencil, UploadIcon } from "@lucide/svelte";
	import { Select as SelectPrimitive } from "bits-ui";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const mutex = new Mutex();
	const manga = $derived(getManga()());
	const isPublished = $derived(manga.state === Constants.State.Published);
	const readHistory = $derived(getReadHistory());

	let isEmpty = $derived(!manga.relationships.available_languages || !manga.relationships.stats?.chapters);
	let currentReadingStatus = $state<Constants.Manga.ReadingStatus>();
	let isUpdatingReadingStatus = $state(false);
	let currentNotificationStatus = $state<boolean>();
	let isUpdatingNotificationStatus = $state(false);

	let isStartReading = $state(false);
	let isChapterSelectionDialogOpen = $state(false);
	let targetAggregatedChapter = $state<ChapterAggregate>();
	let aggregated = $state<ChapterAggregateResponse>();

	onMount(async () => {
		await Session.isValid();

		const status = $Session?.follows?.[manga.id];
		if (status) {
			currentReadingStatus = status[0];
			currentNotificationStatus = status[1] === 1;
		}
	});

	const updateFollowStatus = (readingStatus: Constants.Manga.ReadingStatus) => {
		if (!$Session || readingStatus === currentReadingStatus || !isPublished) {
			return;
		}

		if (currentReadingStatus && !readingStatus) {
			return unfollow();
		}
		return mutex.exec(async () => {
			const { pathname } = window.location;
			isUpdatingReadingStatus = true;
			try {
				const res = await API.MangaFollow.Update(manga.id, {
					status: readingStatus,
					notification: currentNotificationStatus ?? true
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					currentReadingStatus = readingStatus;
					currentNotificationStatus = currentNotificationStatus ?? true;
					if ($Session) {
						Object.assign(($Session.follows ??= {}), {
							[manga.id]: [readingStatus, currentNotificationStatus ? 1 : 0]
						});
					}
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
				isUpdatingReadingStatus = false;
			}
		});
	};

	const updateNotificationStatus = (notificationStatus: boolean) =>
		mutex.exec(async () => {
			if (!$Session || !currentReadingStatus || !isPublished) {
				return;
			}

			const { pathname } = window.location;
			isUpdatingNotificationStatus = true;
			try {
				const res = await API.MangaFollow.Update(manga.id, {
					status: currentReadingStatus,
					notification: notificationStatus
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					currentNotificationStatus = notificationStatus;
					if ($Session) {
						Object.assign(($Session.follows ??= {}), {
							[manga.id]: [currentReadingStatus, notificationStatus ? 1 : 0]
						});
					}
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
				isUpdatingNotificationStatus = false;
			}
		});

	const unfollow = () =>
		mutex.exec(async () => {
			if (!$Session || !isPublished) {
				return;
			}

			const { pathname } = window.location;
			isUpdatingReadingStatus = true;
			try {
				const res = await API.MangaFollow.Unfollow(manga.id);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					currentReadingStatus = undefined;
					currentNotificationStatus = undefined;
					if ($Session?.follows && manga.id in $Session.follows) {
						delete $Session.follows[manga.id];
					}
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isUpdatingReadingStatus = false;
			}
		});

	const startRead = () =>
		mutex.exec(async () => {
			if (!isPublished || isEmpty) {
				return;
			}

			isStartReading = true;
			try {
				if (!aggregated) {
					const { pathname } = window.location;
					const q = new URLSearchParams();
					prefs.applyChapterPreferences(q);

					if (page.url.searchParams.has("tlang")) {
						q.delete("tlang");
						page.url.searchParams.getAll("tlang").forEach(lang => q.append("tlang", lang));
					}
					if (page.url.searchParams.has("group")) {
						q.delete("group");
						page.url.searchParams.getAll("group").forEach(group => q.append("group", group));
					}

					const res = await API.Chapter.Aggregate(manga.id, q);
					if (window.location.pathname !== pathname) {
						return;
					}

					if (res?.status === StatusCodes.OK && res.body) {
						aggregated = res.body;
					} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
						throw Constants.Error.TooManyRequests;
					} else throw Constants.Error.Unknown;
				}

				if (aggregated?.chapters.length) {
					const history = $readHistory?.[manga.id];

					if (history) {
						const filtered = aggregated.chapters.filter(e => !Object.keys(e.entries).some(id => id in history));
						console.info(aggregated.chapters[aggregated.chapters.length - 1]);

						if (filtered.length) {
							targetAggregatedChapter = filtered[filtered.length - 1];
						} else targetAggregatedChapter = aggregated.chapters[aggregated.chapters.length - 1];
					} else targetAggregatedChapter = aggregated.chapters[aggregated.chapters.length - 1];

					if (targetAggregatedChapter) {
						const entries = Object.entries(targetAggregatedChapter.entries);
						if (entries.length === 1) await goto(`/chapter/${entries[0][0]}/1`);
						else isChapterSelectionDialogOpen = true;
					}
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isStartReading = false;
			}
		});
</script>

<div class="flex flex-wrap items-center gap-1">
	<div class="min-w-45">
		{#if $Session}
			<ButtonGroup.Root class="w-full">
				<Select.Root
					type="single"
					value={currentReadingStatus}
					onValueChange={value => updateFollowStatus(<Constants.Manga.ReadingStatus>value)}
					disabled={isUpdatingReadingStatus || isUpdatingNotificationStatus || !isPublished}
				>
					<SelectPrimitive.Trigger class={cn(buttonVariants({ class: "w-0 flex-auto" }))} data-slot="select-trigger">
						<span class="w-0 flex-auto">
							{#if currentReadingStatus}
								{Constants.Manga.ReadingStatusText[currentReadingStatus]}
							{:else}
								Add to Library
							{/if}
						</span>
						<div class="relative size-4 shrink-0 opacity-50">
							<ChevronDownIcon class={cn("scale-100 transition-all", isUpdatingReadingStatus && "scale-0")} />
							<Spinner
								class={cn("absolute top-0 left-0 scale-0 transition-all", isUpdatingReadingStatus && "scale-100 ")}
							/>
						</div>
					</SelectPrimitive.Trigger>
					<Select.Content>
						<Select.Item value="">None</Select.Item>
						{#each Object.entries(Constants.Manga.ReadingStatusText) as [key, value]}
							<Select.Item value={key}>{value}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				{#if currentReadingStatus}
					<Button
						type="button"
						variant="secondary"
						onclick={() => updateNotificationStatus(!currentNotificationStatus)}
						disabled={isUpdatingReadingStatus || isUpdatingNotificationStatus || !isPublished}
					>
						<Bell
							class={cn(
								"scale-100 transition-all",
								(isUpdatingNotificationStatus || currentNotificationStatus) && "scale-0"
							)}
						/>
						<BellRing
							class={cn(
								"absolute scale-0 transition-all",
								!isUpdatingNotificationStatus && currentNotificationStatus && "scale-100 "
							)}
						/>
						<Spinner class={cn("absolute scale-0 transition-all", isUpdatingNotificationStatus && "scale-100 ")} />
					</Button>
				{/if}
			</ButtonGroup.Root>
		{:else}
			<a class={cn(buttonVariants({ class: "w-full" }))} href="/auth/login{page.url.searchParams.toString()}">
				Add to Library
			</a>
		{/if}
	</div>
	<Button
		class="border-none"
		type="button"
		variant="outline"
		onclick={startRead}
		disabled={isStartReading || !isPublished || isEmpty}
	>
		{#if isStartReading}
			<Spinner />
		{:else}
			<BookOpen />
		{/if}
		{#if $readHistory?.[manga.id]}
			Continue Reading
		{:else}
			Start Reading
		{/if}
	</Button>
	<Button
		class="border-none"
		type="button"
		variant="outline"
		onclick={() => {
			ReportDialog.set(manga, Constants.EntityType.Manga);
			ReportDialog.open();
		}}
		disabled={!isPublished}
	>
		<FlagIcon />
		Report
	</Button>
	{#if isPublished}
		<a class={buttonVariants({ class: "border-none", variant: "outline" })} href="/title/{manga.id}/upload">
			<UploadIcon />
			Upload Chapter
		</a>
	{/if}
	{#if $Session && Permission.Manga.canUpdate(manga)}
		<a
			class={buttonVariants({ class: "border-none", variant: "outline" })}
			href="/title/{manga.id}/edit{new URLSearchParams({
				...(isPublished ? {} : { draft: 'true' })
			})}"
		>
			<Pencil />
			Edit
		</a>
	{/if}
</div>

<Dialog.Root bind:open={isChapterSelectionDialogOpen}>
	<Dialog.Content class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(80rem,calc(100dvw-2rem))] flex-col overflow-auto">
		<Dialog.Header>
			<Dialog.Title>Select Chapter</Dialog.Title>
		</Dialog.Header>
		{#if aggregated?.chapters.length && targetAggregatedChapter}
			<div class="mt-1 space-y-0.5">
				{#each Object.entries(targetAggregatedChapter.entries) as [id, entry]}
					<ChapterListEntry
						class="bg-input/30 hover:bg-input/55 rounded-md"
						chapter={{
							id: id,
							chapter: targetAggregatedChapter.chapter,
							volume: targetAggregatedChapter.volume,
							created_at: null!,
							published_at: entry.published_at!,
							updated_at: null!,
							language: aggregated.languages[entry.language],
							relationships: {
								manga: manga,
								groups: entry.groups.map(idx => aggregated!.groups[idx]),
								thread: null!
							},
							version: undefined!
						}}
					/>
				{/each}
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
