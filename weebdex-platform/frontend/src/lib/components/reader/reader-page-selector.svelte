<script lang="ts">
	import { Constants } from "$lib";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import * as Combobox from "$lib/components/ui/combobox";
	import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from "@lucide/svelte";
	import { useReader } from "./context.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;
</script>

<ButtonGroup.Root class="bg-input/30 overflow-hidden rounded-md">
	<Button
		class="border-none"
		variant="outline"
		size="icon"
		onclick={() =>
			reader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.pages.length - 1 : 0)}
		disabled={!reader.pages.length}
	>
		<ChevronsLeft />
	</Button>
	<ButtonGroup.Separator />
	<Button
		class="border-none"
		variant="outline"
		size="icon"
		onclick={() =>
			$readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.gotoNextPage() : reader.gotoPreviousPage()}
		disabled={!reader.pages.length}
	>
		<ChevronLeft />
	</Button>
</ButtonGroup.Root>
<Combobox.Root
	type="single"
	value={$readerPrefs.double
		? reader.spreads.indexOf(reader.currentSpread).toString()
		: (reader.currentPage?.index.toString() ?? "")}
	onValueChange={value => reader.changePage(Number(value))}
	disabled={!reader.pages.length}
>
	<Combobox.Trigger class={buttonVariants({ class: "w-0 flex-auto border-none", variant: "outline" })}>
		<span class="text-foreground w-0 flex-auto truncate">
			{#if reader.chapter.data?.length}
				{#if $readerPrefs.double && reader.currentSpread.length > 1}
					{#if $readerPrefs.direction === Constants.Reader.Direction.RTL}
						{reader.currentSpread[1].index + 1}-{reader.currentSpread[0].index + 1}
					{:else}
						{reader.currentSpread[0].index + 1}-{reader.currentSpread[1].index + 1}
					{/if}
				{:else if reader.currentPage}
					{reader.currentPage.index + 1}
				{:else}
					?
				{/if}
				/ {reader.chapter.data?.length}
			{:else}
				?
			{/if}
		</span>
	</Combobox.Trigger>
	{#if reader.pages.length}
		<Combobox.Content>
			{#if $readerPrefs.double}
				{#each reader.spreads as spread, i}
					<Combobox.Item value={i.toString()}>
						<span class="w-0 flex-auto truncate" class:underline={reader.chapterIndex === i}>
							{#if spread.length > 1}
								{#if $readerPrefs.direction === Constants.Reader.Direction.RTL}
									{spread[1].index + 1}-{spread[0].index + 1}
								{:else}
									{spread[0].index + 1}-{spread[1].index + 1}
								{/if}
							{:else}
								{spread[0].index + 1}
							{/if}
						</span>
					</Combobox.Item>
				{/each}
			{:else}
				{#each reader.pages as page}
					<Combobox.Item value={page.index.toString()}>
						<span class="w-0 flex-auto truncate" class:underline={reader.chapterIndex === page.index}>
							{page.index + 1}
						</span>
					</Combobox.Item>
				{/each}
			{/if}
		</Combobox.Content>
	{/if}
</Combobox.Root>
<ButtonGroup.Root class="bg-input/30 overflow-hidden rounded-md">
	<Button
		class="border-none"
		variant="outline"
		size="icon"
		onclick={() =>
			$readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.gotoPreviousPage() : reader.gotoNextPage()}
		disabled={!reader.pages.length}
	>
		<ChevronRight />
	</Button>
	<ButtonGroup.Separator />
	<Button
		class="border-none"
		variant="outline"
		size="icon"
		onclick={() =>
			reader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? 0 : reader.pages.length - 1)}
		disabled={!reader.pages.length}
	>
		<ChevronsRight />
	</Button>
</ButtonGroup.Root>
