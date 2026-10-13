<script lang="ts">
	import Grid from "$lib/components/ui/Grid.svelte";
	import AspectRatio from "$lib/components/ui/aspect-ratio/aspect-ratio.svelte";
	import { PlusIcon } from "@lucide/svelte";
	import RelationGroupButton from "./RelationGroupButton.svelte";
	import RelationGroupInput from "./RelationGroupInput.svelte";

	let {
		value = $bindable(),
		enabled,
		disabled
	}: {
		value: RelationDraft[];
		enabled: boolean;
		disabled?: boolean;
	} = $props();
	const filtered = $derived(value.filter(relation => !relation.deleted));
</script>

<Grid
	class="gap-3 select-none"
	breakpoints={{
		0: 5,
		960: 4,
		770: 3,
		580: 2
	}}
>
	{#each filtered as relation (relation)}
		<RelationGroupInput
			bind:value={value[value.indexOf(relation)]}
			bind:values={value}
			disabled={!enabled || disabled}
		/>
	{/each}
	{#if enabled}
		<div>
			<AspectRatio ratio={10 / 14}>
				<RelationGroupButton class="size-full" bind:relations={value}>
					<PlusIcon class="size-6" />
				</RelationGroupButton>
			</AspectRatio>
		</div>
	{/if}
</Grid>
