<script lang="ts">
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import { cn, type WithElementRef } from "$lib/Utils";
	import type { HTMLAttributes } from "svelte/elements";
	import { SIDEBAR_WIDTH, SIDEBAR_WIDTH_ICON } from "./constants.js";

	let {
		ref = $bindable(null),
		class: className,
		style,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLDivElement>> = $props();
</script>

<Tooltip.Provider delayDuration={0}>
	<div
		data-slot="sidebar-wrapper"
		style="--sidebar-width: {window.location.pathname.startsWith('/chapter') &&
		!window.location.pathname.endsWith('/edit')
			? '28.125rem'
			: SIDEBAR_WIDTH}; --sidebar-width-icon: {SIDEBAR_WIDTH_ICON}; {style}"
		class={cn("group/sidebar-wrapper has-data-[variant=inset]:bg-sidebar flex min-h-svh w-full", className)}
		bind:this={ref}
		{...restProps}
	>
		{@render children?.()}
	</div>
</Tooltip.Provider>
