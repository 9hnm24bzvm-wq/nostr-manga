<script lang="ts">
	import { Button, type ButtonProps } from "$lib/components/ui/button";
	import * as Dialog from "$lib/components/ui/dialog";
	import { cn } from "$lib/Utils";
	import RelationGroupSearchDialog from "./RelationGroupSearchDialog.svelte";

	let {
		class: className,
		relations = $bindable(),
		children,
		...restProps
	}: {
		relations: RelationDraft[];
	} & ButtonProps = $props();
</script>

<Dialog.Root>
	<Dialog.Trigger>
		{#snippet child({ props })}
			<Button
				class={cn("w-full border border-dashed !bg-transparent", className)}
				variant="ghost"
				{...props}
				{...restProps}
			>
				{@render children?.()}
			</Button>
		{/snippet}
	</Dialog.Trigger>
	<Dialog.Content
		class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(93rem,calc(100dvw-2rem))] flex-col overflow-auto"
		showCloseButton={false}
	>
		<RelationGroupSearchDialog bind:relations />
	</Dialog.Content>
</Dialog.Root>
