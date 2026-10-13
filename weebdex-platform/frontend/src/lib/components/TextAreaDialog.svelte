<script lang="ts">
	import Mutex from "$lib/Mutex";
	import type { Snippet } from "svelte";
	import { Button, buttonVariants } from "./ui/button";
	import * as Dialog from "./ui/dialog";
	import Label from "./ui/label/label.svelte";
	import { Spinner } from "./ui/spinner";
	import { Textarea } from "./ui/textarea";

	const {
		child: children,
		variant,

		title,
		description,

		label,
		placeholder,

		confirmText,
		onConfirm,

		disabled
	}: {
		child: Snippet<[{ props: Record<string, unknown> }]>;
		variant?: "default" | "destructive";

		title: string | Snippet;
		description: string | Snippet;

		label?: string;
		placeholder?: string;

		confirmText: string | Snippet;
		onConfirm: (mutex: Mutex, value: string) => Promise<void>;

		disabled?: boolean;
	} = $props();
	const mutex = new Mutex();

	let open = $state(false);
	let value = $state("");
</script>

<Dialog.Root bind:open>
	<Dialog.Trigger disabled={$mutex || disabled}>
		{#snippet child({ props })}
			{@render children({ props })}
		{/snippet}
	</Dialog.Trigger>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>
				{#if typeof title === "string"}
					{title}
				{:else}
					{@render title()}
				{/if}
			</Dialog.Title>
			<Dialog.Description>
				{#if typeof description === "string"}
					{description}
				{:else}
					{@render description()}
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-2">
			{#if label}
				<Label>{label}</Label>
			{/if}
			<Textarea class="border-none !ring-0 !ring-transparent" {placeholder} bind:value disabled={$mutex || disabled} />
		</div>
		<Dialog.Footer>
			<Dialog.Close
				class={buttonVariants({ class: "border-none", variant: "outline" })}
				type="button"
				disabled={$mutex}
			>
				Cancel
			</Dialog.Close>
			<Button
				class="w-30"
				type="button"
				{variant}
				onclick={async () => {
					try {
						await onConfirm(mutex, value);
					} finally {
						open = false;
					}
				}}
				disabled={$mutex}
			>
				{#if $mutex}
					<Spinner />
				{:else if typeof confirmText === "string"}
					{confirmText}
				{:else}
					{@render confirmText()}
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
