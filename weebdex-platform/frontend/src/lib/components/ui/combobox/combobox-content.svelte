<script lang="ts">
	import * as Command from "$lib/components/ui/command/index.js";
	import SelectPortal from "$lib/components/ui/select/select-portal.svelte";
	import type { WithoutChildrenOrChild } from "$lib/Utils";
	import { cn, type WithoutChild } from "$lib/Utils";
	import { Select as SelectPrimitive } from "bits-ui";
	import type { ComponentProps } from "svelte";

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 4,
		portalProps,
		children,
		preventScroll = true,
		...restProps
	}: WithoutChild<SelectPrimitive.ContentProps> & {
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof SelectPortal>>;
	} = $props();
</script>

<SelectPortal {...portalProps}>
	<SelectPrimitive.Content
		bind:ref
		{sideOffset}
		{preventScroll}
		data-slot="select-content"
		class={cn(
			"bg-popover text-popover-foreground relative z-50 max-h-(--bits-select-content-available-height) min-w-[8rem] origin-(--bits-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-md border shadow-md data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 data-[side=right]:translate-x-1 data-[side=top]:-translate-y-1",
			className
		)}
		{...restProps}
	>
		<Command.Root class="rounded-lg border shadow-md md:min-w-[450px]">
			{#snippet child()}
				<Command.Input placeholder="Search..." autofocus />
				<SelectPrimitive.Viewport
					class={cn("h-(--bits-select-anchor-height) w-full min-w-(--bits-select-anchor-width) scroll-my-1 p-1")}
				>
					<Command.List>
						{#snippet child({ props: { class: _, ...props } })}
							<div class="max-h-75" {...props}>
								<Command.Empty>No results found.</Command.Empty>
								{@render children?.()}
							</div>
						{/snippet}
					</Command.List>
				</SelectPrimitive.Viewport>
			{/snippet}
		</Command.Root>
	</SelectPrimitive.Content>
</SelectPortal>
