<script lang="ts">
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
		{@render children()}
	{:else}
		<LoginRequired {isAction} />
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
