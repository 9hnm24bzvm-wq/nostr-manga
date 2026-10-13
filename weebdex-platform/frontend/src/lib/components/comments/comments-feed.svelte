<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { Skeleton } from "../ui/skeleton";
	import Entry from "./comments-entry.svelte";
	import { useComments } from "./context.svelte";

	const comments = useComments();
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		if ((page.state as any)._$Replies_feed) {
			comments.feed = (page.state as any)._$Replies_feed as typeof comments.feed;
		} else {
			try {
				const q = new URLSearchParams();
				if (page.url.searchParams.has("page")) {
					const p = page.url.searchParams.get("page");
					if (Number(p) > 1) q.set("page", p!);
				}
				if (page.url.searchParams.has("sort")) {
					const sort = page.url.searchParams.get("sort");
					if (sort === "oldest") q.set("order", "asc");
				}

				const res = await API.Thread.Replies(comments.thread.id, q);
				if (res?.status === StatusCodes.OK && res.body) {
					res.body.data ??= [];
					comments.feed = res.body;
					replaceState(window.location.href, {
						...page.state,
						_$Replies_feed: $state.snapshot(comments.feed)
					});
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) error = e;
				else {
					console.error(e);
					error = Constants.Error.Unknown;
				}
			}
		}
		isMounted = true;
	});
</script>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted && comments.feed}
	{#if comments.feed.data?.length}
		<div class="space-y-2">
			{#each comments.feed.data as reply, i (reply)}
				<Entry bind:reply={comments.feed!.data![i]} />
			{/each}
		</div>
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No results found</Empty.Title>
				<Empty.Description>Be the first to comment and share your thoughts!</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<div class="space-y-2">
		{#each Array.from({ length: 20 }) as _}
			<Skeleton class="bg-card h-55" />
		{/each}
	</div>
{/if}
