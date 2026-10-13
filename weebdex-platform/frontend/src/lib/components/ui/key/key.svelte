<script lang="ts">
	import { setKey } from "./context.svelte";

	const { children } = $props();
	const key = setKey();
</script>

{#key key.value}
	{@render children?.()}
{/key}
