<script lang="ts">
	import { Locale } from "$lib";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import * as Combobox from "$lib/components/ui/combobox";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Input } from "$lib/components/ui/input";
	import { Label } from "$lib/components/ui/label";
	import { cn } from "$lib/Utils";

	let {
		draft = $bindable(),
		isOneshot = $bindable(),
		isVolumeValid,
		isChapterValid,
		disabled
	}: {
		draft: ChapterDraft;
		isOneshot: boolean;
		isVolumeValid: boolean;
		isChapterValid: boolean;
		disabled?: boolean;
	} = $props();
</script>

<div class="space-y-4">
	<div class="space-y-2">
		<Label for="name">Name</Label>
		<Input
			class="border-none !ring-0 !ring-transparent"
			id="name"
			type="text"
			bind:value={draft.title}
			placeholder="Chapter Name"
			{disabled}
		/>
	</div>
	<div class="flex flex-wrap gap-x-2 gap-y-5">
		<div class="w-0 min-w-35 flex-auto space-y-2">
			<Label for="volume">Volume</Label>
			<Input
				class={cn("border-none !ring-0 !ring-transparent", {
					"text-red-300 placeholder:text-red-300": !isVolumeValid
				})}
				id="volume"
				type="text"
				bind:value={draft.volume}
				placeholder="Volume No."
				disabled={disabled || isOneshot}
			/>
		</div>
		<div class="w-0 min-w-35 flex-auto space-y-2">
			<Label for="chapter" class="block">
				Chapter{#if !isOneshot}<span class="text-red-500">*</span>{/if}
			</Label>
			<Input
				class={cn("border-none !ring-0 !ring-transparent", {
					"text-red-300 placeholder:text-red-300": !isChapterValid
				})}
				id="chapter"
				type="text"
				bind:value={draft.chapter}
				placeholder="Chapter No."
				required={!isOneshot}
				disabled={disabled || isOneshot}
			/>
		</div>
		<div class="w-0 min-w-50 flex-auto space-y-2">
			<Label class="block">
				Language<span class="text-red-500">*</span>
			</Label>
			<Combobox.Root type="single" bind:value={draft.language} {disabled}>
				<Combobox.Trigger class="w-full border-none">
					{#if draft.language}
						<div class="flex w-0 flex-auto items-center gap-2">
							<span class="size-5 shrink-0">
								<ImageLoader
									class="size-5"
									src="/images/flags/{Locale.to3166(draft.language)}.svg"
									alt={Locale.toLanguage(draft.language)}
								/>
							</span>
							<span class="truncate">
								{Locale.toLanguage(draft.language)}
							</span>
						</div>
					{:else}
						<span class="truncate">Select Language</span>
					{/if}
				</Combobox.Trigger>
				<Combobox.Content>
					{#each Object.entries(Locale.Languages) as [key, value]}
						<Combobox.Item value={key}>
							<span class="size-5 shrink-0">
								<ImageLoader class="size-5" src="/images/flags/{Locale.to3166(key)}.svg" alt={value} />
							</span>
							{value}
						</Combobox.Item>
					{/each}
				</Combobox.Content>
			</Combobox.Root>
		</div>
	</div>
	<div class="flex items-center">
		<Checkbox
			id="oneshot"
			bind:checked={isOneshot}
			onCheckedChange={() => {
				draft.volume = "";
				draft.chapter = "";
			}}
			{disabled}
		/>
		<Label class="pl-2" for="oneshot">This is a oneshot</Label>
	</div>
</div>
