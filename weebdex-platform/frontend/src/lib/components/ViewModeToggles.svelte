<script lang="ts">
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import { mangaView } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { LayoutGrid, LayoutList } from "@lucide/svelte";
	import { Button } from "./ui/button";

	const { class: className }: { class?: string } = $props();
</script>

<ButtonGroup.Root class={className}>
	<Button
		class={cn("text-muted-foreground border-0", {
			"!border-accent text-accent-foreground border-b-2": $mangaView === "list"
		})}
		variant="outline"
		size="icon"
		type="button"
		onclick={() => ($mangaView = "list")}
	>
		<LayoutList strokeWidth={2.5} />
		<span class="sr-only">List View</span>
	</Button>
	<Button
		class={cn("text-muted-foreground border-0", {
			"!border-accent text-accent-foreground border-b-2": $mangaView === "grid"
		})}
		variant="outline"
		size="icon"
		type="button"
		onclick={() => ($mangaView = "grid")}
	>
		<LayoutGrid strokeWidth={2.5} />
		<span class="sr-only">Grid View</span>
	</Button>
</ButtonGroup.Root>
