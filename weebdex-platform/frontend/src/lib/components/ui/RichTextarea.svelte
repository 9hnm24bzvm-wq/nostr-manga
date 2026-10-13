<script lang="ts">
	import { Typography } from "$lib";
	import { Button } from "$lib/components/ui/button";
	import * as InputGroup from "$lib/components/ui/input-group";
	import * as Popover from "$lib/components/ui/popover";
	import { Separator } from "$lib/components/ui/separator";
	import * as Tabs from "$lib/components/ui/tabs";
	import { Textarea } from "$lib/components/ui/textarea";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { cn } from "$lib/Utils";
	import {
		BoldIcon,
		ChevronsRightLeft,
		EyeOffIcon,
		ImageIcon,
		ItalicIcon,
		StrikethroughIcon,
		UnderlineIcon
	} from "@lucide/svelte";
	import type { HTMLTextareaAttributes } from "svelte/elements";

	let {
		class: className,
		value = $bindable(),
		maxlength = 30000,
		parser,
		disabled,

		...restProps
	}: Omit<HTMLTextareaAttributes, "value" | "maxlength"> & {
		value?: string;
		maxlength?: number;
		parser: (value?: string) => string | undefined;
	} = $props();
	const html = $derived(parser(value));

	let ref = $state<HTMLTextAreaElement | null>(null);
	let tab = $state("edit");
	let isEmbed = $state(false);

	const tools = [
		{ text: "Bold", icon: BoldIcon, onclick: () => formatSelection("**", "**") },
		{ text: "Italic", icon: ItalicIcon, onclick: () => formatSelection("*", "*") },
		{ text: "Underline", icon: UnderlineIcon, onclick: () => formatSelection("__", "__") },
		{ text: "Strikethrough", icon: StrikethroughIcon, onclick: () => formatSelection("~~", "~~") },
		{ text: "Spoiler", icon: EyeOffIcon, onclick: () => formatSelection("||", "||") },
		{
			text: "Collapsible",
			icon: ChevronsRightLeft,
			onclick: () => formatSelection("+++Spoiler\n", "\n+++", "Input something here...")
		}
	];

	const formatSelection = (prefix: string, suffix: string, defaultText = "") => {
		if (!ref) {
			return;
		}

		const start = ref.selectionStart;
		const end = ref.selectionEnd;

		const selected = value?.slice(start, end) || defaultText;
		const isRemove = selected.startsWith(prefix) && selected.endsWith(suffix);

		let replacement: string;
		if (isRemove) {
			replacement = selected.slice(prefix.length, selected.length - suffix.length);
		} else replacement = prefix + selected + suffix;

		value = value?.slice(0, start) + replacement + value?.slice(end);
		requestAnimationFrame(() => {
			if (!ref) return;
			ref.focus();

			if (isRemove) ref.setSelectionRange(start, start + selected.length - prefix.length - suffix.length);
			else if (selected) ref.setSelectionRange(start, start + selected.length + prefix.length + suffix.length);
			else ref.setSelectionRange(start + prefix.length, start + prefix.length);
		});
	};

	const onImageEmbed = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		const url = (ev.target as any)?.url.value;
		if (!url) {
			return;
		}

		isEmbed = false;

		if (value?.trim() && !value.endsWith("\n")) {
			value += "\n";
		}
		value += `![](${url})`;
		ref?.focus();
	};
</script>

<Tabs.Root class="bg-input/30 !gap-0 overflow-hidden rounded-md" bind:value={tab}>
	<div class="flex items-center justify-between border-b">
		<Tabs.List class="bg-transparent p-0">
			<Tabs.Trigger value="edit" {disabled}>
				{#snippet child({ props: { class: _, ...props } })}
					<Button
						class={cn("rounded-none border-none", {
							"!bg-transparent": props["aria-selected"] !== "true",
							"!bg-input/20 text-accent-foreground": props["aria-selected"] === "true"
						})}
						variant="outline"
						{...props}
					>
						Edit
					</Button>
				{/snippet}
			</Tabs.Trigger>
			<Tabs.Trigger value="preview" disabled={disabled || !value?.trim().length || !html?.length}>
				{#snippet child({ props: { class: _, ...props } })}
					<Button
						class={cn("rounded-none border-none", {
							"!bg-transparent": props["aria-selected"] !== "true",
							"!bg-input/20 text-accent-foreground": props["aria-selected"] === "true"
						})}
						variant="outline"
						type="button"
						{...props}
					>
						Preview
					</Button>
				{/snippet}
			</Tabs.Trigger>
		</Tabs.List>
	</div>
	<Tabs.Content value="edit">
		<Tooltip.Provider delayDuration={0} disableHoverableContent>
			<div class="bg-input/20 flex items-center border-b">
				{#each tools as tool}
					<Tooltip.Root>
						<Tooltip.Trigger onclick={tool.onclick}>
							{#snippet child({ props })}
								<Button class="rounded-none" variant="ghost" size="icon-sm" {...props} {disabled}>
									<tool.icon />
								</Button>
							{/snippet}
						</Tooltip.Trigger>
						<Tooltip.Content>
							{tool.text}
						</Tooltip.Content>
					</Tooltip.Root>
				{/each}
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Popover.Root bind:open={isEmbed}>
								<Popover.Trigger {...props}>
									{#snippet child({ props: { class: className, ...props } })}
										<Button class="!rounded-none" variant="ghost" size="icon-sm" {...props} {disabled}>
											<ImageIcon />
										</Button>
									{/snippet}
								</Popover.Trigger>
								<Popover.Content class="rounded-md p-1">
									<form onsubmit={onImageEmbed}>
										<InputGroup.Root class="border-none !ring-0 !ring-transparent">
											<InputGroup.Input name="url" type="url" placeholder="Image URL" required />
											<InputGroup.Addon align="inline-end">
												<InputGroup.Button type="submit">OK</InputGroup.Button>
											</InputGroup.Addon>
										</InputGroup.Root>
									</form>
								</Popover.Content>
							</Popover.Root>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>Embed Image</Tooltip.Content>
				</Tooltip.Root>
				<InputGroup.Text
					class={cn("ms-auto mr-2", {
						"text-red-300": maxlength - (value?.length || 0) < 0
					})}
				>
					{(maxlength - (value?.length || 0)).toLocaleString()}
				</InputGroup.Text>
				<Separator class="!h-4" orientation="vertical" />
			</div>
			<Textarea
				class={cn("min-h-30 border-none !bg-transparent !ring-0 !ring-transparent", className)}
				bind:value
				bind:ref
				{disabled}
				{...restProps}
			/>
		</Tooltip.Provider>
	</Tabs.Content>
	<Tabs.Content class="bg-card" value="preview">
		<div class="typography space-y-2 p-3" {@attach Typography.observer}>
			{@html html}
		</div>
	</Tabs.Content>
</Tabs.Root>
