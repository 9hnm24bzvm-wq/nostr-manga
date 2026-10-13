<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import * as Empty from "$lib/components/ui/empty";
</script>

<svelte:head>
	<title>Popular - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<Empty.Root>
	<Empty.Header>
		<Empty.Title>Not yet available</Empty.Title>
		<Empty.Description>
			We're waiting for the database to be populated with data. Please check back later.
		</Empty.Description>
	</Empty.Header>
</Empty.Root>
