<script lang="ts">
	import { Constants, Mutex, Permission, Typography, Utils } from "$lib";
	import API from "$lib/API";
	import { CheckIcon, ExternalLinkIcon, FilterIcon, InfoIcon, Link, TrashIcon, XIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";
	import ConfirmDialog from "./ConfirmDialog.svelte";
	import ReportAuthorCard from "./ReportAuthorCard.svelte";
	import ReportChapterCard from "./ReportChapterCard.svelte";
	import ReportGroupCard from "./ReportGroupCard.svelte";
	import ReportMangaCard from "./ReportMangaCard.svelte";
	import ReportReplyCard from "./ReportReplyCard.svelte";
	import { applyFilter } from "./ReportSearchFilter.svelte";
	import ReportUserCard from "./ReportUserCard.svelte";
	import TextAreaDialog from "./TextAreaDialog.svelte";
	import { Button, buttonVariants } from "./ui/button";
	import * as Dialog from "./ui/dialog";
	import { Label } from "./ui/label";
	import Scrollable from "./ui/Scrollable.svelte";
	import { Separator } from "./ui/separator";
	import { Spinner } from "./ui/spinner";

	let {
		entries = $bindable(),
		report = $bindable()
	}: {
		entries: Report[];
		report: Report;
	} = $props();

	const mutex = new Mutex();
	const link = $derived.by(() => {
		switch (report.relationships.reason.category) {
			case Constants.EntityType.User:
				return report.relationships.user ? `/user/${report.relationships.user.id}` : undefined;
			case Constants.EntityType.Manga:
				return report.relationships.manga ? `/title/${report.relationships.manga.id}` : undefined;
			case Constants.EntityType.Author:
				return report.relationships.author ? `/author/${report.relationships.author.id}` : undefined;
			case Constants.EntityType.Chapter:
				return report.relationships.chapter ? `/chapter/${report.relationships.chapter.id}` : undefined;
			case Constants.EntityType.ScanlationGroup:
				return report.relationships.group ? `/group/${report.relationships.group.id}` : undefined;
			case Constants.EntityType.Thread:
				return report.relationships.thread ? `/forums/threads/${report.relationships.thread.id}` : undefined;
			case Constants.EntityType.Reply:
				return report.relationships.reply
					? `/forums/threads/${report.relationships.reply.relationships.thread?.id}/${report.relationships.reply.id}`
					: undefined;
		}
	});

	let isDeleting = $state(false);
	const deleteReport = () =>
		mutex.exec(async () => {
			isDeleting = true;
			try {
				const res = await API.Report.Delete(report.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					const idx = entries.findIndex(e => e.id === report.id);
					if (idx >= 0) {
						entries.splice(idx, 1);
						entries = entries;
					}
					toast.success("Report deleted successfully");
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
				isDeleting = false;
			}
		});

	let mutexState = $state<Constants.Report.Status>();
	const updateStatus = (status: Constants.Report.Status, notes?: string) =>
		mutex.exec(async () => {
			mutexState = status;
			try {
				const res = await (status === Constants.Report.Status.Refused
					? API.Report.Reject(report.id, notes)
					: API.Report.Accept(report.id, notes));
				if (res?.status === StatusCodes.OK && res.body) {
					toast.success(
						`Report #${report.id} has been ${status === Constants.Report.Status.Refused ? "refused" : "accepted"}`
					);

					report = {
						...res.body,
						relationships: report.relationships
					};
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
			} finally {
				mutexState = undefined;
			}
		});
</script>

<article
	class="bg-input/30 relative flex flex-col space-y-5 overflow-hidden rounded-md border-l-2 p-2 pl-3"
	class:border-l-red-500={report.status === Constants.Report.Status.Refused}
	class:border-l-orange-500={report.status === Constants.Report.Status.Waiting}
	class:border-l-green-500={report.status === Constants.Report.Status.Accepted ||
		report.status === Constants.Report.Status.Autoresolved}
>
	<div class="flex flex-auto">
		<div class="w-0 flex-auto space-y-2">
			<strong class="text-muted-foreground block font-medium">
				#{report.id}
			</strong>
			<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
				<div class="space-y-1">
					<Label>Category</Label>
					<span>
						{Constants.EntityTypeText[report.relationships.reason.category]}
					</span>
				</div>
				<div class="space-y-1">
					<Label>Reason</Label>
					<span>{report.relationships.reason.reason}</span>
				</div>
				{#if report.relationships.submitter}
					<div class="space-y-1">
						<Label>Reported by</Label>
						<a
							class="hover:text-accent-foreground flex items-center gap-1"
							href="/user/{report.relationships.submitter.id}"
							title={report.relationships.submitter.name}
						>
							<Link class="size-3.5 shrink-0" />
							{report.relationships.submitter.name}
						</a>
					</div>
				{/if}
				{#if report.notes?.length}
					<div class="space-y-1">
						<Label>Moderator Notes</Label>
						<span>{report.notes?.length.toLocaleString()} character(s)</span>
					</div>
				{/if}
			</div>
		</div>
		<div class="shrink-0">
			<Dialog.Root>
				<Dialog.Trigger
					class={buttonVariants({ class: "cursor-pointer", variant: "ghost", size: "icon-sm" })}
					title="Show report information"
				>
					<InfoIcon class="size-5" />
				</Dialog.Trigger>
				<Dialog.Content
					class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(48rem,calc(100dvw-2rem))] flex-col overflow-auto"
				>
					<Dialog.Header>
						<Dialog.Title>Report Information</Dialog.Title>
					</Dialog.Header>
					<div class="space-y-2">
						<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
							<div class="space-y-1">
								<Label>ID</Label>
								<span>{report.id}</span>
							</div>
							<div class="space-y-1">
								<Label>Reported At</Label>
								<time datetime={report.created_at}>
									{new Date(report.created_at).toLocaleString("en-US", {
										dateStyle: "short",
										timeStyle: "short"
									})}
								</time>
							</div>
							<div class="space-y-1">
								<Label>Reason</Label>
								<span>{report.relationships.reason.reason}</span>
							</div>
							<div class="space-y-1">
								<Label>Status</Label>
								<span
									class:text-red-400={report.status === Constants.Report.Status.Refused}
									class:text-orange-400={report.status === Constants.Report.Status.Waiting}
									class:text-green-400={report.status === Constants.Report.Status.Accepted ||
										report.status === Constants.Report.Status.Autoresolved}
								>
									{Constants.Report.StatusText[report.status]}
								</span>
							</div>
						</div>
						<Separator />

						{#if report.relationships.submitter}
							<div class="space-y-1">
								<Label>Reported By</Label>
								{#if report.relationships.submitter}
									<ReportUserCard user={report.relationships.submitter} />
								{:else}
									<div
										class="rounded-md border border-dashed border-red-500/50 bg-red-500/15 p-2 text-center text-red-300"
									>
										<strong class="font-semibold">Unknown</strong>
									</div>
								{/if}
							</div>
							<Separator />
						{/if}

						<div class="space-y-1">
							<Label>Subject</Label>
							{#if report.relationships.reason.category === Constants.EntityType.User && report.relationships.user}
								<ReportUserCard user={report.relationships.user} />
							{:else if report.relationships.reason.category === Constants.EntityType.Manga && report.relationships.manga}
								<ReportMangaCard manga={report.relationships.manga} />
							{:else if report.relationships.reason.category === Constants.EntityType.Author && report.relationships.author}
								<ReportAuthorCard author={report.relationships.author} />
							{:else if report.relationships.reason.category === Constants.EntityType.Chapter && report.relationships.chapter}
								{#if report.relationships.chapter.relationships.manga}
									<ReportMangaCard manga={report.relationships.chapter.relationships.manga} />
								{/if}
								<div
									class:ml-1={!!report.relationships.chapter.relationships.manga}
									class:border-l-1={!!report.relationships.chapter.relationships.manga}
									class:pl-1={!!report.relationships.chapter.relationships.manga}
								>
									<ReportChapterCard chapter={report.relationships.chapter} />
								</div>
							{:else if report.relationships.reason.category === Constants.EntityType.ScanlationGroup && report.relationships.group}
								<ReportGroupCard group={report.relationships.group} />
							{:else if report.relationships.reason.category === Constants.EntityType.Reply && report.relationships.reply}
								{#if report.relationships.reply.relationships.author}
									<ReportUserCard user={report.relationships.reply.relationships.author} />
								{/if}
								<div
									class:ml-1={!!report.relationships.reply.relationships.author}
									class:border-l-1={!!report.relationships.reply.relationships.author}
									class:pl-1={!!report.relationships.reply.relationships.author}
								>
									<ReportReplyCard reply={report.relationships.reply} />
								</div>
							{:else}
								<div
									class="rounded-md border border-dashed border-red-500/50 bg-red-500/15 p-2 text-center text-red-300"
								>
									<strong class="font-semibold">Unknown</strong>
								</div>
							{/if}
						</div>
						<Separator />

						<div class="space-y-2">
							<Label>
								Additional Details{#if !report.body}:{/if}
							</Label>
							<Scrollable class="ml-2 max-h-50 border-l-2 pl-2">
								<div class="typography" {@attach Typography.exec}>
									{#if report.body}
										{@html Utils.parseComment(report.body as any)}
									{:else}
										<p>N/A</p>
									{/if}
								</div>
							</Scrollable>
						</div>

						{#if report.notes?.length}
							<Separator />
							<div class="space-y-2">
								<Label>Moderator Notes:</Label>
								<Scrollable class="ml-2 max-h-50 border-l-2 pl-2">
									<div class="typography" {@attach Typography.exec}>
										{@html Utils.parseComment(report.notes as any)}
									</div>
								</Scrollable>
							</div>
						{/if}
					</div>
					<Dialog.Footer>
						<Dialog.Close class={buttonVariants({ class: "border-none", variant: "outline" })} disabled={$mutex}>
							Close
						</Dialog.Close>
						{#if Permission.Report.canDelete()}
							<ConfirmDialog
								variant="destructive"
								title="Delete Report"
								description="Are you sure you want to delete this report?"
								confirmText="Delete"
								onConfirm={m => m.exec(deleteReport)}
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
						{#if report.status === Constants.Report.Status.Waiting && Permission.Report.canUpdate()}
							<TextAreaDialog
								variant="destructive"
								title="Refuse Report"
								description="Are you sure you want to refuse this report?"
								placeholder="Reason (Optional)"
								confirmText="Refuse"
								onConfirm={(m, notes) => m.exec(() => updateStatus(Constants.Report.Status.Refused, notes))}
								disabled={$mutex}
							>
								{#snippet child({ props })}
									<Button class="w-24 border-none !text-red-400" variant="outline" {...props}>
										{#if $mutex && mutexState === Constants.Report.Status.Refused}
											<Spinner />
										{:else}
											<XIcon />
											Refuse
										{/if}
									</Button>
								{/snippet}
							</TextAreaDialog>
							<TextAreaDialog
								variant="default"
								title="Accept Report"
								description="Are you sure you want to accept this report?"
								placeholder="Reason (Optional)"
								confirmText="Accept"
								onConfirm={(m, notes) => m.exec(() => updateStatus(Constants.Report.Status.Accepted, notes))}
								disabled={$mutex}
							>
								{#snippet child({ props })}
									<Button class="w-24 border-none !text-green-400" variant="outline" {...props}>
										{#if $mutex && mutexState === Constants.Report.Status.Accepted}
											<Spinner />
										{:else}
											<CheckIcon />
											Accept
										{/if}
									</Button>
								{/snippet}
							</TextAreaDialog>
						{/if}
					</Dialog.Footer>
				</Dialog.Content>
			</Dialog.Root>
			<Button
				class="cursor-pointer"
				variant="ghost"
				size="icon-sm"
				title="Filter reports by this object"
				onclick={() => applyFilter(report)}
			>
				<FilterIcon class="size-5" />
			</Button>
			{#if link}
				<a
					class={buttonVariants({ class: "cursor-pointer", variant: "ghost", size: "icon-sm" })}
					href={link}
					title="Open subject"
				>
					<ExternalLinkIcon class="size-5" />
				</a>
			{/if}
		</div>
	</div>
	<div class="flex items-center gap-2">
		<div class="w-0 flex-auto"></div>
		<div class="flex shrink-0 items-center gap-2">
			<time datetime={report.created_at}>{report.created_at.ago()}</time>
			<strong
				class="font-semibold"
				class:text-red-400={report.status === Constants.Report.Status.Refused}
				class:text-orange-400={report.status === Constants.Report.Status.Waiting}
				class:text-green-400={report.status === Constants.Report.Status.Accepted ||
					report.status === Constants.Report.Status.Autoresolved}
			>
				[{Constants.Report.StatusText[report.status]}]
			</strong>
		</div>
	</div>
</article>
