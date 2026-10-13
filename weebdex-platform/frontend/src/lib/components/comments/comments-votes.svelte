<script lang="ts" module>
	const mutex = new Mutex();
</script>

<script lang="ts">
	import { API, Constants, Mutex } from "$lib";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import Button from "$lib/components/ui/button/button.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { ThumbsDown, ThumbsUp } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import { useComments } from "./context.svelte";

	const comments = useComments();

	onMount(() =>
		mutex.exec(async () => {
			if ((await Session.isValid()) && comments.voteState === undefined) {
				try {
					const res = await API.Thread.GetThreadVote(comments.thread.id);
					if (res?.status === StatusCodes.OK && res.body) {
						comments.voteState = res.body.vote;
						if (comments.stats) {
							if (comments.stats.up === 0 && comments.voteState === 1) {
								comments.stats.up++;
							} else if (comments.stats.down === 0 && comments.voteState === -1) {
								comments.stats.down++;
							}
						}
					} else if (res?.status === StatusCodes.NO_CONTENT) {
						comments.voteState = 0;
					}
				} catch (e) {}
			}
		})
	);

	let mutexVoteState = $state<-1 | 1>();
	const onVote = (value: -1 | 1) => {
		if (comments.voteState === value) {
			return onDeleteVote(value);
		}
		return mutex.exec(async () => {
			if (!comments.thread || !comments.stats) {
				return;
			}

			if (!(await Session.isValid())) {
				toast.error("Please sign in to perform this action");
				return;
			}

			mutexVoteState = value;
			try {
				const res = await API.Thread.VoteThread(comments.thread.id, value);
				if (res?.status === StatusCodes.NO_CONTENT) {
					if (comments.voteState === -1) {
						comments.stats.down--;
					} else if (comments.voteState === 1) {
						comments.stats.up--;
					}
					comments.stats[value === 1 ? "up" : "down"]++;
					comments.voteState = value;
					comments.stats = comments.stats;
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
			if (!comments.stats) {
				return;
			}

			mutexVoteState = value;
			try {
				const res = await API.Thread.UnvoteThread(comments.thread.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					comments.stats[value === 1 ? "up" : "down"]--;
					comments.voteState = undefined;
					comments.stats = comments.stats;
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
</script>

<ButtonGroup.Root>
	<Button
		class={cn("text-muted-foreground border-0", {
			"!border-accent text-accent-foreground border-b-2": comments.voteState === 1
		})}
		variant="outline"
		onclick={e => {
			e.stopPropagation();
			e.stopImmediatePropagation();
			onVote(1);
		}}
	>
		{#if $mutex && mutexVoteState === 1}
			<Spinner />
		{:else}
			<ThumbsUp />
		{/if}
		{#if comments.stats?.up}
			{comments.stats?.up.toLocaleString()}
		{/if}
	</Button>
	<ButtonGroup.Separator />
	<Button
		class={cn("text-muted-foreground border-0", {
			"!border-accent text-accent-foreground border-b-2": comments.voteState === -1
		})}
		variant="outline"
		size="icon"
		onclick={e => {
			e.stopPropagation();
			e.stopImmediatePropagation();
			onVote(-1);
		}}
	>
		{#if $mutex && mutexVoteState === -1}
			<Spinner />
		{:else}
			<ThumbsDown />
		{/if}
	</Button>
</ButtonGroup.Root>
