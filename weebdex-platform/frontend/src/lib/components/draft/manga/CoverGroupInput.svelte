<script lang="ts">
	import { Locale, Utils } from "$lib";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import { Button } from "$lib/components/ui/button";
	import * as Combobox from "$lib/components/ui/combobox";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Input } from "$lib/components/ui/input";
	import { useLightbox } from "$lib/components/ui/lightbox";
	import { cn } from "$lib/Utils";
	import { Expand, Pin, PinOff, XIcon } from "@lucide/svelte";

	let {
		value = $bindable(),
		values = $bindable(),
		disabled
	}: {
		value: CoverDraft;
		values: CoverDraft[];
		disabled?: boolean;
	} = $props();

	const lightbox = useLightbox();
	const title = $derived(value.volume ? `Volume ${value.volume}` : "No Volume");
	$effect(() => {
		value.valid = value.volume ? Utils.isValidVolumeChapter(value.volume) : true;
	});
</script>

<div class="space-y-1">
	<div class="relative">
		<AspectRatio ratio={10 / 14}>
			{#snippet child({ props })}
				<a
					class="group relative block overflow-hidden rounded-md"
					href={value.url}
					onclick={e => {
						if (!e.ctrlKey) {
							e.preventDefault();
							e.stopPropagation();
							e.stopImmediatePropagation();

							lightbox.set(value.url);
							lightbox.open();
						}
					}}
					{...props}
				>
					<ImageLoader src={value.url} alt={title}>
						{#snippet child({ src })}
							<figure class="size-full">
								<img class="size-full object-cover object-center" {src} alt={title} />
								<div
									class="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100"
								>
									<Expand class="text-white/85" size="40" />
								</div>
								<div class="gradient pointer-events-none absolute inset-0"></div>
							</figure>
							<div class="pointer-events-none absolute inset-0 flex h-full flex-col pb-2">
								<div class="flex flex-auto"></div>
								<h3
									class="line-clamp-2 shrink-0 px-2 font-medium text-balance break-words text-white group-hover:line-clamp-4"
								>
									{title}
								</h3>
							</div>
						{/snippet}
					</ImageLoader>
				</a>
			{/snippet}
		</AspectRatio>
		<Button
			class="bg-input text-foreground hover:bg-destructive hover:text-destructive-foreground absolute top-0 right-0 -mt-2 -mr-2 size-5.5 rounded-full"
			size="icon-sm"
			type="button"
			onclick={() => {
				if (value.file) {
					if (value.thumbnail && value.thumbnail.startsWith("blob:")) {
						URL.revokeObjectURL(value.thumbnail);
					}

					const { main } = value;
					values.splice(values.indexOf(value), 1);
					if (main && values.length) {
						values[0].main = true;
					}
				} else {
					value.deleted = true;
					if (value.main) {
						const filtered = values.filter(e => !e.deleted);
						if (filtered.length) {
							filtered[0].main = true;
						}
					}
				}
			}}
			{disabled}
		>
			<XIcon class="size-3.5" strokeWidth="3" />
		</Button>
	</div>
	<div class="overflow-hidden rounded-md">
		<Combobox.Root type="single" bind:value={value.language} {disabled}>
			<Combobox.Trigger class="w-full rounded-none border-none">
				{#if value.language}
					<div class="flex w-0 flex-auto items-center gap-2">
						<span class="size-5 shrink-0">
							<ImageLoader
								class="size-5"
								src="/images/flags/{Locale.to3166(value.language)}.svg"
								alt={Locale.toLanguage(value.language)}
							/>
						</span>
						<span class="truncate">
							{Locale.toLanguage(value.language)}
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
		<div class="flex items-center border-t">
			<Input
				class={cn("truncate rounded-none border-none !ring-0 !ring-transparent", {
					"text-red-300 placeholder:text-red-300": !value.valid
				})}
				bind:value={value.volume}
				placeholder="No Volume"
				autocomplete="off"
				{disabled}
			/>
			<div class="border-l">
				<Button
					class={cn("rounded-none border-none", {
						"text-accent-foreground": value.main
					})}
					variant="outline"
					size="icon"
					onclick={() => {
						values.forEach(e => (e.main = false));
						value.main = true;
					}}
					disabled={disabled || !values.length}
				>
					{#if value.main}
						<Pin />
					{:else}
						<PinOff />
					{/if}
				</Button>
			</div>
		</div>
	</div>
</div>

<style lang="scss">
	.gradient {
		background-image: linear-gradient(180deg, transparent 50%, rgba(0, 0, 0, 0.5));
	}
	h3 {
		text-shadow:
			0 0 0.2rem rgba(0, 0, 0, 0.75),
			0 0 0.15rem rgba(0, 0, 0, 0.75),
			0 0 0.1rem rgba(0, 0, 0, 0.75);
	}
</style>
