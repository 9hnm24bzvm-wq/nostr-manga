<script lang="ts">
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import { Session } from "$lib/stores";
	import { onMount, type Snippet } from "svelte";
	import Forbidden from "./Forbidden.svelte";
	import LoginRequired from "./LoginRequired.svelte";

	const {
		children,
		isAllowed,
		isAction
	}: {
		children: Snippet;
		isAllowed: boolean;
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
		{#if isAllowed}
			{@render children()}
		{:else}
			<Forbidden {isAction} />
		{/if}
	{:else}
		<LoginRequired {isAction} />
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
