<script lang="ts">
	import { Permission } from "$lib";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import ProtectedVerified from "$lib/components/ui/ProtectedVerified.svelte";
	import { getAuthor } from "$lib/Context";
	import { Session } from "$lib/stores";

	const { children } = $props();
	const author = getAuthor();
</script>

<ProtectedVerified>
	<ProtectedCondition isAllowed={!!$Session && Permission.Author.canUpdate($author)}>
		{@render children?.()}
	</ProtectedCondition>
</ProtectedVerified>
