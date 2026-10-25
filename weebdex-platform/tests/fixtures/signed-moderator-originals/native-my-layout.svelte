<script>
	import Protected from "$lib/components/ui/Protected.svelte";

	const { children } = $props();
</script>

<Protected>
	{@render children?.()}
</Protected>
