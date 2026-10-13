<script lang="ts" module>
	const Languages: Record<string, string> = {
		en: "English",
		ja: "Japanese",
		"ja-ro": "Japanese (Romanized)",
		ko: "Korean",
		"ko-ro": "Korean (Romanized)",
		zh: "Chinese (Simplified)",
		"zh-hk": "Chinese (Traditional)",
		"zh-ro": "Chinese (Romanized)",

		af: "Afrikaans",
		sq: "Albanian",
		ar: "Arabic",
		az: "Azerbaijani",
		eu: "Basque",
		be: "Belarusian",
		bn: "Bengali",
		bg: "Bulgarian",
		my: "Burmese",
		ca: "Catalan",
		cv: "Chuvash",
		hr: "Croatian",
		cs: "Czech",
		da: "Danish",
		nl: "Dutch",
		eo: "Esperanto",
		et: "Estonian",
		tl: "Filipino",
		fi: "Finish",
		fr: "French",
		ka: "Georgian",
		de: "German",
		el: "Greek",
		he: "Hebrew",
		hi: "Hindi",
		hu: "Hungarian",
		id: "Indonesian",
		jv: "Javanese",
		ga: "Irish",
		it: "Italian",
		kk: "Kazakh",
		la: "Latin",
		lt: "Lithuanian",
		ms: "Malay",
		mn: "Mongolian",
		ne: "Nepali",
		no: "Norwegian",
		fa: "Persian",
		pl: "Polish",
		pt: "Portuguese",
		"pt-br": "Portuguese (Brazil)",
		ro: "Romanian",
		ru: "Russian",
		sr: "Serbian",
		sk: "Slovak",
		sl: "Slovenian",
		es: "Spanish",
		"es-la": "Spanish (LATAM)",
		sv: "Swedish",
		tam: "Tamil",
		te: "Telugu",
		th: "Thai",
		tr: "Turkish",
		uk: "Ukranian",
		ur: "Urdu",
		uz: "Uzbek",
		vi: "Vietnamese"
	};
</script>

<script lang="ts">
	import { Locale } from "$lib";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as Combobox from "$lib/components/ui/combobox";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Input } from "$lib/components/ui/input";
	import { cn } from "$lib/Utils";
	import { TrashIcon } from "@lucide/svelte";

	let {
		value = $bindable(),
		values = $bindable(),
		disabled
	}: {
		value: AltTitle;
		values: AltTitle[];
		disabled?: boolean;
	} = $props();

	$effect(() => {
		value.valid = !!value.language && !!value.title && value.title.length <= 1024;
	});
</script>

<div class="bg-input/30 flex items-center overflow-hidden rounded-md">
	<Combobox.Root type="single" bind:value={value.language} {disabled}>
		<Combobox.Trigger
			class={buttonVariants({
				class: "bg-input/25 hover:!bg-input/50 rounded-none !border-none",
				variant: "ghost"
			})}
		>
			<span class="relative h-4 w-6">
				{#if value.language in Languages}
					<ImageLoader class="-mt-1 size-6" src="/images/flags/{Locale.to3166(value.language)}.svg" />
				{:else}
					<ImageLoader class="-mt-1 size-6" src="/images/flags/unknown.svg" />
				{/if}
				{#if value.language.endsWith("-ro")}
					<span
						class="absolute right-0 bottom-0 flex items-center justify-center bg-black px-1 text-[8px] font-semibold text-white opacity-40"
					>
						RO
					</span>
				{/if}
			</span>
		</Combobox.Trigger>
		<Combobox.Content align="start">
			{#each Object.entries(Languages) as [key, value]}
				<Combobox.Item value={key}>
					<span class="size-5 shrink-0">
						<ImageLoader src="/images/flags/{Locale.to3166(key)}.svg" />
					</span>
					<span class="shrink-0">{value}</span>
				</Combobox.Item>
			{/each}
		</Combobox.Content>
	</Combobox.Root>
	<Input
		class={cn("truncate border-none !bg-transparent !ring-0 !ring-transparent", {
			"text-red-300 placeholder:text-red-300": !value.valid
		})}
		bind:value={value.title}
		autocomplete="off"
		{disabled}
	/>
	<Button
		class="rounded-none bg-transparent"
		variant="ghost"
		size="icon"
		onclick={() => {
			const idx = values.indexOf(value);
			if (idx > -1) {
				values.splice(idx, 1);
			}
		}}
		{disabled}
	>
		<TrashIcon />
	</Button>
</div>
