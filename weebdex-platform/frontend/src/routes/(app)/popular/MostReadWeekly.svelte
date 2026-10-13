<script lang="ts">
	import Button from "$lib/components/ui/button/button.svelte";
	import { cn } from "$lib/Utils";
	import { ChevronUp } from "@lucide/svelte";
	import MostReadGrid from "./MostReadGrid.svelte";

	let isCollapsed = $state(false);
</script>

<section>
	<header class="flex items-center gap-2" class:mb-3={!isCollapsed}>
		<div class="flex w-0 flex-auto items-center gap-2">
			<h2 class="text-xl font-medium">Weekly Top 10</h2>
			<Button type="button" class="relative" variant="ghost" size="icon" onclick={() => (isCollapsed = !isCollapsed)}>
				<ChevronUp class={cn("size-6 transition-all", isCollapsed && "-rotate-180")} />
			</Button>
		</div>
	</header>
	<div class:hidden={isCollapsed}>
		<MostReadGrid time="7d" />
	</div>
</section>
