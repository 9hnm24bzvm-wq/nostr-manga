<script lang="ts">
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import { Session } from "$lib/stores";
	import { onMount, type Snippet } from "svelte";
	import LoginRequired from "./LoginRequired.svelte";

	const {
		children,
		isAction
	}: {
		children: Snippet;
		isAction?: boolean;
	} = $props();
	let isMounted = $state(false);

	onMount(async () => {
		await Session.isValid();
		isMounted = true;
	});
</script>

{#if isMounted}
	{#if $Session}
		{#if $Session.roles?.includes(Constants.User.Role.Verified)}
			{@render children()}
		{:else}
			<Empty.Root>
				<Empty.Header>
					<Empty.Title>Forbidden</Empty.Title>
					<Empty.Description>
						{isAction
							? "You need to verify your account to perform this action."
							: "You need to verify your account to access this page."}
					</Empty.Description>
				</Empty.Header>
				<Empty.Content>
					<div class="flex flex-wrap items-center justify-center gap-2">
						<a
							class={buttonVariants({ variant: "default" })}
							href="/auth/verification{page.url.searchParams.toString()}"
						>
							Verify
						</a>
					</div>
				</Empty.Content>
			</Empty.Root>
		{/if}
	{:else}
		<LoginRequired {isAction} />
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
