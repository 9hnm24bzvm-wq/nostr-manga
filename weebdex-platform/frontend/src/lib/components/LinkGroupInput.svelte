<script lang="ts">
	import { ExternalLink, type AuthorLink, type GroupLink, type UserLink } from "$lib";
	import { cn } from "$lib/Utils";
	import { CircleQuestionMark, Globe, Mail, Star, TrashIcon } from "@lucide/svelte";
	import ImageLoader from "./ui/ImageLoader.svelte";
	import { Button, buttonVariants } from "./ui/button";
	import * as Combobox from "./ui/combobox";
	import { Input } from "./ui/input";

	let {
		value = $bindable(),
		values = $bindable(),
		module,
		disabled
	}: {
		value: ExternalLink;
		values: ExternalLink[];
		module: typeof ExternalLink | typeof AuthorLink | typeof GroupLink | typeof UserLink;
		disabled?: boolean;
	} = $props();

	const codes = $derived(
		Object.entries(module.names).filter(([code]) => !values.some(v => v.code === code && code !== value.code))
	);
	$effect(() => {
		if (value.link) {
			const detected = module.getCodeFromUrl(value.link);
			if (detected && detected !== value.code && !values.some(v => v.code === detected)) {
				value.code = detected;
			}
		}
		value.valid =
			!!value.code && !!value.link && value.link.length <= 2048 && !!module.validate(value.code, value.link);
	});
</script>

<div class="bg-input/30 flex items-center overflow-hidden rounded-md">
	<Combobox.Root type="single" bind:value={value.code} disabled={disabled || !codes.length}>
		<Combobox.Trigger
			class={buttonVariants({
				class: "bg-input/25 hover:!bg-input/50 rounded-none !border-none",
				variant: "ghost"
			})}
		>
			{#key value.code}
				{#if value.code in module.baseUrls}
					<span class="size-4">
						<ImageLoader
							class="rounded-full"
							src="https://www.google.com/s2/favicons?sz=14&domain_url={ExternalLink.base(module.baseUrls[value.code])}"
						/>
					</span>
				{:else if value.code === "contact_email"}
					<Mail class="size-4" />
				{:else if value.code === "website"}
					<Globe class="size-4" />
				{:else if value.code === "raw"}
					<span class="font-semibold">RAW</span>
				{:else if value.code == "engtl"}
					<span class="font-semibold">ENTL</span>
				{:else}
					<CircleQuestionMark class="size-4" />
				{/if}
			{/key}
		</Combobox.Trigger>
		<Combobox.Content align="start">
			{#each codes as [code, name]}
				<Combobox.Item value={code}>
					<span class="size-4 shrink-0">
						{#if code in module.baseUrls}
							<ImageLoader
								class="rounded-full"
								src="https://www.google.com/s2/favicons?sz=14&domain_url={ExternalLink.base(module.baseUrls[code])}"
							/>
						{:else if code === "contact_email"}
							<Mail />
						{:else if code === "website"}
							<Globe />
						{:else}
							<Star />
						{/if}
					</span>
					<span class="shrink-0">{name}</span>
				</Combobox.Item>
			{/each}
		</Combobox.Content>
	</Combobox.Root>
	<Input
		class={cn("truncate border-none !bg-transparent !ring-0 !ring-transparent", {
			"text-red-300 placeholder:text-red-300": !value.valid
		})}
		bind:value={value.link}
		placeholder={(module as any).placeholders?.[value.code] ?? ""}
		autocomplete="off"
		disabled={disabled || !codes.length}
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
