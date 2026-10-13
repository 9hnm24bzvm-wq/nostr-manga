<script lang="ts">
	import { onDestroy, onMount } from "svelte";
	import type { HTMLAttributes } from "svelte/elements";

	const {
		disabled = $bindable(false),
		children,
		...restProps
	}: HTMLAttributes<HTMLDivElement> & { disabled?: boolean } = $props();

	let isExpandable = $state(false);
	let isExpanded = $state(false);

	let RAF = $state(0);
	let ref = $state<HTMLElement>();
	let height = $derived(ref?.clientHeight ?? 0);
	let scrollHeight = $derived(ref?.scrollHeight ?? 0);

	const assign = () => {
		window.cancelAnimationFrame(RAF);
		RAF = window.requestAnimationFrame(() => {
			height = ref?.clientHeight ?? 0;
			scrollHeight = ref?.scrollHeight ?? 0;
			isExpandable = height != scrollHeight;
		});
	};

	onMount(() => {
		assign();
		window.addEventListener("resize", assign);
	});

	onDestroy(() => {
		window.removeEventListener("resze", assign);
	});
</script>

<div class="relative">
	<div class:!max-h-none={isExpanded || disabled} {...restProps} bind:this={ref}>
		{@render children?.()}
	</div>
	{#if !disabled && isExpandable}
		{#if !isExpanded}
			<div class="gradient pointer-events-none absolute inset-x-0 bottom-0 h-25"></div>
		{/if}
		<button
			class="hover:text-accent-foreground relative font-medium"
			type="button"
			onclick={() => (isExpanded = !isExpanded)}
		>
			{#if isExpanded}
				[Show Less]
			{:else}
				[Show More]
			{/if}
		</button>
	{/if}
</div>

<style>
	.gradient {
		background-image: linear-gradient(0deg, var(--background), transparent);
	}
</style>
