<script lang="ts">
	import Grid from "$lib/components/ui/Grid.svelte";
	import AspectRatio from "$lib/components/ui/aspect-ratio/aspect-ratio.svelte";
	import { PlusIcon } from "@lucide/svelte";
	import CoverGroupButton from "./CoverGroupButton.svelte";
	import CoverGroupInput from "./CoverGroupInput.svelte";

	let {
		value = $bindable(),
		language,
		enabled,
		disabled
	}: {
		value: CoverDraft[];
		language: string;
		enabled: boolean;
		disabled?: boolean;
	} = $props();
	const filtered = $derived(value.filter(e => !e.deleted));
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
	{#each filtered as cover (cover)}
		<CoverGroupInput bind:value={value[value.indexOf(cover)]} bind:values={value} disabled={!enabled || disabled} />
	{/each}
	{#if enabled}
		<div>
			<AspectRatio ratio={10 / 14}>
				<CoverGroupButton class="size-full" bind:covers={value} {language}>
					<PlusIcon class="size-6" />
				</CoverGroupButton>
			</AspectRatio>
		</div>
	{/if}
</Grid>
