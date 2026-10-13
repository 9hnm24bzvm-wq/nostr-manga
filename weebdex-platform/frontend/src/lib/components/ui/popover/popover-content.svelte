<script lang="ts">
	import { cn, type WithoutChildrenOrChild } from "$lib/Utils";
	import { Popover as PopoverPrimitive } from "bits-ui";
	import type { ComponentProps } from "svelte";
	import PopoverPortal from "./popover-portal.svelte";

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 4,
		align = "center",
		portalProps,
		...restProps
	}: PopoverPrimitive.ContentProps & {
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof PopoverPortal>>;
	} = $props();
</script>

<PopoverPortal {...portalProps}>
	<PopoverPrimitive.Content
		bind:ref
		data-slot="popover-content"
		{sideOffset}
		{align}
		class={cn(
			"bg-popover text-popover-foreground z-50 w-72 origin-(--bits-popover-content-transform-origin) rounded-md border p-4 shadow-md outline-hidden",
			className
		)}
		{...restProps}
	/>
</PopoverPortal>
