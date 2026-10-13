<script lang="ts">
	import RelationGroup from "$lib/components/draft/manga/RelationGroup.svelte";
	import Label from "$lib/components/ui/label/label.svelte";

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
	<div class="space-y-1">
		<Label for="relations">
			Relations
			{#if draft.relations.length}
				({draft.relations.length})
			{/if}
		</Label>
		<p class="text-foreground/85">Relations may take a while to update if you changed them.</p>
	</div>
	<RelationGroup bind:value={draft.relations} {enabled} {disabled} />
	{#if !enabled}
		<p class="text-destructive font-medium">You do not have permission to edit relations.</p>
	{/if}
</div>
