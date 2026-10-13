<script lang="ts">
	import { API } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import Button from "$lib/components/ui/button/button.svelte";
	import { PersistentStore } from "$lib/hooks";
	import { prefs } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { ArrowRightIcon, ChevronUp, LayoutGrid, LayoutList } from "@lucide/svelte";
	import ChapterFeed from "./ChapterFeed.svelte";
	import LatestUpdatesGrid from "./LatestUpdatesGrid.svelte";

	const isCollapsed = new PersistentStore("xlhreqxa", false);
	const view = new PersistentStore("a0yb25h8", "grid");
</script>

<section>
	<header class="flex items-center gap-2" class:mb-3={!$isCollapsed}>
		<div class="flex w-0 flex-auto items-center gap-2">
			<h2 class="text-xl font-medium">Latest Updates</h2>
			<Button type="button" class="relative" variant="ghost" size="icon" onclick={() => ($isCollapsed = !$isCollapsed)}>
				<ChevronUp class={cn("size-6 transition-all", $isCollapsed && "-rotate-180")} />
			</Button>
		</div>
		<div class="flex shrink-0 items-center gap-2">
			<ButtonGroup.Root>
				<Button
					class={cn("text-muted-foreground border-0", {
						"text-accent-foreground !border-accent border-b-2": $view === "list"
					})}
					variant="outline"
					size="icon"
					type="button"
					onclick={() => ($view = "list")}
				>
					<LayoutList strokeWidth={2.5} />
					<span class="sr-only">List View</span>
				</Button>
				<Button
					class={cn("text-muted-foreground border-0", {
						"text-accent-foreground !border-accent border-b-2": $view === "grid"
					})}
					variant="outline"
					size="icon"
					type="button"
					onclick={() => ($view = "grid")}
				>
					<LayoutGrid strokeWidth={2.5} />
					<span class="sr-only">Grid View</span>
				</Button>
			</ButtonGroup.Root>
			<a class={buttonVariants({ variant: "ghost", size: "icon" })} href="/updates">
				<ArrowRightIcon class="size-6" />
			</a>
		</div>
	</header>
	<div class:hidden={$isCollapsed}>
		{#if $view === "grid"}
			<LatestUpdatesGrid />
		{:else}
			<ChapterFeed
				onSearch={(n?: number) => {
					const q = new URLSearchParams();
					prefs.applyMangaPreferences(q);
					prefs.applyChapterPreferences(q);

					if (n) q.set("page", n.toString());
					q.set("limit", "100");

					return API.Chapter.LatestFeed(q);
				}}
			/>
		{/if}
	</div>
</section>
