<script lang="ts">
	import { Constants, Utils } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import * as Combobox from "$lib/components/ui/combobox";
	import { ChevronsLeft, ChevronsRight, HomeIcon, SettingsIcon, Undo2 } from "@lucide/svelte";
	import Headroom from "headroom.js";
	import { onDestroy } from "svelte";
	import Button from "../ui/button/button.svelte";
	import { useSidebar } from "../ui/sidebar";
	import { useReader } from "./context.svelte";

	const {
		sticky
	}: {
		sticky?: boolean;
	} = $props();

	const reader = useReader();
	const readerPrefs = reader.prefs;
	const sidebar = useSidebar();
	let ref = $state<HTMLElement>();

	$effect(() => {
		if (sticky && ref && !reader.headroom && $readerPrefs.layout === Constants.Reader.Layout.Scroll) {
			reader.headroom = new Headroom(ref, {
				offset: ref.clientHeight
			});
			reader.headroom.init();
		}
	});

	onDestroy(() => {
		try {
			reader.headroom?.destroy();
		} catch (e) {
			console.error(e);
		} finally {
			reader.headroom = undefined;
		}
	});
</script>

<nav
	class="top-0 z-[100] w-full p-2 transition"
	class:sticky={sticky && $readerPrefs.layout === Constants.Reader.Layout.Scroll}
	bind:this={ref}
>
	<div class="bg-background m-auto flex max-w-200 items-center overflow-hidden rounded-md">
		<div class="flex shrink-0 items-center">
			<ButtonGroup.Root>
				<a class={buttonVariants({ class: "border-none", variant: "outline", size: "icon-lg" })} href="/">
					<HomeIcon />
				</a>
				<a
					class={buttonVariants({ class: "border-none", variant: "outline", size: "icon-lg" })}
					href="/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(
						reader.chapter.relationships.manga.title
					)}"
				>
					<Undo2 class="size-5" />
				</a>
			</ButtonGroup.Root>
		</div>
		<div class="flex w-0 flex-auto items-center">
			<ButtonGroup.Root>
				<Button
					class="border-none"
					variant="outline"
					size="icon-lg"
					onclick={() =>
						reader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.pages.length - 1 : 0)}
					disabled={!reader.pages.length}
				>
					<ChevronsLeft class="size-5" />
				</Button>
			</ButtonGroup.Root>
			<Combobox.Root
				type="single"
				value={reader.chapterIndex?.toString()}
				onValueChange={value => reader.changeChapter(Number(value))}
				disabled={!reader.aggregated?.chapters.length}
			>
				<Combobox.Trigger
					class={buttonVariants({ class: "!h-10 w-0 flex-auto rounded-none border-none", variant: "outline" })}
				>
					<span class="text-foreground w-0 flex-auto truncate">
						{#if reader.chapter.chapter}
							{#if reader.chapter.volume}
								Vol. {reader.chapter.volume}
							{/if}
							Ch. {reader.chapter.chapter}
						{:else}
							Oneshot
						{/if}
					</span>
				</Combobox.Trigger>
				{#if reader.aggregated}
					<Combobox.Content>
						{#each reader.aggregated.chapters as entry, i}
							<Combobox.Item value={i.toString()}>
								<span class="w-0 flex-auto truncate" class:underline={reader.chapterIndex === i}>
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
			<ButtonGroup.Root>
				<Button
					class="border-none"
					variant="outline"
					size="icon-lg"
					onclick={() =>
						reader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? 0 : reader.pages.length - 1)}
					disabled={!reader.pages.length}
				>
					<ChevronsRight class="size-5" />
				</Button>
			</ButtonGroup.Root>
		</div>
		<div class="flex shrink-0 items-center">
			<ButtonGroup.Root>
				<Button class="border-none" variant="outline" size="icon-lg" onclick={sidebar.toggle}>
					<SettingsIcon class="size-5" />
				</Button>
			</ButtonGroup.Root>
		</div>
	</div>
</nav>

<style lang="postcss">
	:global(.headroom.headroom--pinned) {
		transform: translateY(0%);
	}

	:global(.headroom.headroom--unpinned) {
		transform: translateY(-100%);
	}
</style>
