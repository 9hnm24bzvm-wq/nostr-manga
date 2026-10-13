<script lang="ts">
	import { Permission } from "$lib";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import ProtectedVerified from "$lib/components/ui/ProtectedVerified.svelte";
	import { getGroup } from "$lib/Context";
	import { Session } from "$lib/stores";

	const { children } = $props();
	const group = getGroup();
</script>

<ProtectedVerified>
	<ProtectedCondition isAllowed={!!$Session && Permission.Group.canUpdate($group)}>
		{@render children?.()}
	</ProtectedCondition>
</ProtectedVerified>
