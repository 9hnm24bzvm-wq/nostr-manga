<script lang="ts">
	import { goto } from "$app/navigation";
	import { PUBLIC_UPLOAD_URL } from "$env/static/public";
	import { Constants, Mutex, Permission, Utils } from "$lib";
	import API from "$lib/API";
	import type { UpdateChapterRequest } from "$lib/API/Chapter";
	import ReportMangaCard from "$lib/components/ReportMangaCard.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button/index.js";
	import Label from "$lib/components/ui/label/label.svelte";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import UploadChapterGroups from "$lib/components/upload/UploadChapterGroups.svelte";
	import UploadChapterMetadata from "$lib/components/upload/UploadChapterMetadata.svelte";
	import { setMutex, setUploadSession } from "$lib/Context.js";
	import { from } from "$lib/stores";
	import { ArrowLeftIcon, CrownIcon, ExternalLink, Link, TrashIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import ConfirmDialog from "../ConfirmDialog.svelte";
	import UploadChapterPages from "./UploadChapterPages.svelte";

	let {
		chapter = $bindable(),
		manga,
		user
	}: {
		chapter: Chapter;
		manga: Manga;
		user: User;
	} = $props();
	const mutex = new Mutex();

	let uploadSession = $state<UploadSession>();
	const initial = $derived({
		title: chapter.title ?? "",
		volume: chapter.volume ?? "",
		chapter: chapter.chapter ?? "",
		language: chapter.language ?? "",
		groups: chapter.relationships.groups ?? []
	});
	let draft = $state<typeof initial>();

	const isVolumeValid = $derived(draft?.volume ? Utils.isValidVolumeChapter(draft.volume) : true);
	const isChapterValid = $derived(draft?.chapter ? Utils.isValidVolumeChapter(draft.chapter) : true);

	let isOneshot = $state(!chapter.volume && !chapter.chapter);
	let isChanged = $derived(!equal(draft, initial));
	let isValid = $derived.by(() => {
		if (uploadSession) {
			return (
				uploadSession.relationships.files?.length &&
				uploadSession.relationships.files.every(e => e.id.length === 10 && !e.file && !e.error)
			);
		}
		return draft?.language?.length && (isOneshot || (isVolumeValid && isChapterValid));
	});

	setMutex(mutex);
	setUploadSession(() => uploadSession);
	onMount(() => onReset());

	const onReset = async (ev?: Event & { currentTarget: HTMLFormElement }) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		draft = structuredClone($state.snapshot(initial));
		isOneshot = !chapter.volume && !chapter.chapter;
	};

	let isSubmitting = $state(false);
	const onSubmit = async (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!draft || !isValid || !isChanged) {
				return;
			}

			const { pathname } = window.location;
			isSubmitting = true;

			try {
				const payload: UpdateChapterRequest = { version: chapter.version };
				Object.entries(draft).forEach(([key, value]) => {
					if (!equal(value, initial![key as keyof typeof initial])) {
						if (key === "groups") payload.groups = (value as ScanlationGroup[])?.map(({ id }) => id) ?? [];
						else (payload[key as keyof UpdateChapterRequest] as any) = value;
					}
				});
				if (Object.keys(payload).length <= 1) {
					return;
				}

				const res = await API.Chapter.Update(chapter.id, payload);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					toast.success("Chapter updated successfully", {
						description: "It may take a few minutes for the changes to be reflected."
					});
					res.body.relationships.groups ??= [];
					chapter = res.body;
					onReset();
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				isSubmitting = false;
			}
		});
	};

	let isClaiming = $state(false);
	const onClaim = () =>
		mutex.exec(async () => {
			isClaiming = true;
			try {
				const res = await API.Chapter.Claim(chapter.id);
				if (res?.status === StatusCodes.OK && res.body) {
					toast.success("Chapter claimed successfully", {
						description: "It may take a few minutes for the change to be reflected."
					});
					chapter = res.body;
					onReset();
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				isClaiming = false;
			}
		});

	let isDeleting = $state(false);
	const onDelete = async () =>
		mutex.exec(async () => {
			const { pathname } = window.location;
			isDeleting = true;

			try {
				const res = await API.Chapter.Delete(chapter.id);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Chapter deleted successfully", {
						description: "It may take a few minutes for the change to be reflected."
					});
					if ($from && !$from.pathname.startsWith("/chapter")) {
						await goto($from);
					} else await goto(`/title/${manga.id}${Utils.makeSlug(manga.title)}`);
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				isDeleting = false;
			}
		});

	let isCreatingUploadSession = $state(false);
	let submitter = $state<string>();

	const createUploadSession = (ev: SubmitEvent) =>
		mutex.exec(async () => {
			if (uploadSession) {
				return;
			}

			const { pathname } = window.location;
			isCreatingUploadSession = true;
			submitter = (<HTMLButtonElement>ev?.submitter)?.value;

			try {
				await deleteExistingUploadSession();

				const res = await API.UploadSession.BeginEdit(chapter.id, {
					replace: submitter === "replace",
					version: chapter.version
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.CREATED && res.body) {
					res.body.relationships.files =
						res.body.relationships.files?.map((e, i) => ({
							...e,

							original_file_name: (i + 1).toString() + e.file_ext,
							thumbnail: `${PUBLIC_UPLOAD_URL}/${e.file_name}`
						})) ?? [];
					uploadSession = res.body;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				submitter = undefined;
				isCreatingUploadSession = false;
			}
		});

	const commitUploadSession = async (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!uploadSession?.relationships?.files?.length || !isValid) {
				return;
			}

			const { pathname } = window.location;
			isSubmitting = true;

			try {
				const res = await API.UploadSession.Commit(uploadSession.id, {
					draft: {
						volume: chapter.volume,
						chapter: chapter.chapter,
						title: chapter.title,
						language: chapter.language
					},
					page_order: uploadSession.relationships.files.map(e => e.id)
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					toast.success("Pages updated successfully", {
						description: "It may take a few minutes for the changes to be reflected."
					});
					uploadSession = undefined;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				isSubmitting = false;
			}
		});
	};

	const deleteExistingUploadSession = async () => {
		const res = await API.UploadSession.Get();
		if (res?.status === StatusCodes.OK && res.body?.id) {
			await API.UploadSession.Delete(res.body.id);
		} else if (res?.status === StatusCodes.FORBIDDEN) {
			throw Constants.Error.Forbidden;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else if (res?.status !== StatusCodes.NO_CONTENT) {
			throw Constants.Error.Unknown;
		}
	};

	const cancelUploadSession = () =>
		mutex.exec(async () => {
			if (!uploadSession) {
				return;
			}

			await API.UploadSession.Delete(uploadSession.id);
			uploadSession = undefined;
		});
</script>

{#if draft}
	<section class="mx-auto flex w-full max-w-270 flex-auto flex-col space-y-5">
		<header class="flex items-center gap-2">
			<a
				class={buttonVariants({ class: "rounded-full", variant: "ghost", size: "icon" })}
				href={$from && !$from.pathname.startsWith("/chapter")
					? $from.href
					: `/title/${manga.id}${Utils.makeSlug(manga.title)}`}
			>
				<ArrowLeftIcon class="size-5" />
			</a>
			<h2 class="text-xl font-medium">Edit Chapter</h2>
		</header>
		<form
			class="space-y-5"
			onsubmit={uploadSession ? commitUploadSession : onSubmit}
			onreset={uploadSession ? undefined : onReset}
		>
			<div class="space-y-2">
				<Label>Title</Label>
				<ReportMangaCard {manga} />
			</div>

			<Separator />
			<div class="flex flex-wrap items-center justify-between gap-4">
				<div class="flex items-center gap-x-4 gap-y-2">
					<div class="space-y-1">
						<Label>Created At</Label>
						<time datetime={chapter.created_at}>
							{new Date(chapter.created_at).toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" })}
						</time>
					</div>
					<div class="space-y-1">
						<Label>Last Updated At</Label>
						<div>
							<time datetime={chapter.updated_at}>
								{new Date(chapter.updated_at).toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" })}
							</time>
							<span>(Version {chapter.version})</span>
						</div>
					</div>
					{#if chapter.relationships.uploader && chapter.relationships.uploader.name !== "system"}
						<div class="space-y-1">
							<Label>Uploaded By</Label>
							<a
								class="hover:text-accent-foreground flex items-center gap-1"
								href="/user/{chapter.relationships.uploader.id}"
							>
								<Link class="size-3.5 shrink-0" />
								{chapter.relationships.uploader.name}
							</a>
						</div>
					{/if}
					{#if chapter.source_id}
						<div class="space-y-1">
							<Label>Source</Label>
							{#if chapter.source_id.startsWith("mangadex:")}
								{@const id = chapter.source_id.replace("mangadex:", "")}
								<a
									class="hover:text-accent-foreground flex items-center gap-1"
									href={`https://mangadex.org/chapter/${id}`}
									target="_blank"
									rel="noopener noreferrer nofollow"
								>
									<ExternalLink class="size-3.5 shrink-0" />
									{id}
								</a>
							{:else}
								<span>{chapter.source_id}</span>
							{/if}
						</div>
					{/if}
				</div>
				{#if Permission.Chapter.canClaim(chapter)}
					<div class="flex items-center gap-1">
						<ConfirmDialog
							title="Claim Chapter"
							description="Are you sure you want to claim this chapter?"
							confirmText="Claim"
							variant="default"
							onConfirm={e => e.exec(onClaim)}
							disabled={$mutex}
						>
							{#snippet child({ props })}
								<Button class="w-24 border-none" type="button" variant="outline" size="sm" {...props}>
									{#if $mutex && isClaiming}
										<Spinner />
									{:else}
										<CrownIcon />
										Claim
									{/if}
								</Button>
							{/snippet}
						</ConfirmDialog>
					</div>
				{/if}
			</div>

			<Separator />
			<UploadChapterGroups
				{user}
				bind:groups={draft.groups}
				enabled={!uploadSession}
				disabled={$mutex || !!uploadSession}
			/>

			<Separator />
			<UploadChapterMetadata
				bind:draft
				bind:isOneshot
				{isVolumeValid}
				{isChapterValid}
				disabled={$mutex || !!uploadSession}
			/>

			{#if uploadSession}
				<Separator />
				<UploadChapterPages bind:pages={uploadSession.relationships.files!} disabled={$mutex} />
			{/if}

			<Separator />
			<div class="bg-background sticky bottom-0 flex flex-wrap items-center justify-between gap-1 py-1">
				<div>
					{#if !uploadSession}
						{#if Permission.Chapter.canRemove(chapter)}
							<ConfirmDialog
								title="Delete Chapter"
								description="Are you sure you want to delete this chapter?"
								confirmText="Delete"
								variant="destructive"
								onConfirm={e => e.exec(onDelete)}
								disabled={$mutex}
							>
								{#snippet child({ props })}
									<Button class="w-24" variant="destructive" {...props}>
										{#if $mutex && isDeleting}
											<Spinner />
										{:else}
											<TrashIcon />
											Delete
										{/if}
									</Button>
								{/snippet}
							</ConfirmDialog>
						{/if}
					{/if}
				</div>
				<div class="flex flex-wrap items-center gap-1">
					{#if uploadSession}
						<Button class="border-none" variant="outline" type="button" onclick={cancelUploadSession} disabled={$mutex}>
							Cancel
						</Button>
						<Button class="w-30" type="submit" disabled={$mutex || !isValid}>
							{#if $mutex && isSubmitting}
								<Spinner />
							{:else}
								Save
							{/if}
						</Button>
					{:else}
						<Button
							class="!text-destructive w-24 border-none"
							variant="outline"
							type="reset"
							disabled={$mutex || !isValid || !isChanged}
						>
							Revert
						</Button>
						<Button class="w-30" type="submit" disabled={$mutex || !isValid || !isChanged}>
							{#if $mutex && isSubmitting}
								<Spinner />
							{:else}
								Save
							{/if}
						</Button>
					{/if}
				</div>
			</div>
		</form>

		{#if !uploadSession}
			<Separator />
			<form class="space-y-4" onsubmit={createUploadSession}>
				<div class="flex flex-wrap items-center gap-1">
					<Button class="w-28 border-none" variant="outline" type="submit" value="edit" disabled={$mutex}>
						{#if isCreatingUploadSession && submitter === "edit"}
							<Spinner />
						{:else}
							Edit Pages
						{/if}
					</Button>
					<Button class="w-34 border-none" variant="outline" type="submit" value="replace" disabled={$mutex}>
						{#if isCreatingUploadSession && submitter === "replace"}
							<Spinner />
						{:else}
							Replace Pages
						{/if}
					</Button>
				</div>
				<ul>
					<li>
						<strong class="font-semibold">Edit Pages:</strong> creates an edit session with all existing pages. This function
						may take a while to load if the chapter has a large number of pages.
					</li>
					<li>
						<strong class="font-semibold">Replace Pages:</strong> creates an edit session without any existing pages. Upon
						saving, it will replace and delete all previous pages.
					</li>
				</ul>
			</form>
		{/if}
	</section>
{/if}
