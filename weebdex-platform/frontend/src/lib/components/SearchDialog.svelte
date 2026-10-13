<script lang="ts" module>
	let query = $state<string>("");
	let mangaFeed = $state<MangaFeedResponse>();
	let authorFeed = $state<TagFeedResponse>();
	let groupFeed = $state<ScanlationGroupFeedResponse>();
</script>

<script lang="ts">
	import { API, Mutex } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { isShowSearchDialog, prefs } from "$lib/stores";
	import { ArrowRightIcon, SearchIcon, X } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import SearchDialogMangaEntry from "./SearchDialogMangaEntry.svelte";
	import { buttonVariants } from "./ui/button";
	import Button from "./ui/button/button.svelte";
	import ScrollArea from "./ui/scroll-area/scroll-area.svelte";
	import Spinner from "./ui/spinner/spinner.svelte";

	const mutex = new Mutex();
	let timeout = $state(0);

	const searchManga = async (title: string) => {
		try {
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);

			q.set("title", title);
			q.set("sort", "relevance");
			q.set("limit", "5");

			const res = await API.Manga.Search(q);
			if (res?.status === StatusCodes.OK && res.body) {
				mangaFeed = res.body;
			}
		} catch (e) {
			console.error(e);
		}
	};

	const searchAuthors = async (name: string) => {
		try {
			const q = new URLSearchParams();
			q.set("name", name);
			q.set("limit", "5");

			const res = await API.Author.Search(q);
			if (res?.status === StatusCodes.OK && res.body) {
				authorFeed = res.body;
			}
		} catch (e) {
			console.error(e);
		}
	};

	const searchGroups = async (name: string) => {
		try {
			const q = new URLSearchParams();
			q.set("name", name);
			q.set("limit", "5");

			const res = await API.Group.Search(q);
			if (res?.status === StatusCodes.OK && res.body) {
				groupFeed = res.body;
			}
		} catch (e) {
			console.error(e);
		}
	};

	const onSearch = async () => {
		if (!query.length) {
			return;
		}

		await Promise.all([searchManga(query), searchAuthors(query), searchGroups(query)]);
	};

	const onInput = () => {
		window.clearTimeout(timeout);
		timeout = window.setTimeout(
			() =>
				mutex.exec(() => {
					mangaFeed = undefined;
					authorFeed = undefined;
					groupFeed = undefined;
					return onSearch();
				}),
			500
		);
	};

	const onSubmit = () => {
		window.clearTimeout(timeout);
		mutex.exec(() => {
			mangaFeed = undefined;
			authorFeed = undefined;
			groupFeed = undefined;
			return onSearch();
		});
	};
</script>

<div class="flex shrink-0 items-center gap-1">
	<InputGroup.Root class="!border-border rounded-none !border-x-0 !border-t-0 py-5 !ring-0 !ring-transparent">
		<InputGroup.Input
			class="w-0 flex-auto truncate"
			bind:value={query}
			placeholder="Search titles, authors, or groups..."
			autocomplete="off"
			oninput={onInput}
			onsubmit={onSubmit}
		/>
		<InputGroup.Addon>
			{#if $mutex}
				<Spinner />
			{:else}
				<SearchIcon />
			{/if}
		</InputGroup.Addon>
		<InputGroup.Addon align="inline-end">
			<Button class="-mr-0.75" type="button" variant="ghost" size="icon" onclick={() => ($isShowSearchDialog = false)}>
				<X class="size-5" />
			</Button>
		</InputGroup.Addon>
	</InputGroup.Root>
</div>
<div class="flex h-full flex-auto overflow-hidden p-2">
	<ScrollArea class="w-full flex-auto" scrollbarYClasses="hidden">
		{#if $mutex}
			<Empty.Root>
				<Empty.Header>
					<Empty.Description>Searching...</Empty.Description>
				</Empty.Header>
			</Empty.Root>
		{:else if mangaFeed || authorFeed || groupFeed}
			{#if mangaFeed?.total || authorFeed?.total || groupFeed?.total}
				<div class="space-y-3">
					{#if mangaFeed?.total}
						<section>
							<header class="mb-1 flex items-center">
								<h2 class="w-0 flex-auto font-medium">Titles</h2>
								{#if mangaFeed.total > mangaFeed.limit}
									<div class="shrink-0">
										<a
											class={buttonVariants({
												variant: "ghost",
												class: "hover:!bg-input/50 size-6"
											})}
											href={`/search${new URLSearchParams({ title: query, sort: "relevance" }).toString()}`}
										>
											<ArrowRightIcon class="size-5" />
										</a>
									</div>
								{/if}
							</header>
							<div class="space-y-1">
								{#each mangaFeed.data as manga}
									<SearchDialogMangaEntry {manga} />
								{/each}
							</div>
						</section>
					{/if}

					{#if groupFeed?.total}
						<section>
							<header class="mb-1 flex items-center">
								<h2 class="w-0 flex-auto font-medium">Scanlation Groups</h2>
								{#if groupFeed.total > groupFeed.limit}
									<div class="shrink-0">
										<a
											class={buttonVariants({
												variant: "ghost",
												class: "hover:!bg-input/50 size-6"
											})}
											href={`/groups${new URLSearchParams({ name: query }).toString()}`}
										>
											<ArrowRightIcon class="size-5" />
										</a>
									</div>
								{/if}
							</header>
							<div class="space-y-1">
								{#each groupFeed.data as group}
									<article>
										<a
											class={buttonVariants({
												variant: "outline",
												class: "w-full justify-start border-none"
											})}
											href="/group/{group.id}"
											title={group.name}
										>
											<span class="truncate">{group.name}</span>
										</a>
									</article>
								{/each}
							</div>
						</section>
					{/if}

					{#if authorFeed?.total}
						<section>
							<header class="mb-1 flex items-center">
								<h2 class="w-0 flex-auto font-medium">Authors</h2>
							</header>
							<div class="space-y-1">
								{#each authorFeed.data as author}
									<article>
										<a
											class={buttonVariants({
												variant: "outline",
												class: "w-full justify-start border-none"
											})}
											href="/author/{author.id}"
											title={author.name}
										>
											<span class="truncate">{author.name}</span>
										</a>
									</article>
								{/each}
							</div>
						</section>
					{/if}
				</div>
			{:else}
				<Empty.Root>
					<Empty.Header>
						<Empty.Description>No results found</Empty.Description>
					</Empty.Header>
				</Empty.Root>
			{/if}
		{:else}
			<Empty.Root>
				<Empty.Header>
					<Empty.Description>Type to search…</Empty.Description>
				</Empty.Header>
			</Empty.Root>
		{/if}
	</ScrollArea>
</div>
