<script lang="ts">
	import { Constants } from "$lib";
	import { Button } from "$lib/components/ui/button";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import { commentsVisibility, dataSaver, skipEndOfChapterPage } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import {
		ArrowLeftToLine,
		ArrowRightToLine,
		BookOpenIcon,
		Expand,
		GalleryHorizontalEnd,
		GalleryVerticalEnd,
		ImageDown,
		ImageIcon,
		ImageUp,
		Move,
		MoveHorizontal,
		MoveVertical,
		PanelLeft,
		PanelRight,
		RatioIcon
	} from "@lucide/svelte";
	import isMobile from "is-mobile";
	import Label from "../ui/label/label.svelte";
	import Separator from "../ui/separator/separator.svelte";
	import { Slider } from "../ui/slider";
	import { Switch } from "../ui/switch";
	import { useReader } from "./context.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;

	const changeLayout = async (layout: Constants.Reader.Layout) => {
		$readerPrefs.layout = layout;
		if (reader.currentPage) {
			reader.scrollIntoView(reader.currentPage);
		}
	};

	const changeSpread = async (value: boolean) => {
		$readerPrefs.double = value;
		if (reader.currentPage) {
			if (value && reader.currentPage !== reader.currentSpread[0]) {
				reader.currentPage = reader.currentSpread[0];
			}
			reader.scrollIntoView(reader.currentPage);
		}
	};

	const changeScale = async (scale: Constants.Reader.Scale) => {
		$readerPrefs.scale = scale;
		if (reader.currentPage) {
			reader.scrollIntoView(reader.currentPage);
		}
	};
</script>

<div class="space-y-1 p-2">
	<Button
		class={cn("w-full flex-auto border-none", {
			"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.Original
		})}
		variant="outline"
		onclick={() => changeScale(Constants.Reader.Scale.Original)}
	>
		<RatioIcon />
		Original Size
	</Button>
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Limit</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.LimitAll
			})}
			variant="outline"
			onclick={() => changeScale(Constants.Reader.Scale.LimitAll)}
		>
			<Expand />
			All
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.LimitWidth
			})}
			variant="outline"
			onclick={() => changeScale(Constants.Reader.Scale.LimitWidth)}
		>
			<svg class="size-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
				<path fill="currentColor" d="M9 11h6V8l4 4l-4 4v-3H9v3l-4-4l4-4zm-7 9V4h2v16zm18 0V4h2v16z" />
			</svg>
			Width
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.LimitHeight
			})}
			variant="outline"
			onclick={() => changeScale(Constants.Reader.Scale.LimitHeight)}
		>
			<svg class="size-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
				<path fill="currentColor" d="M13 9v6h3l-4 4l-4-4h3V9H8l4-4l4 4zM4 2h16v2H4zm0 18h16v2H4z" />
			</svg>
			Height
		</Button>
	</ButtonGroup.Root>
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Stretch</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.StretchAll
			})}
			variant="outline"
			onclick={() => changeScale(Constants.Reader.Scale.StretchAll)}
		>
			<Move />
			All
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.StretchWidth
			})}
			variant="outline"
			onclick={() => changeScale(Constants.Reader.Scale.StretchWidth)}
		>
			<MoveHorizontal />
			Width
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.scale === Constants.Reader.Scale.StretchHeight
			})}
			variant="outline"
			onclick={() => changeScale(Constants.Reader.Scale.StretchHeight)}
		>
			<MoveVertical />
			Height
		</Button>
	</ButtonGroup.Root>
</div>

<Separator />
<div class="space-y-1 p-2">
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Quality</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": !$dataSaver || !reader.chapter.data_optimized
			})}
			variant="outline"
			onclick={() => ($dataSaver = false)}
			disabled={!reader.chapter.data_optimized}
		>
			<ImageUp />
			Original
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $dataSaver && reader.chapter.data_optimized
			})}
			variant="outline"
			onclick={() => ($dataSaver = true)}
			disabled={!reader.chapter.data_optimized}
		>
			<ImageDown />
			Optimized
		</Button>
	</ButtonGroup.Root>
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Layout</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.layout === Constants.Reader.Layout.Paged
			})}
			variant="outline"
			onclick={() => changeLayout(Constants.Reader.Layout.Paged)}
		>
			<GalleryHorizontalEnd />
			Paged
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.layout === Constants.Reader.Layout.Scroll
			})}
			variant="outline"
			onclick={() => changeLayout(Constants.Reader.Layout.Scroll)}
		>
			<GalleryVerticalEnd />
			Long Strip
		</Button>
	</ButtonGroup.Root>
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Direction</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.direction === Constants.Reader.Direction.LTR
			})}
			variant="outline"
			onclick={() => ($readerPrefs.direction = Constants.Reader.Direction.LTR)}
		>
			<ArrowRightToLine />
			Left-to-right
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.direction === Constants.Reader.Direction.RTL
			})}
			variant="outline"
			onclick={() => ($readerPrefs.direction = Constants.Reader.Direction.RTL)}
		>
			<ArrowLeftToLine />
			Right-to-left
		</Button>
	</ButtonGroup.Root>
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Spread</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": !$readerPrefs.double
			})}
			variant="outline"
			onclick={() => changeSpread(false)}
		>
			<ImageIcon />
			Single
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.double
			})}
			variant="outline"
			onclick={() => changeSpread(true)}
		>
			<BookOpenIcon />
			Double
		</Button>
	</ButtonGroup.Root>
	<ButtonGroup.Root class="w-full">
		<ButtonGroup.Text class="w-0 max-w-25 flex-auto justify-center text-center">Panel</ButtonGroup.Text>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.panel_position === "left"
			})}
			variant="outline"
			onclick={() => ($readerPrefs.panel_position = "left")}
		>
			<PanelLeft />
			Left
		</Button>
		<ButtonGroup.Separator />
		<Button
			class={cn("w-0 flex-auto border-none", {
				"text-accent-foreground": $readerPrefs.panel_position === "right"
			})}
			variant="outline"
			onclick={() => ($readerPrefs.panel_position = "right")}
		>
			<PanelRight />
			Right
		</Button>
	</ButtonGroup.Root>
</div>

<Separator />
<div class="p-2">
	<span
		class="bg-muted block flex h-9 items-center justify-center rounded-t-md border-b text-center text-sm font-medium shadow-xs"
		>Comments Visibility</span
	>
	<ButtonGroup.Separator orientation="horizontal" />
	<ButtonGroup.Root class="w-full flex-wrap !rounded-t-none">
		{#each Object.entries(Constants.Reader.CommentsText) as [key, value], i}
			{#if i}
				<ButtonGroup.Separator />
			{/if}
			<Button
				class={cn("w-0 flex-auto !rounded-t-none border-none", {
					"text-accent-foreground":
						$commentsVisibility === (Number(key) as unknown as Constants.Reader.CommentsVisibility)
				})}
				variant="outline"
				onclick={() => ($commentsVisibility = Number(key) as unknown as Constants.Reader.CommentsVisibility)}
			>
				{value}
			</Button>
		{/each}
	</ButtonGroup.Root>
</div>
<Separator />

<div class="space-y-4 p-2 pt-3 pb-4">
	<div
		class={cn("space-y-2", {
			"pointer-events-none opacity-50": ![
				Constants.Reader.Scale.LimitWidth,
				Constants.Reader.Scale.StretchWidth
			].includes($readerPrefs.scale)
		})}
	>
		<Label class="block text-center">Maximum Page Width ({$readerPrefs.width}%)</Label>
		<Slider type="single" bind:value={$readerPrefs.width} min={10} max={100} step={1} />
	</div>
	<div
		class={cn("space-y-2", {
			"pointer-events-none opacity-50": $readerPrefs.layout !== Constants.Reader.Layout.Scroll
		})}
	>
		<Label class="block text-center">Long Strip Gap ({$readerPrefs.gap}px)</Label>
		<Slider type="single" bind:value={$readerPrefs.gap} min={0} max={100} step={1} />
	</div>
	<div
		class={cn("space-y-2", {
			"pointer-events-none opacity-50": !$readerPrefs.double
		})}
	>
		<Label class="block text-center">Spread Offset (+{$readerPrefs.offset})</Label>
		<Slider type="single" bind:value={$readerPrefs.offset} min={0} max={10} step={1} />
	</div>
</div>

<Separator />
<div class="space-y-4 p-2 pt-3 pb-4">
	<div class="space-y-2">
		<Label class="block text-center">Maximum Preload Range ({$readerPrefs.preload})</Label>
		<Slider type="single" bind:value={$readerPrefs.preload} min={1} max={100} step={1} />
	</div>
	<div class="space-y-2">
		<Label class="block text-center">Maximum Parallel Downloads ({$readerPrefs.parallel})</Label>
		<Slider type="single" bind:value={$readerPrefs.parallel} min={1} max={10} step={1} />
	</div>
</div>

<Separator />
<div class="space-y-2 p-2">
	{#if !isMobile()}
		<div class="space-y-2 py-2">
			<Label class="block text-center">Keyboard Scroll Speed ({$readerPrefs.scroll}px)</Label>
			<Slider type="single" bind:value={$readerPrefs.scroll} min={5} max={100} step={5} />
		</div>
		<div
			class={cn("flex items-center justify-between", {
				"pointer-events-none opacity-50": $readerPrefs.layout !== Constants.Reader.Layout.Scroll
			})}
		>
			<Label for="keyboard_navigation">Keyboard Navigation (Long Strip)</Label>
			<Switch id="keyboard_navigation" bind:checked={$readerPrefs.keyboard_navigation} />
		</div>
	{/if}
	<div
		class={cn("flex items-center justify-between", {
			"pointer-events-none opacity-50": $readerPrefs.layout !== Constants.Reader.Layout.Scroll
		})}
	>
		<Label for="tap_navigation">Tap Navigation (Long Strip)</Label>
		<Switch id="tap_navigation" bind:checked={$readerPrefs.tap_navigation} />
	</div>
	{#if !isMobile()}
		<div class="flex items-center justify-between">
			<Label for="scroll_navigation">Scroll Navigation</Label>
			<Switch id="scroll_navigation" bind:checked={$readerPrefs.scroll_navigation} />
		</div>
	{/if}
	<div class={cn("flex items-center justify-between")}>
		<Label for="auto_advance">Auto Advance Chapter on Last Page</Label>
		<Switch id="auto_advance" bind:checked={$readerPrefs.auto_advance} />
	</div>
	<div class={cn("flex items-center justify-between")}>
		<Label for="skip_end_of_chapter_page">Skip End of Chapter Page</Label>
		<Switch id="skip_end_of_chapter_page" bind:checked={$skipEndOfChapterPage} />
	</div>
</div>
