<script>
	import { Constants } from "$lib";
	import Protected from "$lib/components/ui/Protected.svelte";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import { Session } from "$lib/stores";

	const { children } = $props();
</script>

<Protected>
	<ProtectedCondition isAllowed={Session.hasRole(Constants.User.Role.User)}>
		{@render children?.()}
	</ProtectedCondition>
</Protected>
