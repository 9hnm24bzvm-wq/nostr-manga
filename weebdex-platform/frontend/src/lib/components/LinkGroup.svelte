<script lang="ts">
	import type { AuthorLink, ExternalLink, GroupLink, UserLink } from "$lib";
	import LinkGroupInput from "./LinkGroupInput.svelte";

	let {
		value = $bindable(),
		module,
		disabled
	}: {
		value: ExternalLink[];
		module: typeof UserLink | typeof ExternalLink | typeof AuthorLink | typeof GroupLink;
		disabled?: boolean;
	} = $props();
</script>

{#each value as link, idx (link)}
	<LinkGroupInput bind:value={value[idx]} bind:values={value} {module} {disabled} />
{/each}
