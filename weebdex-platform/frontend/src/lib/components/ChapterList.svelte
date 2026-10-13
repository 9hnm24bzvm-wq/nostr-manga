<script lang="ts">
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import * as Collapsible from "$lib/components/ui/collapsible";
	import * as Empty from "$lib/components/ui/empty";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { getManga } from "$lib/Context";
	import { ChevronsUpDownIcon } from "@lucide/svelte";
	import { onMount } from "svelte";
	import ChapterListEntry from "./ChapterListEntry.svelte";

	let {
		feed = $bindable(),
		onSearch
	}: {
		feed?: ChapterFeedResponse;
		onSearch?: () => Promise<void>;
	} = $props();

	const manga = $derived(getManga()());
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	const grouped = $derived(
		feed?.data?.reduce<Map<string, Map<string, Chapter[]>>>((acc, chapter) => {
			const v = chapter.volume ?? "";
			const c = chapter.chapter ?? "";

			if (!acc.has(v)) acc.set(v, new Map());
			if (!acc.get(v)!.has(c)) acc.get(v)!.set(c, []);
			acc.get(v)!.get(c)!.push(chapter);

			return acc;
		}, new Map())
	);

	onMount(async () => {
		const state = page.state as {
			_$ChapterList_feed: typeof feed;
		};
		if (state._$ChapterList_feed) {
			feed = state._$ChapterList_feed;
		} else {
			try {
				await onSearch?.();
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
{:else if isMounted && feed}
	{#if feed.data?.length}
		{#if grouped}
			<div class="space-y-3">
				{#each grouped.entries() as [v, groups]}
					<Collapsible.Root open={true}>
						<Collapsible.Trigger class="flex w-full items-center text-left">
							<strong class="w-0 flex-auto font-semibold">
								{#if v}
									Volume {v}
								{:else}
									No Volume
								{/if}
							</strong>
							<ChevronsUpDownIcon class="shrink-0" size="18" />
						</Collapsible.Trigger>
						<Collapsible.Content class="mt-1 space-y-0.5">
							{#each groups.entries() as [c, chapters]}
								{#if chapters.length === 1}
									<ChapterListEntry class="bg-input/30 hover:bg-input/55 rounded-md" chapter={chapters[0]} />
								{:else}
									<Collapsible.Root class="bg-input/30 overflow-hidden rounded-md" open={true}>
										<Collapsible.Trigger class="hover:bg-input/25 flex w-full items-center px-2.5 py-1.5 text-left">
											<strong class="w-0 flex-auto font-semibold">
												{#if c}
													Chapter {c}
												{:else}
													Oneshot
												{/if}
											</strong>
											<ChevronsUpDownIcon class="shrink-0" size="18" />
										</Collapsible.Trigger>
										<Collapsible.Content class="border-border overflow-auto border-t-2 pl-2.5">
											<div class="divide-y-2 border-l-2">
												{#each chapters as chapter}
													<ChapterListEntry class="hover:bg-input/25" {chapter} depth={1} />
												{/each}
											</div>
										</Collapsible.Content>
									</Collapsible.Root>
								{/if}
							{/each}
						</Collapsible.Content>
					</Collapsible.Root>
				{/each}
			</div>
		{/if}
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No chapters found</Empty.Title>
				{#if manga.relationships.available_languages?.length || manga.relationships.available_groups}
					<Empty.Description>Try adjusting your filters and search again.</Empty.Description>
				{:else}
					<Empty.Description>There are no chapters available for this title yet.</Empty.Description>
				{/if}
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
