<script lang="ts" module>
	let lastThreadId = $state("");
</script>

<script lang="ts">
	import { page } from "$app/state";
	import { API } from "$lib";
	import Create from "$lib/components/comments/comments-create.svelte";
	import EditDialog from "$lib/components/comments/comments-edit-dialog.svelte";
	import Feed from "$lib/components/comments/comments-feed.svelte";
	import Filter from "$lib/components/comments/comments-filter.svelte";
	import GenericPagination from "$lib/components/GenericPagination.svelte";
	import * as Key from "$lib/components/ui/key";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { LockIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount, tick } from "svelte";
	import { useComments } from "./context.svelte";

	const { class: className }: { class?: string } = $props();
	const comments = useComments();

	onMount(async () => {
		if (lastThreadId === comments.thread.id) {
			return;
		}

		try {
			const res = await API.Statistics.Thread(comments.thread.id);
			if (res?.status === StatusCodes.OK && res.body) {
				comments.stats = res.body;
				lastThreadId = comments.thread.id;
			}
		} catch (e) {
			console.error(e);
		}
	});
</script>

{#snippet create()}
	{#if $Session}
		<Create />
	{:else}
		<div class="rounded-md border border-dashed border-slate-500/50 bg-slate-500/15 p-3 text-center text-slate-300">
			<strong class="text-lg font-medium">Have something to say?</strong>
			<p>
				<a
					class="hover:text-foreground underline"
					href="/auth/login{new URLSearchParams({
						redirect: page.url.href
					})}">Login</a
				>
				or
				<a
					class="hover:text-foreground underline"
					href="/auth/register{new URLSearchParams({
						redirect: page.url.href
					})}">Register</a
				> to join the conversation.
			</p>
		</div>
	{/if}
{/snippet}

<Key.Provider>
	<section
		class={cn("space-y-5", className)}
		data-thread-id={comments.thread.id}
		id="comments"
		bind:this={comments.ref}
	>
		<header class="space-y-2">
			{#if comments.thread.locked}
				<div
					class="rounded-md border border-dashed border-yellow-500/50 bg-yellow-500/15 p-3 text-center text-yellow-300"
				>
					<LockIcon class="inline size-4 align-middle" />
					<strong class="align-middle font-medium">
						This thread is locked and can no longer receive new replies.
					</strong>
				</div>
			{/if}

			<Filter />
			{#if page.url.searchParams.get("sort") !== "oldest"}
				{@render create()}
			{/if}
		</header>
		<div class="space-y-2">
			{#key page.url.search}
				<Feed />
			{/key}
		</div>
		<footer class="space-y-2">
			{#if comments.feed && comments.feed.total > comments.feed.limit}
				<GenericPagination
					feed={comments.feed}
					onPaginate={async () => {
						await tick();
						window.setTimeout(() => {
							if (comments.ref) {
								window.scroll({
									top: (comments.ref?.getBoundingClientRect().top ?? 0) - 100
								});
							}
						}, 10);
					}}
				/>
			{/if}
			{#if page.url.searchParams.get("sort") === "oldest"}
				{@render create()}
			{/if}
		</footer>
	</section>
	<EditDialog />
</Key.Provider>
