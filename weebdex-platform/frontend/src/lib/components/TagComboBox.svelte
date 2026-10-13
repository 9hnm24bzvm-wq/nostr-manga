<script lang="ts">
	import { Constants } from "$lib";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import * as Command from "$lib/components/ui/command/index.js";
	import Grid from "$lib/components/ui/Grid.svelte";
	import Label from "$lib/components/ui/label/label.svelte";
	import { tagList } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import type { Snippet } from "svelte";

	let {
		children,
		value = $bindable(),
		indeterminate = $bindable(),
		variant,
		disabled,

		class: className,
		breakpoints = { 0: 5, 1080: 4, 896: 3, 768: 4, 620: 3, 478: 2 }
	}: {
		children?: Snippet;
		value: string[];
		indeterminate?: string[];
		variant?: "default" | "destructive";
		disabled?: boolean;

		class?: string;
		breakpoints?: Record<number, number>;
	} = $props();
</script>

{#if $tagList}
	<Command.Root>
		{#snippet child()}
			<div class="space-y-3">
				<div class="bg-input/30">
					<Command.Input placeholder="Search tags..." />
				</div>
				<Command.List>
					{#snippet child({ props: { class: _, ...restProps } })}
						<div class={cn("space-y-4", className)} {...restProps}>
							<Command.Empty>No results found.</Command.Empty>
							{#each Object.entries($tagList) as [group, tags]}
								<Command.Group>
									{#snippet child({ props: { class: _, ...restProps } })}
										<section {...restProps}>
											<header class="mb-2">
												<strong class="font-medium">{Constants.Tag.GroupText[group]}</strong>
											</header>
											<Grid class="gap-1.5" {breakpoints}>
												{#each tags as tag}
													{@const checkedIndex = value.findIndex(e => e === tag.id)}
													{@const indeterminateIndex = indeterminate?.findIndex(e => e === tag.id)}
													<Command.Item>
														{#snippet child({ props: { class: _, ...restProps } })}
															<div class="group flex items-center" {...restProps}>
																{#if indeterminate && indeterminateIndex !== undefined}
																	{#key [checkedIndex, indeterminateIndex]}
																		<Checkbox
																			class={cn("shrink-0", {
																				"!bg-destructive !text-black": indeterminateIndex >= 0
																			})}
																			id="tag-{tag.id}"
																			checked={checkedIndex >= 0}
																			indeterminate={indeterminateIndex >= 0}
																			onCheckedChange={() => {
																				if (checkedIndex >= 0) {
																					value.splice(checkedIndex, 1);
																					indeterminate!.push(tag.id);
																					indeterminate = indeterminate!.sort((a, b) => a.localeCompare(b));
																				} else if (indeterminateIndex >= 0) {
																					indeterminate!.splice(indeterminateIndex, 1);
																				} else {
																					value.push(tag.id);
																					value = value.sort((a, b) => a.localeCompare(b));
																				}
																			}}
																		/>
																	{/key}
																	<Label
																		class={cn(
																			"text-foreground/85 group-hover:text-foreground flex-auto cursor-pointer pr-5 pl-2 font-normal whitespace-nowrap",
																			{
																				"text-foreground": checkedIndex >= 0,
																				"!text-destructive line-through": indeterminateIndex >= 0
																			}
																		)}
																		for="tag-{tag.id}"
																	>
																		{tag.name}
																	</Label>
																{:else}
																	<Checkbox
																		class={cn("shrink-0", {
																			"!bg-destructive !border-destructive !text-b	lack":
																				checkedIndex >= 0 && variant === "destructive"
																		})}
																		id="tag-{tag.id}"
																		checked={checkedIndex >= 0}
																		onCheckedChange={() => {
																			if (checkedIndex >= 0) value.splice(checkedIndex, 1);
																			else value.push(tag.id);
																			value = value.sort((a, b) => a.localeCompare(b));
																		}}
																		{disabled}
																	/>
																	<Label
																		class={cn(
																			"text-foreground/85 group-hover:text-foreground flex-auto cursor-pointer pr-5 pl-2 font-normal whitespace-nowrap",
																			{
																				"!text-destructive line-through": checkedIndex >= 0 && variant === "destructive"
																			}
																		)}
																		for="tag-{tag.id}"
																	>
																		{tag.name}
																	</Label>
																{/if}
															</div>
														{/snippet}
													</Command.Item>
												{/each}
											</Grid>
										</section>
									{/snippet}
								</Command.Group>
							{/each}
							{@render children?.()}
						</div>
					{/snippet}
				</Command.List>
			</div>
		{/snippet}
	</Command.Root>
{/if}
