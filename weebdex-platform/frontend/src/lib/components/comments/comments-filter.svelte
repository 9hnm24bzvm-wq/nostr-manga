<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { cn } from "$lib/Utils";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import Button from "$lib/components/ui/button/button.svelte";
	import { tick } from "svelte";
	import Votes from "./comments-votes.svelte";
	import { useComments } from "./context.svelte";

	const comments = useComments();
	const isOldest = $derived(page.url.searchParams.get("sort") === "oldest");

	const changeSort = async (value: string) => {
		const q = new URLSearchParams(window.location.search);
		if (value === "oldest") q.set("sort", value);
		else q.delete("sort");

		const target = window.location.pathname + q.toString();
		if (target !== window.location.pathname + window.location.search) {
			await goto(target);
			window.requestAnimationFrame(async () => {
				await tick();
				if (comments.ref) {
					window.scroll({
						top: (comments.ref?.getBoundingClientRect().top ?? 0) - 100
					});
				}
			});
		}
	};
</script>

<div class="flex items-center justify-end gap-2">
	{#if comments.feed?.total}
		<ButtonGroup.Root>
			<Button
				class={cn("text-muted-foreground border-0", {
					"!border-accent text-accent-foreground border-b-2": !isOldest
				})}
				variant="outline"
				onclick={() => changeSort("latest")}
			>
				Latest
			</Button>
			<Button
				class={cn("text-muted-foreground border-0", {
					"!border-accent text-accent-foreground border-b-2": isOldest
				})}
				variant="outline"
				onclick={() => changeSort("oldest")}
			>
				Oldest
			</Button>
		</ButtonGroup.Root>
	{/if}
	<Votes />
</div>
