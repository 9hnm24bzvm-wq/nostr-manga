<script lang="ts">
	import CoverGroup from "$lib/components/draft/manga/CoverGroup.svelte";
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
		<Label for="covers">
			Covers
			{#if draft.covers.length}
				({draft.covers.length})
			{/if}
		</Label>
		<p class="text-foreground/85">
			Use the original cover in the original language when possible. At least 1 cover in the original language in all
			cases. Prefer the best quality available. The format must be JPEG, PNG, or GIF and portrait aspect ratio is
			preferred.
		</p>
		<p class="text-foreground/85">Covers may take a while to update if you changed them.</p>
	</div>
	<CoverGroup bind:value={draft.covers} language={draft.language} {enabled} {disabled} />
	{#if !enabled}
		<p class="text-destructive font-medium">You do not have permission to edit covers.</p>
	{/if}
</div>
