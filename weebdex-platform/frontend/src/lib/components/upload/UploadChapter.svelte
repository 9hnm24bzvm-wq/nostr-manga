<script lang="ts">
	import { goto } from "$app/navigation";
	import { Constants, Mutex, Utils } from "$lib";
	import API from "$lib/API";
	import Guidelines from "$lib/components/Guidelines.svelte";
	import ReportMangaCard from "$lib/components/ReportMangaCard.svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import Checkbox from "$lib/components/ui/checkbox/checkbox.svelte";
	import * as Dialog from "$lib/components/ui/dialog";
	import Label from "$lib/components/ui/label/label.svelte";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import UploadChapterGroups from "$lib/components/upload/UploadChapterGroups.svelte";
	import UploadChapterMetadata from "$lib/components/upload/UploadChapterMetadata.svelte";
	import UploadChapterPages from "$lib/components/upload/UploadChapterPages.svelte";
	import { setMutex, setUploadSession } from "$lib/Context.js";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	const {
		manga,
		user
	}: {
		manga: Manga;
		user: User;
	} = $props();
	const mutex = new Mutex();

	let uploadSession = $state<UploadSession>();
	let draft = $state<ChapterDraft>({});
	let groups = $state<ScanlationGroup[]>([]);

	const isVolumeValid = $derived(draft?.volume ? Utils.isValidVolumeChapter(draft.volume) : true);
	const isChapterValid = $derived(draft?.chapter ? Utils.isValidVolumeChapter(draft.chapter) : true);

	let isOneshot = $state(false);
	let isTemsAccepted = $state(false);
	let isValid = $derived(
		isTemsAccepted &&
			draft?.language?.length &&
			(isOneshot || (isVolumeValid && isChapterValid)) &&
			uploadSession?.relationships.files?.length &&
			uploadSession?.relationships?.files?.every(e => e.id.length === 10 && !e.file && !e.error)
	);

	setMutex(mutex);
	setUploadSession(() => uploadSession);

	let submitter = $state<string>();
	const createUploadSession = async (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (uploadSession) {
				return;
			}

			const { pathname } = window.location;
			submitter = (<HTMLButtonElement>ev?.submitter)?.value;

			try {
				const res = await API.UploadSession.Begin(
					manga.id,
					groups.map(e => e.id)
				);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.CREATED && res.body) {
					(res.body.relationships ??= {}).files = [];
					uploadSession = res.body;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				submitter = undefined;
			}
		});
	};

	let message = $state<{ title: string; description: string }>();
	let isShowMessage = $state(false);
	let isBeginAnotherUploadSession = $state(false);
	let chapter = $state<Chapter>();

	const commitUploadSession = async (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			const target = <HTMLButtonElement>ev.submitter;
			if (!uploadSession?.relationships?.files?.length || !isValid || !target) {
				return;
			}

			const { pathname } = window.location;
			submitter = target.value;
			isBeginAnotherUploadSession = target.value === "batch";

			try {
				const res = await API.UploadSession.Commit(uploadSession.id, {
					draft,
					page_order: uploadSession.relationships.files.map(e => e.id)
				});
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					uploadSession.is_committed = true;

					if (await isPending()) {
						message = {
							title: "Chapter added to approval queue",
							description:
								"As you have not had a chapter approved for this language before, this chapter has been added to the approval queue.<br/><br/> Chapters generally take a day or two to be approved."
						};
					} else {
						chapter = res.body;
						message = {
							title: "Chapter uploaded successfully",
							description:
								"Chapter has been uploaded successfully. It may take a few minutes to process before it becomes visible to the public."
						};
					}
					isShowMessage = true;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			} finally {
				submitter = undefined;
			}
		});
	};

	const isPending = async () => {
		if (!draft.language) {
			throw Constants.Error.Unknown;
		}

		const res = await API.UploadSession.CheckApprovalRequired(draft.language);
		if (res?.status === StatusCodes.OK) {
			return true;
		} else if (res?.status === StatusCodes.NO_CONTENT) {
			return false;
		} else if (res?.status === StatusCodes.FORBIDDEN) {
			throw Constants.Error.Forbidden;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

<Dialog.Root
	bind:open={isShowMessage}
	onOpenChange={async open => {
		if (open) return;

		isShowMessage = false;
		if (isBeginAnotherUploadSession) {
			isBeginAnotherUploadSession = false;
			uploadSession = undefined;
			draft = { language: draft.language };
			isOneshot = false;

			createUploadSession();
		} else {
			if (chapter) await goto(`/chapter/${chapter.id}/1`);
			else await goto(`/title/${manga.id}${Utils.makeSlug(manga.title)}`);
		}
	}}
>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>{message?.title}</Dialog.Title>
			<Dialog.Description>
				{@html message?.description}
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Dialog.Close>
				{#snippet child({ props })}
					<Button class="border-none" variant="outline" type="button" {...props}>Close</Button>
				{/snippet}
			</Dialog.Close>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<section class="mx-auto flex w-full max-w-270 flex-auto flex-col space-y-5">
	<header class="space-y-5">
		<h2 class="text-xl font-medium">Upload Chapter</h2>
	</header>
	<form class="space-y-5" onsubmit={uploadSession ? commitUploadSession : createUploadSession}>
		<div class="space-y-2">
			<Label>Title</Label>
			<ReportMangaCard {manga} />
		</div>

		<Separator />
		<UploadChapterGroups {user} bind:groups enabled={!uploadSession} disabled={$mutex || !!uploadSession} />

		{#if uploadSession}
			<Separator />
			<UploadChapterMetadata
				bind:draft
				bind:isOneshot
				{isVolumeValid}
				{isChapterValid}
				disabled={($mutex && !!submitter) || !uploadSession}
			/>

			<Separator />
			<UploadChapterPages bind:pages={uploadSession.relationships.files!} disabled={$mutex || !uploadSession} />

			<Separator />
			<div class="space-y-2">
				<div class="flex items-center">
					<Checkbox id="terms" bind:checked={isTemsAccepted} />
					<Label class="block cursor-pointer pl-2" for="terms">
						I have read and agree to the
						<a
							class="hover:text-accent-foreground underline"
							href="/terms"
							target="_blank"
							onclick={e => {
								e.stopPropagation();
								e.stopImmediatePropagation();
							}}
						>
							Terms of Service
						</a>
						and
						<a
							class="hover:text-accent-foreground underline"
							href="/guidelines"
							target="_blank"
							onclick={e => {
								e.stopPropagation();
								e.stopImmediatePropagation();
							}}
						>
							Guidelines
						</a>
					</Label>
				</div>
			</div>
		{/if}

		<Separator />
		<div class="bg-background sticky bottom-0 flex flex-wrap justify-end gap-1 py-1">
			{#if uploadSession}
				<Button class="w-50 border-none" variant="outline" type="submit" value="batch" disabled={$mutex || !isValid}>
					{#if $mutex && submitter === "batch"}
						<Spinner />
					{:else}
						Upload and add another
					{/if}
				</Button>
				<Button class="w-30" type="submit" value="commit" disabled={$mutex || !isValid}>
					{#if $mutex && submitter === "commit"}
						<Spinner />
					{:else}
						Upload
					{/if}
				</Button>
			{:else}
				<Button
					class="w-60 border-none"
					variant="outline"
					type="submit"
					value="nogroup"
					disabled={$mutex || !!groups.length}
				>
					{#if $mutex && submitter === "nogroup"}
						<Spinner />
					{:else}
						Not affiliated with any group
					{/if}
				</Button>
				<Button class="w-30" type="submit" value="group" disabled={$mutex || !groups.length}>
					{#if $mutex && submitter === "group"}
						<Spinner />
					{:else}
						Confirm
					{/if}
				</Button>
			{/if}
		</div>
	</form>
	<div class="space-y-2">
		<strong class="block text-xl font-semibold">Guidelines</strong>
		<Guidelines />
	</div>
</section>
