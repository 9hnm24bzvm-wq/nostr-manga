<script lang="ts">
	import type { Constants } from "$lib";
	import * as Empty from "$lib/components/ui/empty";

	const { error }: { error: Constants.Error.Error } = $props();
</script>

<Empty.Root>
	<Empty.Header>
		<Empty.Title>{error.title}</Empty.Title>
		<Empty.Description>{error.body}</Empty.Description>
	</Empty.Header>
</Empty.Root>
