<script lang="ts">
	import { ExternalLink } from "$lib";
	import LinkGroup from "$lib/components/LinkGroup.svelte";
	import LinkGroupButton from "$lib/components/LinkGroupButton.svelte";
	import Label from "$lib/components/ui/label/label.svelte";
	import { PlusIcon } from "@lucide/svelte";

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
	<Label for="links">
		External Links
		{#if draft.links.length}
			({draft.links.length})
		{/if}
	</Label>
	{#if draft.links.length > 0}
		<div class="space-y-1">
			<LinkGroup bind:value={draft.links} module={ExternalLink} disabled={!enabled || disabled} />
		</div>
	{/if}
	{#if !enabled}
		<p class="text-destructive font-medium">You do not have permission to edit links.</p>
	{:else}
		<LinkGroupButton bind:links={draft.links} {disabled}>
			<PlusIcon />
			Add External Link
		</LinkGroupButton>
	{/if}
</div>
