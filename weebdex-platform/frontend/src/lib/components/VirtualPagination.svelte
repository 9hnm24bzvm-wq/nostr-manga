<script lang="ts">
	import Button from "$lib/components/ui/button/button.svelte";
	import * as Pagination from "$lib/components/ui/pagination";
	import { EllipsisIcon } from "@lucide/svelte";

	const {
		feed,
		onPagination
	}: {
		feed: FeedResponse<any>;
		onPagination: (page: number) => void;
	} = $props();
	const pages = $derived(Math.ceil(feed.total / feed.limit));
</script>

<Pagination.Root count={feed.total} perPage={feed.limit} page={feed.page}>
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
									onPagination(page);
								}
							}}
						>
							<EllipsisIcon class="size-4" />
							<span class="sr-only">Jump to page</span>
						</Button>
					</Pagination.Item>
				{:else}
					<Pagination.Item>
						<Pagination.Link
							class="border-none"
							{page}
							isActive={currentPage === page.value}
							onclick={() => onPagination(page.value)}
						>
							{page.value}
						</Pagination.Link>
					</Pagination.Item>
				{/if}
			{/each}
		</Pagination.Content>
	{/snippet}
</Pagination.Root>
