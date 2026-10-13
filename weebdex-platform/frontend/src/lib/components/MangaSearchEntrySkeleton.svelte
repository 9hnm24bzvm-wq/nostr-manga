<script lang="ts">
	import { AspectRatio } from "./ui/aspect-ratio";
	import { Skeleton } from "./ui/skeleton";

	const { isGrid }: { isGrid?: boolean } = $props();
</script>

{#if isGrid}
	<AspectRatio ratio={10 / 14} class="bg-card size-full rounded-md">
		{#snippet child({ props })}
			<Skeleton {...props} />
		{/snippet}
	</AspectRatio>
{:else}
	<div class="group flex gap-4">
		<figure class="w-[35%] shrink-0 md:w-[30%] lg:w-[25%]">
			<AspectRatio ratio={10 / 14} class="bg-card rounded-md">
				{#snippet child({ props })}
					<Skeleton {...props} />
				{/snippet}
			</AspectRatio>
		</figure>
		<div class="flex w-0 flex-auto flex-col">
			<Skeleton class="bg-card mb-2 h-4" />
			<div class="flex h-0 flex-auto flex-col space-y-2">
				<Skeleton class="bg-card h-4 w-1/2 shrink-0" />
				<Skeleton class="bg-card h-0 flex-auto" />
			</div>
		</div>
	</div>
{/if}
