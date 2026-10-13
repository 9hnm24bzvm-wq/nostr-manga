<script lang="ts">
	import { cn, type WithoutChild } from "$lib/Utils";
	import { ScrollArea as ScrollAreaPrimitive } from "bits-ui";
	import { onMount } from "svelte";
	import ScrollArea from "./scroll-area/scroll-area.svelte";

	const {
		class: className,
		showShadow,
		children,
		...restProps
	}: WithoutChild<ScrollAreaPrimitive.RootProps> & {
		showShadow?: boolean;
		orientation?: "vertical" | "horizontal" | "both" | undefined;
		scrollbarXClasses?: string | undefined;
		scrollbarYClasses?: string | undefined;
		viewportRef?: HTMLElement | null;
	} = $props();
	let RAF = $state(0);
	let ref = $state<HTMLElement>(null!);

	let height = $derived(ref?.clientHeight ?? 0);
	let scrollHeight = $derived(ref?.scrollHeight ?? 0);

	const assign = () => {
		window.cancelAnimationFrame(RAF);
		RAF = window.requestAnimationFrame(() => {
			height = ref?.clientHeight ?? 0;
			scrollHeight = ref?.scrollHeight ?? 0;
		});
	};
	onMount(assign);
</script>

<svelte:window onresize={assign} ontransitionend={assign} />

<ScrollArea
	class={cn("relative", className)}
	{...restProps}
	style={scrollHeight ? `height: ${scrollHeight}px` : undefined}
	bind:ref
>
	{@render children?.()}
</ScrollArea>
