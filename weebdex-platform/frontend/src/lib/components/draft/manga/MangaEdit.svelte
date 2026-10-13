<script lang="ts">
	import { goto } from "$app/navigation";
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { Constants, ExternalLink, Mutex, Permission, Utils } from "$lib";
	import API from "$lib/API";
	import type { UpdateCoverRequest } from "$lib/API/Cover";
	import type { UpdateMangaRequest } from "$lib/API/Manga";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import CoversSection from "$lib/components/draft/manga/CoversSection.svelte";
	import LinksSection from "$lib/components/draft/manga/LinksSection.svelte";
	import MetadataSection from "$lib/components/draft/manga/MetadataSection.svelte";
	import RelationsSection from "$lib/components/draft/manga/RelationsSection.svelte";
	import TitleSection from "$lib/components/draft/manga/TitleSection.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import { Label } from "$lib/components/ui/label";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tabs from "$lib/components/ui/tabs/index.js";
	import { populateTagList } from "$lib/stores";
	import { CheckIcon, Link, LockIcon, LockOpen, SendIcon, TrashIcon, XIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onDestroy, onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import Guidelines from "./Guidelines.svelte";
	import TagsSection from "./TagsSection.svelte";

	const { data }: { data: { manga: Manga; covers: Cover[] } } = $props();
	const { pathname } = window.location;
	const mutex = new Mutex();
	const STATE = {
		Idle: 0,
		UpdatingEntry: 1,
		UploadingCovers: 2,
		UpdatingCovers: 3,
		DeletingCovers: 4,
		CreatingRelations: 5,
		UpdatingRelations: 6,
		DeletingRelations: 7
	} as const;
	let mutexState = $state<(typeof STATE)[keyof typeof STATE]>(STATE.Idle);

	// svelte-ignore state_referenced_locally
	let manga = $state<Manga>(data.manga);
	// svelte-ignore state_referenced_locally
	let covers = $state<CoverDraft[]>(
		data.covers.map(cover => ({
			...cover,
			valid: true,

			main: cover.id === data.manga.relationships.cover?.id,
			url: `${PUBLIC_COVER_URL}/${data.manga.id}/${cover.id}${cover.ext}`,
			thumbnail: `${PUBLIC_COVER_URL}/${data.manga.id}/${cover.id}.512.webp`
		}))
	);

	const initial = $derived.by<MangaDraft>(() => {
		const snapshot = $state.snapshot(manga);
		return {
			title: snapshot.title,
			description: snapshot.description,
			year: snapshot.year ?? null!,
			language: snapshot.language,
			demographic: snapshot.demographic ?? null!,
			status: snapshot.status,
			content_rating: snapshot.content_rating,

			last_volume: snapshot.last_volume ?? null!,
			last_chapter: snapshot.last_chapter ?? null!,

			covers: covers ? structuredClone($state.snapshot(covers)) : [],
			relations: snapshot.relationships.relations ? structuredClone(snapshot.relationships.relations) : [],

			alt_titles: Object.entries(snapshot.alt_titles ?? [])
				.map(([language, titles]) => titles.map(title => ({ language, title, valid: true })))
				.flat(),
			tags: snapshot.relationships.tags?.map(e => e.id).sort((a, b) => a.localeCompare(b)) ?? [],
			authors: <Author[]>(<any>(snapshot.relationships.authors?.sort((a, b) => a.name.localeCompare(b.name)) ?? [])),
			artists: <Author[]>(<any>(snapshot.relationships.artists?.sort((a, b) => a.name.localeCompare(b.name)) ?? [])),
			links: Object.entries(snapshot.relationships.links ?? []).map(([code, link]) => ({
				code,
				link: ExternalLink.build(code, link),
				valid: true
			}))
		};
	});
	// svelte-ignore state_referenced_locally
	let draft = $state<MangaDraft>(structuredClone($state.snapshot(initial)));

	const isCompletedOrCancelled = $derived(
		[Constants.Manga.PublicationStatus.Completed, Constants.Manga.PublicationStatus.Cancelled].includes(draft.status)
	);
	const isVolumeValid = $derived(
		isCompletedOrCancelled && draft.last_volume ? Utils.isValidVolumeChapter(draft.last_volume) : true
	);
	const isChapterValid = $derived(
		isCompletedOrCancelled && draft.last_chapter ? Utils.isValidVolumeChapter(draft.last_chapter) : true
	);

	const isChanged = $derived(
		!equal(initial, {
			...draft,
			alt_titles: draft.alt_titles.filter(title => title.language && title.title),
			links: draft.links.filter(link => link.code && link.link)
		})
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
			if (!manga || !initial || !draft || !isChanged || !isValid) {
				return;
			}

			try {
				await updateManga();
				await processCovers();
				await processRelations();

				onReset();
				toast.success("Title updated successfully");
			} catch (e) {
				let prefix = "";
				switch (mutexState) {
					case STATE.UpdatingEntry:
						prefix = "Failed to update entry";
						break;
					case STATE.UploadingCovers:
						prefix = "Failed to upload covers";
						break;
					case STATE.UpdatingCovers:
						prefix = "Failed to update covers";
						break;
					case STATE.DeletingCovers:
						prefix = "Failed to delete covers";
						break;
					case STATE.CreatingRelations:
						prefix = "Failed to create relations";
						break;
					case STATE.UpdatingRelations:
						prefix = "Failed to update relations";
						break;
					case STATE.DeletingRelations:
						prefix = "Failed to delete relations";
						break;
				}
				if (prefix) {
					prefix += ": ";
				}

				if (e instanceof Constants.Error.Error) toast.error(prefix + e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(prefix + Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				mutexState = STATE.Idle;
			}
		});
	};

	const onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		draft = structuredClone($state.snapshot(initial));
	};

	const updateManga = async () => {
		if (!manga || !draft) {
			return;
		}

		const payload: UpdateMangaRequest = { version: manga.version };
		Object.entries(draft).forEach(([key, value]) => {
			if (!equal(value, initial[key as keyof MangaDraft])) {
				switch (key) {
					case "year":
						payload.year = value || 0;
						break;
					case "demographic":
						payload.demographic = value || Constants.Manga.Demographic.None;
						break;

					case "alt_titles":
						payload.alt_titles = {};
						(value as { language: string; title: string }[]).forEach(({ language, title }) => {
							(payload.alt_titles![language] ??= []).push(title);
						});
						break;

					case "covers":
					case "relations":
						break;

					case "authors":
					case "artists":
						(payload[key as keyof UpdateMangaRequest] as string[]) = (value as Tag[]).map(({ id }) => id);
						break;

					case "links":
						payload.links = (value as ExternalLink[]).reduce(
							(acc, { code, link }) => {
								link = ExternalLink.validate(code, link);
								if (link) {
									acc[code] = link;
								}
								return acc;
							},
							{} as Record<string, string>
						);
						break;

					default:
						(payload[key as keyof UpdateMangaRequest] as any) = value;
				}
			}
		});
		if (Object.keys(payload).length <= 1) {
			return;
		}

		mutexState = STATE.UpdatingEntry;
		const res = await API.Manga.Update(manga.id, payload);
		if (window.location.pathname !== pathname) {
			return;
		}

		if (res?.status === StatusCodes.OK && res.body) {
			manga = res.body;
		} else if (res?.status === StatusCodes.FORBIDDEN) {
			throw Constants.Error.Forbidden;
		} else if (res?.status === StatusCodes.CONFLICT) {
			throw Constants.Error.UpdateConflict;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};

	const processCovers = async () => {
		if (!manga || !draft) {
			return;
		}

		let tmp: CoverDraft[] = [];
		for (const coverDraft of draft.covers) {
			if (window.location.pathname !== pathname) {
				return;
			}

			if (coverDraft.file) {
				const form = new FormData();
				form.append("file", coverDraft.file!);
				form.append("language", coverDraft.language!);
				if (coverDraft.volume) form.append("volume", coverDraft.volume);
				if (coverDraft.description) form.append("description", coverDraft.description);
				if (coverDraft.main) form.append("main", "true");

				mutexState = STATE.UploadingCovers;
				const res = await API.Cover.Upload(manga.id, form);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					coverDraft.file = undefined;
					tmp.push({
						...coverDraft,
						...res.body
					});
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else if (res?.status === StatusCodes.INTERNAL_SERVER_ERROR && res.body?.error) {
					throw new Constants.Error.Error("Internal Server Error", res.body.error);
				} else throw Constants.Error.Unknown;
			} else if (coverDraft.deleted) {
				mutexState = STATE.DeletingCovers;
				const res = await API.Cover.Delete(coverDraft.id!);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					//
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} else {
				const initialCover = initial.covers.find(e => e.id === coverDraft.id);
				if (!initialCover || equal(initialCover, coverDraft)) {
					if (initialCover) {
						tmp.push(initialCover);
					}
					continue;
				}

				const payload: UpdateCoverRequest = { version: coverDraft.version };
				if (coverDraft.volume !== initialCover.volume) payload.volume = coverDraft.volume;
				if (coverDraft.description !== initialCover.description) payload.description = coverDraft.description;
				if (coverDraft.language !== initialCover.language) payload.language = coverDraft.language;
				if (coverDraft.main != initialCover.main) {
					if (coverDraft.main) payload.main = coverDraft.main;
					else if (initialCover) initialCover.main = false;
				}

				if (Object.keys(payload).length <= 1) {
					tmp.push(initialCover);
					continue;
				}

				mutexState = STATE.UpdatingCovers;
				const res = await API.Cover.Update(coverDraft.id!, payload);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					tmp.push({
						...coverDraft,
						...res.body
					});
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			}
		}
		covers = tmp;
	};

	const processRelations = async () => {
		if (!manga || !draft) {
			return;
		}

		let tmp: RelationDraft[] = [];
		for (const relationDraft of draft.relations) {
			if (window.location.pathname !== pathname) return;
			if (relationDraft.id === manga.id) continue;

			if (relationDraft.deleted) {
				mutexState = STATE.DeletingRelations;
				const res = await API.Manga.DeleteRelation(manga.id, {
					related_id: relationDraft.id
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					//
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} else {
				if (!relationDraft.new) {
					const initialRelation = initial.relations.find(e => e.id === relationDraft.id);
					if (initialRelation && equal(initialRelation, relationDraft)) {
						tmp.push(initialRelation);
						continue;
					}
				}

				mutexState = relationDraft.new ? STATE.CreatingRelations : STATE.UpdatingRelations;
				const res = await API.Manga.CreateRelation(manga.id, {
					related_id: relationDraft.id,
					type: relationDraft.type
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					tmp.push(res.body);
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			}
		}
		manga.relationships.relations = tmp;
	};

	const onCommit = () =>
		mutex.exec(async () => {
			try {
				const res = await API.Manga.Commit(manga.id, manga.version);
				if (res?.status === StatusCodes.OK && res.body) {
					manga = res.body;
					toast.success("Draft submitted successfully");
				} else if (res?.status === StatusCodes.BAD_REQUEST && res?.body?.error) {
					throw new Constants.Error.Error("Failed to submit draft", res.body.error);
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});

	let isPublishing = $state(false);
	const onPublish = () =>
		mutex.exec(async () => {
			if (manga.state !== Constants.State.Submitted) {
				return;
			}

			isPublishing = true;
			try {
				const res = await API.Manga.Publish(manga.id, manga.version);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Draft published successfully");
					manga.state = Constants.State.Published;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isPublishing = false;
			}
		});

	let isRejecting = $state(false);
	const onReject = () =>
		mutex.exec(async () => {
			if (manga.state !== Constants.State.Submitted) {
				return;
			}

			isRejecting = true;
			try {
				const res = await API.Manga.Reject(manga.id, manga.version);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Draft rejected successfully");
					manga.state = Constants.State.Rejected;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isRejecting = false;
			}
		});

	let isUpdatingLock = $state(false);
	const updateLockState = () =>
		mutex.exec(async () => {
			isUpdatingLock = true;
			try {
				const res = await (manga.locked ? API.Manga.Unlock : API.Manga.Lock)(manga.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Lock state updated successfully");
					manga.locked = !manga.locked;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isUpdatingLock = false;
			}
		});

	const onDelete = () =>
		mutex.exec(async () => {
			try {
				const res = await API.Manga.Delete(manga.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Draft deleted successfully");
					goto("/drafts/title");
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});
</script>

{#if mutexState === STATE.Idle}
	{@const canSubmit =
		[Constants.State.Draft, Constants.State.Rejected].includes(manga.state) && Permission.Manga.canUpdate(manga)}
	{@const canUpdateState = manga.state === Constants.State.Submitted && Permission.Manga.canUpdateState()}
	{@const canUpdateLock = Permission.Manga.canUpdateLock()}

	<form class="flex flex-auto flex-col space-y-5" onsubmit={onSubmit} onreset={onReset}>
		<div class="flex flex-wrap items-center justify-between gap-4">
			<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
				<div class="space-y-1">
					<Label>Created At</Label>
					<time datetime={manga.created_at}>
						{new Date(manga.created_at).toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" })}
					</time>
				</div>
				<div class="space-y-1">
					<Label>Last Updated At</Label>
					<div>
						<time datetime={manga.updated_at}>
							{new Date(manga.updated_at).toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" })}
						</time>
						<span>(Version {manga.version})</span>
					</div>
				</div>
				<div class="space-y-1">
					<Label>Status</Label>
					<div>
						<span
							class:text-slate-400={manga.state === Constants.State.Draft}
							class:text-orange-400={manga.state === Constants.State.Submitted}
							class:text-red-400={manga.state === Constants.State.Rejected}
							class:text-green-400={manga.state === Constants.State.Published}
						>
							{Constants.StateText[manga.state]}
						</span>
					</div>
				</div>
				{#if manga.relationships.submitter && manga.relationships.submitter.name !== "system"}
					<div class="space-y-1">
						<Label>Submitted By</Label>
						<a
							class="hover:text-accent-foreground flex items-center gap-1"
							href="/user/{manga.relationships.submitter.id}"
						>
							<Link class="size-3.5 shrink-0" />
							{manga.relationships.submitter.name}
						</a>
					</div>
				{/if}
			</div>
			{#if canSubmit || canUpdateState || canUpdateLock}
				<div class="flex items-center gap-1">
					{#if canSubmit}
						<ConfirmDialog
							title="Submit Draft"
							description="Are you sure you want to submit this draft?"
							confirmText="Submit"
							variant="default"
							onConfirm={e => e.exec(onCommit)}
							disabled={$mutex}
						>
							{#snippet child({ props })}
								<Button class="w-24" type="button" size="sm" {...props}>
									<SendIcon />
									Submit
								</Button>
							{/snippet}
						</ConfirmDialog>
					{/if}
					{#if canUpdateState}
						<ConfirmDialog
							title="Reject Draft"
							description="Are you sure you want to reject this draft?"
							confirmText="Reject"
							variant="destructive"
							onConfirm={async e => e.exec(onReject)}
							disabled={$mutex}
						>
							{#snippet child({ props })}
								<Button class="w-24 border-none !text-red-400" variant="outline" type="button" size="sm" {...props}>
									{#if $mutex && isRejecting}
										<Spinner />
									{:else}
										<XIcon />
										Reject
									{/if}
								</Button>
							{/snippet}
						</ConfirmDialog>
						<ConfirmDialog
							title="Publish Draft"
							description="Are you sure you want to publish this draft?"
							confirmText="Publish"
							variant="default"
							onConfirm={async e => e.exec(onPublish)}
							disabled={$mutex}
						>
							{#snippet child({ props })}
								<Button class="w-24 border-none !text-green-400" variant="outline" type="button" size="sm" {...props}>
									{#if $mutex && isPublishing}
										<Spinner />
									{:else}
										<CheckIcon />
										Publish
									{/if}
								</Button>
							{/snippet}
						</ConfirmDialog>
					{/if}
					{#if canUpdateLock}
						<ConfirmDialog
							title="{manga.locked ? 'Unlock' : 'Lock'} {manga.state === Constants.State.Published ? 'Title' : 'Draft'}"
							description="Are you sure you want to {manga.locked ? 'unlock' : 'lock'} this {manga.state ===
							Constants.State.Published
								? 'title'
								: 'draft'}?"
							confirmText={manga.locked ? "Unlock" : "Lock"}
							variant={manga.locked ? "default" : "destructive"}
							onConfirm={async e => e.exec(updateLockState)}
							disabled={$mutex}
						>
							{#snippet child({ props })}
								<Button class="w-24 border-none" type="button" variant="outline" size="sm" {...props}>
									{#if $mutex && isUpdatingLock}
										<Spinner />
									{:else if manga.locked}
										<LockOpen />
										Unlock
									{:else}
										<LockIcon />
										Lock
									{/if}
								</Button>
							{/snippet}
						</ConfirmDialog>
					{/if}
				</div>
			{/if}
		</div>
		<Separator />

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
				<TagsSection bind:draft enabled={Permission.Manga.canEditTags(manga)} disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="links">
				<LinksSection bind:draft enabled={Permission.Manga.canEditLinks(manga)} disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="relations">
				<RelationsSection bind:draft enabled={Permission.Manga.canEditRelations(manga)} disabled={$mutex} />
			</Tabs.Content>
			<Tabs.Content class="space-y-5" value="covers">
				<CoversSection bind:draft enabled={Permission.Manga.canEditCovers(manga)} disabled={$mutex} />
			</Tabs.Content>
		</Tabs.Root>

		<Separator />
		<div class="bg-background sticky bottom-0 flex flex-wrap items-center justify-between gap-1 py-1">
			<div>
				{#if Permission.Manga.canDelete(manga)}
					<ConfirmDialog
						title={manga.state === Constants.State.Published ? "Delete Title" : "Delete Draft"}
						description={manga.state === Constants.State.Published
							? "Are you sure you want to delete this title?"
							: "Are you sure you want to delete this draft?"}
						confirmText="Delete"
						variant="destructive"
						onConfirm={e => e.exec(onDelete)}
						disabled={$mutex}
					>
						{#snippet child({ props })}
							<Button class="w-24" variant="destructive" {...props}>
								<TrashIcon />
								Delete
							</Button>
						{/snippet}
					</ConfirmDialog>
				{/if}
			</div>
			<div class="flex items-center gap-1">
				<Button
					class="!text-destructive w-24 border-none"
					variant="outline"
					type="reset"
					disabled={$mutex || !isChanged}
				>
					Revert
				</Button>
				<Button class="w-30" type="submit" disabled={$mutex || !isChanged || !isValid}>Save</Button>
			</div>
		</div>
		<Separator />

		<div class="space-y-2">
			<strong class="block text-xl font-semibold">Guidelines</strong>
			<Guidelines />
		</div>
	</form>
{:else}
	<Empty.Root>
		<Empty.Header>
			<Empty.Media>
				<Spinner class="size-10" />
			</Empty.Media>
			<Empty.Title>
				{#if mutexState === STATE.UpdatingEntry}
					Updating Draft
				{:else if mutexState === STATE.UploadingCovers}
					Uploading Covers
				{:else if mutexState === STATE.UpdatingCovers}
					Updating Covers
				{:else if mutexState === STATE.DeletingCovers}
					Deleting Covers
				{:else if mutexState === STATE.CreatingRelations}
					Creating Relations
				{:else if mutexState === STATE.UpdatingRelations}
					Updating Relations
				{:else if mutexState === STATE.DeletingRelations}
					Deleting Relations
				{/if}
			</Empty.Title>
		</Empty.Header>
	</Empty.Root>
{/if}
