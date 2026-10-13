<script lang="ts">
	import { Constants, Locale, Mutex, Permission } from "$lib";
	import API from "$lib/API";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import { buttonVariants } from "$lib/components/ui/button";
	import Button from "$lib/components/ui/button/button.svelte";
	import { Label } from "$lib/components/ui/label";
	import { ExternalLinkIcon, Link, Pencil, SendIcon, TrashIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	let {
		entries = $bindable(),
		manga = $bindable()
	}: {
		entries: Manga[];
		manga: Manga;
	} = $props();

	const mutex = new Mutex();

	const onCommit = () =>
		mutex.exec(async () => {
			try {
				const res = await API.Manga.Commit(manga.id, manga.version);
				if (res?.status === StatusCodes.OK && res.body) {
					manga = res.body;
					entries = entries;
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

	const onDelete = () =>
		mutex.exec(async () => {
			try {
				const res = await API.Manga.Delete(manga.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					const idx = entries.findIndex(m => m.id === manga.id);
					if (idx >= 0) {
						entries.splice(idx, 1);
						entries = entries;
					}
					toast.success("Draft deleted successfully");
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
</script>

<article
	class="bg-input/30 relative flex flex-col space-y-5 overflow-hidden rounded-md border-l-2 p-2 pl-3"
	class:border-l-slate-500={manga.state === Constants.State.Draft}
	class:border-l-red-500={manga.state === Constants.State.Rejected}
	class:border-l-orange-500={manga.state === Constants.State.Submitted}
	class:border-l-green-500={manga.state === Constants.State.Published}
>
	<div class="flex flex-auto">
		<div class="w-0 flex-auto space-y-2">
			<strong class="text-muted-foreground block font-medium">#{manga.id}</strong>
			<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
				{#if manga.relationships.submitter}
					<div class="space-y-1">
						<Label>Submitted by</Label>
						<a
							class="hover:text-accent-foreground flex items-center gap-1"
							href="/user/{manga.relationships.submitter.id}"
							title={manga.relationships.submitter.name}
						>
							<Link class="size-3.5 shrink-0" />
							{manga.relationships.submitter.name}
						</a>
					</div>
				{/if}
				<div class="space-y-1">
					<Label>Title</Label>
					<div class="gap-2">
						<img
							class="inline size-4 shrink-0"
							src="/images/flags/{Locale.to3166(manga.language)}.svg"
							alt={Locale.toLanguage(manga.language)}
						/>
						<h3 class="inline font-medium">{manga.title}</h3>
					</div>
				</div>
			</div>
		</div>
		<div class="shrink-0">
			{#if [Constants.State.Draft, Constants.State.Rejected].includes(manga.state) && Permission.Manga.canUpdate(manga)}
				<ConfirmDialog
					title="Submit Draft"
					description="Are you sure you want to submit this draft?"
					confirmText="Submit"
					variant="default"
					onConfirm={e => e.exec(onCommit)}
					disabled={$mutex}
				>
					{#snippet child({ props })}
						<Button variant="ghost" size="icon-sm" title="Submit" {...props}>
							<SendIcon class="size-5" />
						</Button>
					{/snippet}
				</ConfirmDialog>
			{/if}
			{#if Permission.Manga.canDelete(manga)}
				<ConfirmDialog
					title="Delete Draft"
					description="Are you sure you want to delete this draft?"
					confirmText="Delete"
					variant="destructive"
					onConfirm={e => e.exec(onDelete)}
					disabled={$mutex}
				>
					{#snippet child({ props })}
						<Button variant="ghost" size="icon-sm" title="Delete" {...props}>
							<TrashIcon class="size-5" />
						</Button>
					{/snippet}
				</ConfirmDialog>
			{/if}
			{#if Permission.Manga.canUpdate(manga)}
				<a
					class={buttonVariants({ class: "cursor-pointer", variant: "ghost", size: "icon-sm" })}
					href="/title/{manga.id}/edit{new URLSearchParams({
						...(manga.state === Constants.State.Published ? {} : { draft: 'true' })
					})}"
					title="Edit"
				>
					<Pencil class="size-5" />
				</a>
			{/if}
			<a
				class={buttonVariants({ class: "cursor-pointer", variant: "ghost", size: "icon-sm" })}
				href="/title/{manga.id}{new URLSearchParams({
					...(manga.state === Constants.State.Published ? {} : { draft: 'true' })
				})}"
				title={manga.state === Constants.State.Published ? "View title" : "Preview draft"}
			>
				<ExternalLinkIcon class="size-5" />
			</a>
		</div>
	</div>
	<div class="flex items-center gap-2">
		<div class="w-0 flex-auto"></div>
		<div class="flex shrink-0 items-center gap-2">
			<time datetime={manga.created_at}>{manga.created_at.ago()}</time>
			<strong
				class="font-semibold"
				class:text-slate-400={manga.state === Constants.State.Draft}
				class:text-orange-400={manga.state === Constants.State.Submitted}
				class:text-red-400={manga.state === Constants.State.Rejected}
				class:text-green-400={manga.state === Constants.State.Published}
				>[{Constants.StateText[manga.state]}]
			</strong>
		</div>
	</div>
</article>
