<script lang="ts">
	import { Permission } from "$lib";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import ProtectedVerified from "$lib/components/ui/ProtectedVerified.svelte";
	import { Session } from "$lib/stores";

	const { children } = $props();
</script>

<ProtectedVerified>
	<ProtectedCondition isAllowed={!!$Session && Permission.Manga.canCreate()}>
		{@render children?.()}
	</ProtectedCondition>
</ProtectedVerified>
