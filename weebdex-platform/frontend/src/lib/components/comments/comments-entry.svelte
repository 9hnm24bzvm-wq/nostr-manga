<script lang="ts" module>
	const isRemoved = (reply: Reply) =>
		[
			Constants.Thread.ReplyState.DeletedByUser,
			Constants.Thread.ReplyState.DeletedByModerator,
			Constants.Thread.ReplyState.DeletedBySystem
		].includes(reply.state ?? <any>"");
</script>

<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { Constants, Mutex, Permission, Typography, Utils } from "$lib";
	import API from "$lib/API";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import * as ReportDialog from "$lib/components/ReportDialog.svelte";
	import AspectRatio from "$lib/components/ui/aspect-ratio/aspect-ratio.svelte";
	import * as Avatar from "$lib/components/ui/avatar";
	import { useKey } from "$lib/components/ui/key";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { Session } from "$lib/stores";
	import {
		PencilIcon,
		ReplyIcon,
		ThumbsDown,
		ThumbsUpIcon,
		TrashIcon,
		TriangleAlertIcon,
		UserIcon
	} from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount, tick } from "svelte";
	import { toast } from "svelte-sonner";
	import { useComments } from "./context.svelte";

	let { reply = $bindable() }: { reply: Reply } = $props();
	const { author } = reply.relationships;

	const mutex = new Mutex();
	const comments = useComments();
	const key = useKey();

	const createdAt = $derived(new Date(reply.created_at));
	const updatedAt = $derived(new Date(reply.updated_at));
	const role = $derived(
		author?.roles
			? author.roles
					.slice()
					.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b))[0]
			: undefined
	);

	let ref = $state<HTMLDivElement>();
	let isPoster = $derived(
		author?.id === comments.thread.relationships.author?.id ||
			author?.id === comments.thread.relationships.user?.id ||
			author?.id === comments.thread.relationships.chapter?.relationships.uploader?.id
	);
	let isVotable = $derived($Session && author?.id !== $Session?.identity.id);
	let isUpvoted = $derived(reply.relationships.vote_state === 1);
	let isDownvoted = $derived(reply.relationships.vote_state === -1);
	let voteScore = $derived(reply.relationships.stats.up - reply.relationships.stats.down);

	onMount(() => {
		const hash = page.url.hash;
		if (hash === `#p${reply.id}`) {
			window.requestAnimationFrame(async () => {
				await tick();

				ref?.scrollIntoView({ block: "center" });
				ref?.setAttribute("data-highlight", "true");
				setTimeout(() => ref?.removeAttribute("data-highlight"), 2000);
			});
		}
	});

	let mutexVoteState = $state<-1 | 1>();
	const onVote = (value: -1 | 1) => {
		if (reply.relationships.vote_state === value) {
			return onDeleteVote(value);
		}
		return mutex.exec(async () => {
			mutexVoteState = value;
			try {
				const res = await API.Thread.VoteReply(comments.thread.id, reply.id, value);
				if (res?.status === StatusCodes.NO_CONTENT) {
					if (reply.relationships.vote_state === -1) {
						reply.relationships.stats.down--;
					} else if (reply.relationships.vote_state === 1) {
						reply.relationships.stats.up--;
					}
					reply.relationships.stats[value === 1 ? "up" : "down"]++;
					reply.relationships.vote_state = value;
					reply = reply;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
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
				mutexVoteState = undefined;
			}
		});
	};

	const onDeleteVote = (value: -1 | 1) =>
		mutex.exec(async () => {
			mutexVoteState = value;
			try {
				const res = await API.Thread.UnvoteReply(comments.thread.id, reply.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					if (reply.relationships.vote_state === 1) {
						reply.relationships.stats.up--;
					} else if (reply.relationships.vote_state === -1) {
						reply.relationships.stats.down--;
					}
					reply.relationships.vote_state = undefined;
					reply = reply;
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
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
				mutexVoteState = undefined;
			}
		});

	const onRemove = () =>
		mutex.exec(async () => {
			if (!$Session) {
				return;
			}

			try {
				const res = await API.Thread.RemoveReply(comments.thread.id, reply.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Comment deleted successfully");
					replaceState(window.location.href, {
						...page.state,
						_$Replies_feed: undefined
					});

					key.refresh();
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

	const onDelete = () =>
		mutex.exec(async () => {
			if (!$Session) {
				return;
			}

			try {
				const res = await API.Thread.DeleteReply(comments.thread.id, reply.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Comment deleted successfully");
					replaceState(window.location.href, {
						...page.state,
						_$Replies_feed: undefined
					});

					key.refresh();
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

<div
	class="bg-card ring-accent/50 flex flex-col gap-2 rounded-md p-2 transition duration-500 md:flex-row [&[data-highlight]]:ring-2"
	class:min-h-50={reply.body}
	id="p{reply.id}"
	bind:this={ref}
>
	{#if author}
		<div
			class="bg-background flex shrink-0 gap-4 rounded-md p-2 md:block md:w-34 md:space-y-2 md:border-r md:bg-transparent md:pr-4"
		>
			<div class="mx-auto w-24 shrink-0 md:w-[unset]">
				<AspectRatio ratio={1 / 1}>
					<a href="/user/{author.id}">
						<Avatar.Root class="size-full !rounded-md">
							<Avatar.Fallback class="!rounded-md text-2xl uppercase">
								{#if author}
									{author.name.charAt(0)}
								{:else}
									?
								{/if}
							</Avatar.Fallback>
						</Avatar.Root>
					</a>
					{#if isPoster}
						<span class="absolute inset-x-0 -bottom-2 flex items-center justify-center">
							<Tooltip.Provider delayDuration={0} disableHoverableContent>
								<Tooltip.Root>
									<Tooltip.Trigger class="bg-accent rounded-full p-0.5">
										<UserIcon class="size-5" />
									</Tooltip.Trigger>
									<Tooltip.Content>Original Poster</Tooltip.Content>
								</Tooltip.Root>
							</Tooltip.Provider>
						</span>
					{/if}
				</AspectRatio>
			</div>
			{#if author}
				<div class="flex-auto space-y-2 text-xs md:text-center">
					<div>
						<a
							class="text-accent-foreground block text-sm font-semibold break-words hover:underline"
							href="/user/{author.id}"
						>
							{author?.name ?? "Anonymous"}
						</a>
						{#if role}
							<strong
								class="block font-medium"
								class:text-orange-600={role === Constants.User.Role.Sysop ||
									role === Constants.User.Role.Staff ||
									role === Constants.User.Role.Moderator}
								class:text-green-600={role === Constants.User.Role.PowerUploader}
								class:text-emerald-600={role === Constants.User.Role.Contributor}
								class:text-sky-600={role === Constants.User.Role.GroupLeader}
								class:text-cyan-600={role === Constants.User.Role.GroupOfficer}>{Constants.User.RoleText[role]}</strong
							>
						{/if}
					</div>
					{#if author.relationships?.stats}
						<div>
							<div class="flex items-center justify-between">
								<strong class="font-semibold">Messages:</strong>
								<span>{author.relationships.stats.messages.short()}</span>
							</div>
							{#if author.relationships.stats.chapters}
								<div class="flex items-center justify-between">
									<strong class="font-semibold">Uploads:</strong>
									<span>{author.relationships.stats.chapters.short()}</span>
								</div>
							{/if}
							{#if author.relationships.stats.points !== 0}
								<div class="flex items-center justify-between">
									<strong class="font-semibold">Points:</strong>
									<span>{author.relationships.stats.points.short()}</span>
								</div>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	{/if}
	<div class="flex flex-auto flex-col md:w-0">
		<header class="text-muted-foreground flex shrink-0 items-center justify-between border-b pb-1 text-xs font-medium">
			<div>
				<a class="hover:underline" href="/forums/threads/{comments.thread.id}/{reply.id}" rel="nofollow">
					<time datetime={reply.created_at}>
						{#if createdAt.getTime() > Date.now() - 24 * 3600 * 1000}
							{reply.created_at.ago()}
						{:else}
							{createdAt.toLocaleString("en-US", { dateStyle: "full", timeStyle: "short" })}
						{/if}
					</time>
				</a>
			</div>
			<div class="flex items-center gap-2">
				<a
					class="hover:text-foreground font-medium"
					href="/forums/threads/{comments.thread.id}/{reply.id}"
					rel="nofollow"
				>
					#{reply.reply_number}
				</a>
			</div>
		</header>
		<div class="flex flex-auto flex-col space-y-2 py-2" class:pb-0={!reply.body} class:pb-5={reply.body}>
			<div class="typography comment flex-auto" {@attach Typography.exec}>
				{#if reply.body}
					{@html Utils.parseComment(reply.body)}
				{:else}
					<p class="text-muted-foreground text-xs italic">
						{#if reply.state && reply.state in Constants.Thread.ReplyRemovedStateText}
							{Constants.Thread.ReplyRemovedStateText[reply.state]}
						{:else}
							This comment is unavailable
						{/if}
					</p>
				{/if}
			</div>
		</div>
		{#if !isRemoved(reply) || ($Session && Permission.Reply.canDelete())}
			<footer class="text-muted-foreground shrink-0 space-y-1 text-xs">
				{#if !isRemoved(reply) && reply.version > 1 && reply.updated_at != reply.created_at}
					<em class="text-muted-foreground block text-right">
						{#if updatedAt.getTime() > Date.now() - 24 * 3600 * 1000}
							Edited {reply.updated_at.ago()}
						{:else}
							Edited {updatedAt.toLocaleString("en-US", { dateStyle: "full", timeStyle: "short" })}
						{/if}
					</em>
				{/if}
				<div class="flex items-center justify-between gap-2">
					<div class="flex items-center gap-1">
						{#if $Session && !isRemoved(reply)}
							<div class="flex items-center gap-1">
								<button
									class="hover:text-accent-foreground flex size-6 items-center justify-center"
									class:pointer-events-none={$mutex || !isVotable}
									class:text-accent-foreground={isUpvoted}
									onclick={() => onVote(1)}
									disabled={$mutex || !isVotable}
								>
									{#if $mutex && mutexVoteState === 1}
										<Spinner class="size-3.5" />
									{:else}
										<ThumbsUpIcon class="size-3.5 " />
									{/if}
								</button>
								{#if voteScore !== 0}
									<span class="min-w-2 text-center">
										{voteScore}
									</span>
								{/if}
								<button
									class="hover:text-accent-foreground flex size-6 items-center justify-center"
									class:pointer-events-none={$mutex || !isVotable}
									class:text-accent-foreground={isDownvoted}
									onclick={() => onVote(-1)}
									disabled={$mutex || !isVotable}
								>
									{#if $mutex && mutexVoteState === -1}
										<Spinner class="size-3.5" />
									{:else}
										<ThumbsDown class="size-3.5 " />
									{/if}
								</button>
							</div>
							{#if $Session && !comments.thread.locked}
								<button
									class="hover:text-accent-foreground flex items-center gap-1 p-1 font-medium"
									onclick={() => comments.quote(reply)}
								>
									<ReplyIcon class="size-3.5 shrink-0" />
									Reply
								</button>
							{/if}
						{/if}
					</div>
					<div class="flex items-center gap-2">
						{#if !comments.thread.locked}
							{#if isRemoved(reply) && Permission.Reply.canDelete()}
								<ConfirmDialog
									title="Delete Reply"
									description="Are you sure you want to delete this reply?"
									confirmText="Delete"
									variant="destructive"
									onConfirm={e => e.exec(onDelete)}
									disabled={$mutex}
								>
									{#snippet child({ props })}
										<button class="flex items-center gap-2 p-1 font-medium text-red-400 hover:text-red-300" {...props}>
											<TrashIcon class="size-3.5 shrink-2" />
											Hard Delete
										</button>
									{/snippet}
								</ConfirmDialog>
							{:else}
								{#if Permission.Reply.canUpdate(reply)}
									<button
										class="hover:text-accent-foreground flex items-center gap-2 p-1 font-medium"
										onclick={() => comments.edit(reply)}
									>
										<PencilIcon class="size-3.5 shrink-0" />
										Edit
									</button>
								{/if}
								{#if Permission.Reply.canRemove(reply)}
									<ConfirmDialog
										title="Delete Reply"
										description="Are you sure you want to delete this reply?"
										confirmText="Delete"
										variant="destructive"
										onConfirm={e => e.exec(onRemove)}
										disabled={$mutex}
									>
										{#snippet child({ props })}
											<button
												class="flex items-center gap-2 p-1 font-medium text-red-400 hover:text-red-300"
												{...props}
											>
												<TrashIcon class="size-3.5 shrink-2" />
												Delete
											</button>
										{/snippet}
									</ConfirmDialog>
								{/if}
								<button
									class="flex items-center gap-2 p-1 font-medium text-red-400 hover:text-red-300"
									onclick={() => {
										ReportDialog.set(reply, Constants.EntityType.Reply);
										ReportDialog.open();
									}}
								>
									<TriangleAlertIcon class="size-3.5 shrink-2" />
									Report
								</button>
							{/if}
						{/if}
					</div>
				</div>
			</footer>
		{/if}
	</div>
</div>
