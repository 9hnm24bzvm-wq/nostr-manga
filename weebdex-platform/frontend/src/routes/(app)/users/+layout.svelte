<script>
	import ProtectedVerified from "$lib/components/ui/ProtectedVerified.svelte";

	const { children } = $props();
</script>

<ProtectedVerified>
	{@render children?.()}
</ProtectedVerified>
