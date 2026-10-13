<script lang="ts">
	import * as Empty from "$lib/components/ui/empty";

	const {
		isAction
	}: {
		isAction?: boolean;
	} = $props();
</script>

<Empty.Root>
	<Empty.Header>
		<Empty.Title>Forbidden</Empty.Title>
		<Empty.Description>
			{isAction
				? "You do not have permission to perform this action."
				: "You do not have permission to access this page."}
		</Empty.Description>
	</Empty.Header>
</Empty.Root>
