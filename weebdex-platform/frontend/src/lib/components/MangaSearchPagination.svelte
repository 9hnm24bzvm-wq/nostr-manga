<script lang="ts">
	import { goto } from "$app/navigation";
	import Button from "$lib/components/ui/button/button.svelte";
	import * as Pagination from "$lib/components/ui/pagination";
	import { EllipsisIcon } from "@lucide/svelte";

	const { feed }: { feed: MangaFeedResponse | MangaFollowFeedResponse } = $props();
	const isFollows = window.location.pathname.startsWith("/library");
	const pages = $derived(Math.ceil((isFollows ? feed.total : Math.min(feed.total, 10000)) / feed.limit));

	const makePath = (page: number) => {
		page = Math.min(Math.max(page, 1), pages);
		const searchParams = new URLSearchParams(window.location.search);
		if (page > 1) searchParams.set("page", page.toString());
		else searchParams.delete("page");
		return window.location.pathname + searchParams.toString();
	};
</script>

<Pagination.Root
	count={isFollows ? feed.total : Math.min(feed.total, 10000 - feed.limit)}
	perPage={feed.limit}
	page={feed.page}
>
	{#snippet children({ pages: items, currentPage })}
		<Pagination.Content>
			{#each items as page (page.key)}
				{#if page.type === "ellipsis"}
					<Pagination.Item>
						<Button
							type="button"
							variant="ghost"
							size="icon"
							onclick={() => {
								const value = (prompt("Enter page number") || "").trim();
								if (!value) {
									return;
								}

								const page = Math.min(Math.max(Number(value), 1), pages);
								if (page !== feed.page) {
									goto(makePath(page));
								}
							}}
						>
							<EllipsisIcon class="size-4" />
							<span class="sr-only">Jump to page</span>
						</Button>
					</Pagination.Item>
				{:else}
					<Pagination.Item>
						<Pagination.Link class="border-none" {page} isActive={currentPage === page.value}>
							{#snippet child({ props })}
								<a href={makePath(page.value)} {...props}>
									{page.value}
								</a>
							{/snippet}
						</Pagination.Link>
					</Pagination.Item>
				{/if}
			{/each}
		</Pagination.Content>
	{/snippet}
</Pagination.Root>
