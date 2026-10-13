<script lang="ts">
	import TitleGroupInput from "./TitleGroupInput.svelte";

	let {
		value = $bindable(),
		disabled
	}: {
		value: AltTitle[];
		disabled?: boolean;
	} = $props();
</script>

{#each value as title, idx (title)}
	<TitleGroupInput bind:value={value[idx]} bind:values={value} {disabled} />
{/each}
