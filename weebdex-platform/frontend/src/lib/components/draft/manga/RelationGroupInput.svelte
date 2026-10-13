<script lang="ts">
	import { Constants } from "$lib";
	import MangaSearchEntryCard from "$lib/components/MangaSearchEntryCard.svelte";
	import Button from "$lib/components/ui/button/button.svelte";
	import * as Combobox from "$lib/components/ui/combobox";
	import { XIcon } from "@lucide/svelte";

	let {
		value = $bindable(),
		values = $bindable(),
		disabled
	}: {
		value: RelationDraft;
		values: RelationDraft[];
		disabled?: boolean;
	} = $props();
</script>

<div class="space-y-1">
	<div class="relative">
		<MangaSearchEntryCard manga={<Manga>(<any>value)} isGrid isEdit />
		<Button
			class="bg-input text-foreground hover:bg-destructive hover:text-destructive-foreground absolute top-0 right-0 -mt-2 -mr-2 size-5.5 rounded-full"
			size="icon-sm"
			type="button"
			onclick={() => {
				if (value.new) values.splice(values.indexOf(value), 1);
				else value.deleted = true;
			}}
			{disabled}
		>
			<XIcon class="size-3.5" strokeWidth="3" />
		</Button>
	</div>
	<Combobox.Root type="single" bind:value={value.type} {disabled}>
		<Combobox.Trigger class="w-full border-none">
			<span class="truncate">
				{#if value.type in Constants.Manga.RelationTypeText}
					{Constants.Manga.RelationTypeText[value.type]}
				{:else}
					Select Type
				{/if}
			</span>
		</Combobox.Trigger>
		<Combobox.Content>
			{#each Object.entries(Constants.Manga.RelationTypeText) as [key, value]}
				<Combobox.Item value={key}>
					{value}
				</Combobox.Item>
			{/each}
		</Combobox.Content>
	</Combobox.Root>
</div>
