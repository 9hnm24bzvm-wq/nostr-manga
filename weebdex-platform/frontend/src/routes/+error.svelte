<script lang="ts">
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import { StatusCodes } from "http-status-codes";
</script>

<Empty.Root class="min-h-dvh">
	<Empty.Header>
		{#if page.status === StatusCodes.FORBIDDEN}
			<Empty.Title>403 - Forbidden</Empty.Title>
			<Empty.Description>You don't have permission to access this page.</Empty.Description>
		{:else if page.status === StatusCodes.NOT_FOUND}
			<Empty.Title>404 - Not Found</Empty.Title>
			<Empty.Description>The page you're looking for doesn't exist.</Empty.Description>
		{:else if page.status === StatusCodes.TOO_MANY_REQUESTS}
			<Empty.Title>429 - {Constants.Error.TooManyRequests.title}</Empty.Title>
			<Empty.Description>{Constants.Error.TooManyRequests.body}</Empty.Description>
		{:else}
			<Empty.Title>{Constants.Error.Unknown.title}</Empty.Title>
			<Empty.Description>{Constants.Error.Unknown.body}</Empty.Description>
		{/if}
	</Empty.Header>
	<Empty.Content>
		<a class={buttonVariants({ variant: "outline" })} href="/">Home</a>
	</Empty.Content>
</Empty.Root>
