<script lang="ts">
	import { API, Constants, ExternalLink, Mutex, Permission, Utils } from "$lib";
	import type { CreateMangaRequest } from "$lib/API/Manga";
	import CoversSection from "$lib/components/draft/manga/CoversSection.svelte";
	import LinksSection from "$lib/components/draft/manga/LinksSection.svelte";
	import MetadataSection from "$lib/components/draft/manga/MetadataSection.svelte";
	import RelationsSection from "$lib/components/draft/manga/RelationsSection.svelte";
	import TitleSection from "$lib/components/draft/manga/TitleSection.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tabs from "$lib/components/ui/tabs/index.js";
	import { populateTagList } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Check, XIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onDestroy, onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import Guidelines from "./Guidelines.svelte";
	import TagsSection from "./TagsSection.svelte";

	const { pathname } = window.location;
	const mutex = new Mutex();
	const STATE = {
		Idle: 0,
		CreatingEntry: 1,
		UploadingCovers: 2,
		CreatingRelations: 3,
		Done: 4,
		Error: 5
	} as const;
	let mutexState = $state<(typeof STATE)[keyof typeof STATE]>(STATE.Idle);

	let manga = $state<Manga>();
	let draft = $state<MangaDraft>({
		title: "",
		language: "ja",
		status: Constants.Manga.PublicationStatus.Ongoing,
		content_rating: Constants.Manga.ContentRating.Safe,

		covers: [],
		alt_titles: [],
		tags: [],
		authors: [],
		artists: [],
		links: [],
		relations: []
	});

	const isCompletedOrCancelled = $derived(
		[Constants.Manga.PublicationStatus.Completed, Constants.Manga.PublicationStatus.Cancelled].includes(draft.status)
	);
	const isVolumeValid = $derived(
		isCompletedOrCancelled && draft.last_volume ? Utils.isValidVolumeChapter(draft.last_volume) : true
	);
	const isChapterValid = $derived(
		isCompletedOrCancelled && draft.last_chapter ? Utils.isValidVolumeChapter(draft.last_chapter) : true
	);
	const isValid = $derived(
		draft?.title &&
			isVolumeValid &&
			isChapterValid &&
			draft.authors.length &&
			draft.artists.length &&
			draft.covers.every(e => e.valid) &&
			draft.alt_titles.every(e => e.valid) &&
			draft.links.every(e => e.valid)
	);

	onMount(async () => {
		try {
			await populateTagList();
		} catch (e) {
			console.error(e);
		}
	});

	onDestroy(() => {
		draft.covers.forEach(cover => {
			if (cover.thumbnail && cover.thumbnail.startsWith("blob:")) {
				URL.revokeObjectURL(cover.thumbnail);
			}
		});
	});

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		return mutex.exec(async () => {
			if (!draft || !isValid) {
				return;
			}

			try {
				await createManga();
				await uploadCovers();
				await createRelations();

				mutexState = STATE.Done;
			} catch (e) {
				let prefix = "";
				switch (mutexState) {
					case STATE.CreatingEntry:
						prefix = "Failed to create manga entry";
						break;
					case STATE.UploadingCovers:
						prefix = "Failed to upload covers";
						break;
					case STATE.CreatingRelations:
						prefix = "Failed to create relations";
						break;
				}
				if (prefix) {
					prefix += ": ";
				}

				mutexState = mutexState > STATE.CreatingEntry ? STATE.Error : STATE.Idle;
				if (e instanceof Constants.Error.Error) toast.error(prefix + e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(prefix + Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});
	};

	const createManga = async () => {
		if (!draft) {
			return;
		}

		const payload: CreateMangaRequest = {
			title: draft.title,
			description: draft.description || undefined,
			year: draft.year || undefined,
			language: draft.language,
			demographic: draft.demographic || undefined,
			status: draft.status,
			content_rating: draft.content_rating,

			last_volume: draft.last_volume || undefined,
			last_chapter: draft.last_chapter || undefined,

			alt_titles: draft.alt_titles.reduce(
				(acc, { language, title }) => {
					acc[language] ??= [];
					acc[language].push(title);
					return acc;
				},
				{} as Record<string, string[]>
			),

			tags: draft.tags,
			authors: draft.authors.map(e => e.id),
			artists: draft.artists.map(e => e.id),

			links: draft.links.reduce(
				(acc, { code, link }) => {
					link = ExternalLink.validate(code, link);
					if (link) acc[code] = link;
					return acc;
				},
				{} as Record<string, string>
			)
		};

		mutexState = STATE.CreatingEntry;
		const res = await API.Manga.Create(payload);
		if (window.location.pathname !== pathname) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			manga = res.body;
		} else if (res?.status === StatusCodes.FORBIDDEN) {
			throw Constants.Error.Forbidden;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};

	const uploadCovers = async () => {
		if (!manga || !draft) {
			return;
		}

		mutexState = STATE.UploadingCovers;
		for (const coverDraft of draft.covers) {
			if (window.location.pathname !== pathname) {
				return;
			}

			const form = new FormData();
			form.append("file", coverDraft.file!);
			form.append("language", coverDraft.language!);
			if (coverDraft.volume) form.append("volume", coverDraft.volume);
			if (coverDraft.description) form.append("description", coverDraft.description);
			if (coverDraft.main) form.append("main", "true");

			const res = await API.Cover.Upload(manga.id, form);
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				coverDraft.file = undefined;
			} else if (res?.status === StatusCodes.FORBIDDEN) {
				throw Constants.Error.Forbidden;
			} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
				throw Constants.Error.TooManyRequests;
			} else if (res?.status === StatusCodes.INTERNAL_SERVER_ERROR && res.body?.error) {
				throw new Constants.Error.Error("Internal Server Error", res.body.error);
			} else throw Constants.Error.Unknown;
		}
	};

	const createRelations = async () => {
		if (!manga || !draft) {
			return;
		}

		mutexState = STATE.CreatingRelations;
		for (const relationDraft of draft.relations) {
			if (window.location.pathname !== pathname) {
				return;
			}

			const res = await API.Manga.CreateRelation(manga.id, {
				related_id: relationDraft.id,
				type: relationDraft.type
			});
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				//
			} else if (res?.status === StatusCodes.FORBIDDEN) {
				throw Constants.Error.Forbidden;
			} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
				throw Constants.Error.TooManyRequests;
			} else throw Constants.Error.Unknown;
		}
	};
</script>

{#if mutexState === STATE.Idle}
	<form class="flex flex-auto flex-col space-y-5" onsubmit={onSubmit}>
		<Tabs.Root class="flex-auto space-y-5" value="title">
			<Tabs.List>
				<Tabs.Trigger value="title">Title</Tabs.Trigger>
				<Tabs.Trigger value="metadata">Metadata</Tabs.Trigger>
				<Tabs.Trigger value="tags">Tags</Tabs.Trigger>
				<Tabs.Trigger value="links">Links</Tabs.Trigger>
				<Tabs.Trigger value="relations">Relations</Tabs.Trigger>
				<Tabs.Trigger value="covers">Covers</Tabs.Trigger>
			</Tabs.List>
			<Tabs.Content class="space-y-5" value="title">
				<TitleSection bind:draft disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="metadata">
				<MetadataSection bind:draft {isCompletedOrCancelled} {isVolumeValid} {isChapterValid} disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="tags">
				<TagsSection bind:draft enabled disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="links">
				<LinksSection bind:draft enabled disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="relations">
				<RelationsSection bind:draft enabled={Permission.Manga.canEditRelations()} disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="covers">
				<CoversSection bind:draft enabled disabled={$mutex} />
			</Tabs.Content>
		</Tabs.Root>
		<Separator />
		<div class="bg-background sticky bottom-0 flex items-center justify-end gap-1 py-1">
			<Button type="submit" disabled={$mutex || !isValid}>Save</Button>
		</div>
	</form>
	<div class="space-y-2">
		<strong class="block text-xl font-semibold">Guidelines</strong>
		<Guidelines />
	</div>
{:else}
	<Empty.Root>
		<Empty.Header>
			<Empty.Media>
				<Check class={cn("size-10 scale-0 transition", mutexState === STATE.Done && "scale-100")} />
				<XIcon class={cn("absolute size-10 scale-0 transition", mutexState === STATE.Error && "scale-100")} />
				<Spinner
					class={cn(
						"absolute size-10 scale-0 transition",
						![STATE.Done, STATE.Error].includes(<any>mutexState) && "scale-100 "
					)}
				/>
			</Empty.Media>
			<Empty.Title>
				{#if mutexState === STATE.CreatingEntry}
					Creating Draft
				{:else if mutexState === STATE.UploadingCovers}
					Uploading Covers
				{:else if mutexState === STATE.CreatingRelations}
					Creating Relations
				{:else if mutexState === STATE.Done}
					Draft successfully created
				{:else if mutexState === STATE.Error}
					Error
				{/if}
			</Empty.Title>
		</Empty.Header>
		{#if manga && mutexState === STATE.Done}
			<Empty.Content>
				<div class="flex flex-wrap items-center justify-center gap-2">
					<a
						class={buttonVariants({ class: "border-none", variant: "outline" })}
						href="/title/{manga.id}/edit{new URLSearchParams({
							...(manga.state === Constants.State.Published ? {} : { draft: 'true' })
						})}"
					>
						Edit draft
					</a>
					<a
						class={buttonVariants({ class: "border-none", variant: "outline" })}
						href="/title/{manga.id}{new URLSearchParams({
							...(manga.state === Constants.State.Published ? {} : { draft: 'true' })
						})}"
					>
						Preview draft
					</a>
					<a class={buttonVariants({ class: "border-none", variant: "outline" })} href="/drafts/title">
						View my drafts
					</a>
				</div>
			</Empty.Content>
		{:else if mutexState === STATE.Error}
			<Empty.Content>
				<div class="flex flex-wrap items-center justify-center gap-2">
					<a class={buttonVariants({ class: "border-none", variant: "outline" })} href="/drafts/title">
						View my drafts
					</a>
				</div>
			</Empty.Content>
		{/if}
	</Empty.Root>
{/if}
