<script lang="ts">
	import { Constants } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import * as Combobox from "$lib/components/ui/combobox";
	import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from "@lucide/svelte";
	import { useReader } from "./context.svelte";

	const reader = $derived(useReader());
	const readerPrefs = $derived(reader.prefs);
	const firstChapter = $derived(reader.firstChapter);
	const lastChapter = $derived(reader.lastChapter);
	const nextChapter = $derived(reader.nextChapter);
	const prevChapter = $derived(reader.prevChapter);
	const chapterIndex = $derived(reader.chapterIndex);
	const chapter = $derived(reader.chapter);
	const aggregated = $derived(reader.aggregated);
</script>

<ButtonGroup.Root class="bg-input/30 overflow-hidden rounded-md">
	<a
		class={buttonVariants({ class: "border-none", variant: "outline", size: "icon" })}
		class:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !lastChapter : !firstChapter}
		class:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !lastChapter : !firstChapter}
		href={$readerPrefs.direction === Constants.Reader.Direction.RTL
			? lastChapter
				? `/chapter/${lastChapter.id}/1`
				: undefined
			: firstChapter
				? `/chapter/${firstChapter.id}/1`
				: undefined}
	>
		<ChevronsLeft />
	</a>
	<ButtonGroup.Separator />
	<a
		class={buttonVariants({ class: "border-none", variant: "outline", size: "icon" })}
		class:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !nextChapter : !prevChapter}
		class:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !nextChapter : !prevChapter}
		href={$readerPrefs.direction === Constants.Reader.Direction.RTL
			? nextChapter
				? `/chapter/${nextChapter.id}/1`
				: undefined
			: prevChapter
				? `/chapter/${prevChapter.id}/1`
				: undefined}
	>
		<ChevronLeft />
	</a>
</ButtonGroup.Root>
<Combobox.Root
	type="single"
	value={chapterIndex?.toString()}
	onValueChange={value => reader.changeChapter(Number(value))}
	disabled={!aggregated?.chapters.length}
>
	<Combobox.Trigger class={buttonVariants({ class: "w-0 flex-auto border-none", variant: "outline" })}>
		<span class="text-foreground w-0 flex-auto truncate">
			{#if chapter.chapter}
				{#if chapter.volume}
					Vol. {chapter.volume}
				{/if}
				Ch. {chapter.chapter}
			{:else}
				Oneshot
			{/if}
		</span>
	</Combobox.Trigger>
	{#if aggregated}
		<Combobox.Content>
			{#each aggregated.chapters as entry, i}
				<Combobox.Item value={i.toString()}>
					<span class="w-0 flex-auto truncate" class:underline={chapterIndex === i}>
						{#if entry.chapter}
							{#if entry.volume}
								Vol. {entry.volume}
							{/if}
							Ch. {entry.chapter}
						{:else}
							Oneshot
						{/if}
					</span>
				</Combobox.Item>
			{/each}
		</Combobox.Content>
	{/if}
</Combobox.Root>
<ButtonGroup.Root class="bg-input/30 overflow-hidden rounded-md">
	<a
		class={buttonVariants({ class: "border-none", variant: "outline", size: "icon" })}
		class:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !prevChapter : !nextChapter}
		class:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !prevChapter : !nextChapter}
		href={$readerPrefs.direction === Constants.Reader.Direction.RTL
			? prevChapter
				? `/chapter/${prevChapter.id}/1`
				: undefined
			: nextChapter
				? `/chapter/${nextChapter.id}/1`
				: undefined}
	>
		<ChevronRight />
	</a>
	<ButtonGroup.Separator />
	<a
		class={buttonVariants({ class: "border-none", variant: "outline", size: "icon" })}
		class:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !firstChapter : !lastChapter}
		class:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !firstChapter : !lastChapter}
		href={$readerPrefs.direction === Constants.Reader.Direction.RTL
			? firstChapter
				? `/chapter/${firstChapter.id}/1`
				: undefined
			: lastChapter
				? `/chapter/${lastChapter.id}/1`
				: undefined}
	>
		<ChevronsRight />
	</a>
</ButtonGroup.Root>
