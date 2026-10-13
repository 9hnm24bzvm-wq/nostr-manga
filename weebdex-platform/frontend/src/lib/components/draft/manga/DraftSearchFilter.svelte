<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import { Button } from "$lib/components/ui/button";
	import * as Select from "$lib/components/ui/select";
	import { SearchIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";

	let {
		feed = $bindable()
	}: {
		feed: MangaFeedResponse | undefined;
	} = $props();

	const defaultSearch: {
		state: string[];
		sort: Constants.Manga.DraftSortBy;
		order: "asc" | "desc";
	} = {
		state: [],
		sort: Constants.Manga.DraftSortBy.CreatedAt,
		order: "desc"
	};
	let search = $state<typeof defaultSearch>({
		state: page.url.searchParams.getAll("state") ?? [],
		sort: (page.url.searchParams.get("sort") as Constants.Manga.DraftSortBy) ?? Constants.Manga.DraftSortBy.CreatedAt,
		order: (page.url.searchParams.get("order") as "asc" | "desc") ?? "desc"
	});

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();

		const q = new URLSearchParams();
		for (const [key, value] of Object.entries(search)) {
			if (value === undefined || value === null || (typeof value === "string" && !value.length)) {
				continue;
			}

			if (Array.isArray(value)) {
				value.forEach(v => q.append(key, v));
			} else {
				q.set(key, value);
			}
		}

		if (search.sort === Constants.Manga.DraftSortBy.CreatedAt) q.delete("sort");
		if (search.order === "desc") q.delete("order");
		q.delete("page");

		const target = window.location.pathname + q.toString();
		if (target !== window.location.pathname + window.location.search) {
			feed = undefined;
			goto(target);
		}
	};

	const onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		search = structuredClone(defaultSearch);
		onSubmit();
	};
</script>

<form class="space-y-1" onsubmit={onSubmit} onreset={onReset}>
	<div class="flex flex-wrap items-center gap-1">
		<Select.Root type="multiple" bind:value={search.state}>
			<Select.Trigger class="flex w-0 min-w-45 flex-auto border-none">
				<span>
					State:
					{#if search.state?.length}
						{search.state.map(k => Constants.StateText[k]).join(", ")}
					{:else}
						Any
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries(Constants.StateText) as [key, value]}
					<Select.Item value={key}>{value}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" bind:value={search.sort}>
			<Select.Trigger class="text-muted-foreground flex w-0 min-w-45 flex-auto border-none">
				<span>
					Sort:
					{#if search.sort}
						{Constants.Manga.DraftSortByText[search.sort]}
					{:else}
						Any
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries(Constants.Manga.DraftSortByText) as [key, value]}
					<Select.Item value={key}>{value}</Select.Item>
				{/each}
			</Select.Content><Select.Root type="single" bind:value={search.order}>
				<Select.Trigger class="text-muted-foreground flex w-0 min-w-60 flex-auto border-none">
					<span class="truncate">
						Order:
						{#if search.order === "asc"}
							Ascending
						{:else}
							Descending
						{/if}
					</span>
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="asc">Ascending</Select.Item>
					<Select.Item value="desc">Descending</Select.Item>
				</Select.Content>
			</Select.Root>
		</Select.Root>
	</div>
	<div class="mt-5 flex items-center gap-2">
		<div class="w-0 flex-auto">
			<span class="text-muted-foreground">
				{#if feed?.total}
					Found {feed?.total} results
				{/if}
			</span>
		</div>
		<div class="flex shrink-0 items-center gap-1">
			<Button
				class="!text-destructive border-none"
				variant="outline"
				disabled={equal(search, defaultSearch)}
				type="reset"
			>
				Reset
			</Button>
			<Button variant="default" type="submit">
				<SearchIcon />
				Search
			</Button>
		</div>
	</div>
</form>
