<script lang="ts">
	import { goto } from "$app/navigation";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as Combobox from "$lib/components/ui/combobox";
	import { cn } from "$lib/Utils";
	import { GitCompare, Users } from "@lucide/svelte";
	import { useReader } from "./context.svelte";

	const {
		class: className
	}: {
		class?: string;
	} = $props();

	const reader = useReader();
	const entries = $derived.by(() => {
		if (
			reader.chapterIndex === undefined ||
			reader.chapterIndex < 0 ||
			reader.chapterIndex >= Number(reader.aggregated?.chapters.length)
		) {
			return;
		}

		const aggregate = reader.aggregated?.chapters[reader.chapterIndex];
		if (!aggregate) {
			return;
		}
		return Object.entries(aggregate.entries);
	});

	const changeChapter = (id: string) => {
		if (reader.chapter.id !== id) {
			goto(`/chapter/${id}/1`);
		}
	};
</script>

{#if Number(entries?.length) > 1}
	<Combobox.Root type="single" value={reader.chapter.id} onValueChange={value => changeChapter(value)}>
		<Combobox.Trigger
			class={buttonVariants({ class: cn("text-accent-foreground border-none", className), variant: "default" })}
		>
			<GitCompare class="text-inherit" />
			Change Group
		</Combobox.Trigger>
		{#if reader.aggregated}
			<Combobox.Content>
				{#each entries as [id, entry]}
					<Combobox.Item value={id}>
						<div class="w-0 flex-auto">
							{#each entry.groups as idx}
								<div class="flex items-center gap-2">
									<Users class="shrink-0" />
									<span class="w-0 flex-auto truncate">
										{reader.aggregated.groups[idx].name}
									</span>
								</div>
							{/each}
						</div>
					</Combobox.Item>
				{/each}
			</Combobox.Content>
		{/if}
	</Combobox.Root>
{/if}
