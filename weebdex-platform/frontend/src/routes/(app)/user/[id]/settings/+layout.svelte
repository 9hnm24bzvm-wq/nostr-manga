<script lang="ts">
	import { Permission } from "$lib";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import ProtectedVerified from "$lib/components/ui/ProtectedVerified.svelte";
	import { getUser } from "$lib/Context";
	import { Session } from "$lib/stores";

	const { children } = $props();
	const user = getUser();
</script>

<ProtectedVerified>
	<ProtectedCondition isAllowed={!!$Session && Permission.User.canUpdate($user)}>
		{@render children?.()}
	</ProtectedCondition>
</ProtectedVerified>
