<script lang="ts">
	import { API, Constants } from "$lib";
	import type { SearchMangaRequest } from "$lib/API/Manga";
	import MangaSearch from "$lib/components/MangaSearch.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { Label } from "$lib/components/ui/label";
	import VirtualPagination from "$lib/components/VirtualPagination.svelte";
	import { prefs } from "$lib/stores";
	import { SearchIcon, XIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	let {
		relations = $bindable()
	}: {
		relations: RelationDraft[];
	} = $props();

	let closeRef = $state<HTMLButtonElement>(null!);
	let search = $state<SearchMangaRequest>({
		limit: 28,
		sort: Constants.Manga.SortBy.Relevance,
		order: "desc"
	});
	let ignoreFilters = $state(false);

	let key = $state(0);
	let page = $state(1);

	let feed = $state<MangaFeedResponse>();
	const onSearch = async () => {
		const q = new URLSearchParams();
		if (!ignoreFilters) {
			prefs.applyMangaPreferences(q);
		}

		if (search.title) q.set("title", search.title);
		if (search.sort) q.set("sort", search.sort);
		if (search.order) q.set("order", search.order);
		if (page && page > 1) q.set("page", page.toString());
		q.set("limit", "28");

		const res = await API.Manga.Search(q);
		if (res?.status === StatusCodes.OK && res.body) {
			feed = res.body;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		page = 1;
		key++;
	};

	const onPagination = (n: number) => {
		if (feed) feed.data = undefined;
		page = n;
		key++;
	};
</script>

<section class="space-y-5">
	<header>
		<Dialog.Close class="hidden" bind:ref={closeRef} />
		<form onsubmit={onSubmit}>
			<InputGroup.Root class="w-full border-none py-5 !ring-0 !ring-transparent">
				<InputGroup.Input class="truncate" bind:value={search.title} placeholder="Search..." autocomplete="off" />
				<InputGroup.Addon>
					<SearchIcon />
				</InputGroup.Addon>
				{#if feed?.total}
					<InputGroup.Addon align="inline-end">{feed.total.short()}</InputGroup.Addon>
				{/if}
				<InputGroup.Addon align="inline-end">
					<Dialog.Close
						class={buttonVariants({
							class: "-mr-0.75",
							variant: "ghost",
							size: "icon"
						})}
						type="button"
					>
						<XIcon class="size-6" />
					</Dialog.Close>
				</InputGroup.Addon>
			</InputGroup.Root>
			<div class="mt-3 flex items-start">
				<Checkbox id="ignoreFilters" bind:checked={ignoreFilters} />
				<Label class="cursor-pointer pl-2" for="ignoreFilters">Ignore Filters</Label>
			</div>
			<div class="mt-5 flex items-center gap-2">
				<div class="w-0 flex-auto"></div>
				<div class="flex shrink-0 items-center gap-1">
					<Button variant="default" type="submit">
						<SearchIcon />
						Search
					</Button>
				</div>
			</div>
		</form>
	</header>
	{#key key}
		<MangaSearch
			bind:feed
			isRelationSearch
			{onSearch}
			onClick={manga => {
				const idx = relations.findIndex(r => r.id === manga.id);
				if (idx >= 0) {
					if (relations[idx].deleted) {
						relations[idx].deleted = false;
					} else {
						toast.error("Title is already in the relations list.");
					}
				} else {
					closeRef?.click();
					relations.push({
						...manga,
						type: Constants.Manga.RelationType.MainStory,
						new: true
					});
				}
			}}
		/>
	{/key}
	{#if feed && feed.total > feed.limit}
		<footer>
			<VirtualPagination {feed} {onPagination} />
		</footer>
	{/if}
</section>
