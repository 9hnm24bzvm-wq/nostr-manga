<script lang="ts">
	import { Constants, Mutex } from "$lib";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as Empty from "$lib/components/ui/empty";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { SearchIcon, X } from "@lucide/svelte";
	import type { Snippet } from "svelte";
	import { toast } from "svelte-sonner";
	import Button from "./ui/button/button.svelte";
	import ScrollArea from "./ui/scroll-area/scroll-area.svelte";
	import Spinner from "./ui/spinner/spinner.svelte";

	const {
		onSearch,
		children
	}: {
		onSearch: (query: string) => Promise<FeedResponse<any>>;
		children: Snippet<[{ data: any }]>;
	} = $props();
	const mutex = new Mutex();

	let feed = $state<FeedResponse<any>>();
	let query = $state<string>("");
	let timeout = $state(0);

	const search = async () => {
		feed = undefined;
		if (query.length) {
			try {
				feed = await onSearch(query);
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		}
	};

	const onInput = () => {
		window.clearTimeout(timeout);
		timeout = window.setTimeout(() => mutex.exec(search), 500);
	};

	const onSubmit = () => {
		window.clearTimeout(timeout);
		mutex.exec(search);
	};
</script>

<div class="flex shrink-0 items-center gap-1">
	<InputGroup.Root class="!border-border rounded-none !border-x-0 !border-t-0 py-5 !ring-0 !ring-transparent">
		<InputGroup.Input
			class="w-0 flex-auto truncate"
			bind:value={query}
			placeholder="Search..."
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
			<Dialog.Close>
				{#snippet child({ props })}
					<Button class="-mr-0.75" type="button" variant="ghost" size="icon" {...props}>
						<X class="size-5" />
					</Button>
				{/snippet}
			</Dialog.Close>
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
		{:else if feed}
			{#if feed?.total}
				{@render children({ data: feed.data })}
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
