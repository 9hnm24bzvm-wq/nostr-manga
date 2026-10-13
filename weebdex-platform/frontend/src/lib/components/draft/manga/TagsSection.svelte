<script lang="ts">
	import TagComboBox from "$lib/components/TagComboBox.svelte";

	let {
		draft = $bindable(),
		enabled,
		disabled
	}: {
		draft: MangaDraft;
		enabled: boolean;
		disabled: boolean;
	} = $props();
</script>

<div class="space-y-2">
	<TagComboBox bind:value={draft.tags} disabled={!enabled || disabled} />
	{#if !enabled}
		<p class="text-destructive font-medium">You do not have permission to edit tags.</p>
	{/if}
</div>
