<script lang="ts">
	import { API, Constants, Locale } from "$lib";
	import AuthorCreateDialog from "$lib/components/draft/manga/AuthorCreateDialog.svelte";
	import EntitySearchDialog from "$lib/components/EntitySearchDialog.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Combobox from "$lib/components/ui/combobox";
	import * as Dialog from "$lib/components/ui/dialog";
	import Grid from "$lib/components/ui/Grid.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import * as InputGroup from "$lib/components/ui/input-group";
	import Input from "$lib/components/ui/input/input.svelte";
	import Label from "$lib/components/ui/label/label.svelte";
	import * as Select from "$lib/components/ui/select";
	import { Separator } from "$lib/components/ui/separator";
	import { cn } from "$lib/Utils";
	import { Plus, PlusIcon, SearchIcon, Trash } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";

	let {
		draft = $bindable(),
		isCompletedOrCancelled,
		isVolumeValid,
		isChapterValid,
		disabled
	}: {
		draft: MangaDraft;
		isCompletedOrCancelled: boolean;
		isVolumeValid: boolean;
		isChapterValid: boolean;
		disabled: boolean;
	} = $props();

	let isCreateAuthor = $state(false);
	let isCreateArtist = $state(false);

	const onSearch = async (query: string) => {
		const q = new URLSearchParams();
		q.set("name", query);
		q.set("limit", "100");

		const res = await API.Author.Search(q);
		if (res?.status === StatusCodes.OK && res.body) {
			return res.body;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

{#snippet entitySearchTrigger()}
	<Dialog.Trigger type="button" {disabled}>
		{#snippet child({ props: { class: _, ...props } })}
			<InputGroup.Root
				class={cn("hover:!bg-input/50 cursor-pointer border-none !ring-0", {
					"!cursor-not-allowed opacity-50": disabled
				})}
				{...props}
			>
				<InputGroup.Text class="px-2">Search...</InputGroup.Text>
				<InputGroup.Addon>
					<SearchIcon />
				</InputGroup.Addon>
			</InputGroup.Root>
		{/snippet}
	</Dialog.Trigger>
{/snippet}

{#snippet authorButton(author: Author)}
	{@const selectedIdx = draft!.authors.findIndex(e => e.id === author.id)}
	<Button
		class={cn("justify-start border-none", {
			"!text-accent-foreground": selectedIdx >= 0
		})}
		type="button"
		variant="outline"
		title={author.name}
		onclick={() => {
			if (selectedIdx >= 0) draft!.authors.splice(selectedIdx, 1);
			else draft!.authors.push(author);
		}}
		{disabled}
	>
		<span class="flex-auto truncate text-left">
			{author.name}
		</span>
		{#if selectedIdx >= 0}
			<Trash class="shrink-0" />
		{:else}
			<Plus class="shrink-0" />
		{/if}
	</Button>
{/snippet}

{#snippet artistButton(artist: Author)}
	{@const selectedIdx = draft!.artists.findIndex(e => e.id === artist.id)}
	<Button
		class={cn("justify-start border-none", {
			"!text-accent-foreground": selectedIdx >= 0
		})}
		type="button"
		variant="outline"
		title={artist.name}
		onclick={() => {
			if (selectedIdx >= 0) draft!.artists.splice(selectedIdx, 1);
			else draft!.artists.push(artist);
		}}
		{disabled}
	>
		<span class="flex-auto truncate text-left">
			{artist.name}
		</span>
		{#if selectedIdx >= 0}
			<Trash class="shrink-0" />
		{:else}
			<Plus class="shrink-0" />
		{/if}
	</Button>
{/snippet}

<div class="flex flex-wrap gap-x-2 gap-y-5">
	<div class="w-0 min-w-100 flex-auto space-y-2">
		<Label class="block" for="authors">Authors<span class="text-red-500">*</span></Label>
		<Dialog.Root>
			{@render entitySearchTrigger()}
			<Dialog.Content
				class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
				showCloseButton={false}
			>
				<EntitySearchDialog {onSearch}>
					{#snippet children({ data }: { data: Author[] })}
						<Grid class="mt-1 gap-1" breakpoints={{ 0: 2, 525: 1 }}>
							{#each data as author}
								{@render authorButton(author)}
							{/each}
						</Grid>
					{/snippet}
				</EntitySearchDialog>
				<div class="p-2 pt-0">
					<Button
						class="w-full border border-dashed border-green-500/50 bg-green-500/15 !p-3 text-green-300 hover:!bg-green-500/20"
						variant="ghost"
						type="button"
						onclick={() => (isCreateAuthor = !isCreateAuthor)}
					>
						<PlusIcon />
						Create
					</Button>
				</div>
			</Dialog.Content>
		</Dialog.Root>
		<Dialog.Root bind:open={isCreateAuthor}>
			<Dialog.Content
				class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(30rem,calc(100dvw-2rem))] flex-col overflow-auto"
			>
				<AuthorCreateDialog title="Author" bind:value={draft.authors} bind:open={isCreateAuthor} />
			</Dialog.Content>
		</Dialog.Root>
		<div class="flex flex-wrap items-center gap-1">
			{#if draft.authors.length}
				{#each draft.authors as author}
					<div class="flex">
						{@render authorButton(author)}
					</div>
				{/each}
			{:else}
				<p class="text-muted-foreground">No authors selected</p>
			{/if}
		</div>
	</div>
	<div class="w-0 min-w-100 flex-auto space-y-2">
		<Label class="block" for="artists">Artists<span class="text-red-500">*</span></Label>
		<Dialog.Root>
			{@render entitySearchTrigger()}
			<Dialog.Content
				class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
				showCloseButton={false}
			>
				<EntitySearchDialog {onSearch}>
					{#snippet children({ data }: { data: Author[] })}
						<Grid class="mt-1 gap-1" breakpoints={{ 0: 2, 525: 1 }}>
							{#each data as artist}
								{@render artistButton(artist)}
							{/each}
						</Grid>
					{/snippet}
				</EntitySearchDialog>
				<div class="p-2 pt-0">
					<Button
						class="w-full border border-dashed border-green-500/50 bg-green-500/15 !p-3 text-green-300 hover:!bg-green-500/20"
						variant="ghost"
						type="button"
						onclick={() => (isCreateArtist = !isCreateArtist)}
					>
						<PlusIcon />
						Create
					</Button>
				</div>
			</Dialog.Content>
		</Dialog.Root>
		<Dialog.Root bind:open={isCreateArtist}>
			<Dialog.Content
				class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(30rem,calc(100dvw-2rem))] flex-col overflow-auto"
			>
				<AuthorCreateDialog title="Artist" bind:value={draft.artists} bind:open={isCreateArtist} />
			</Dialog.Content>
		</Dialog.Root>
		<div class="flex flex-wrap items-center gap-1">
			{#if draft.artists.length}
				{#each draft.artists as artist}
					<div class="flex">
						{@render artistButton(artist)}
					</div>
				{/each}
			{:else}
				<p class="text-muted-foreground">No artists selected</p>
			{/if}
		</div>
	</div>
</div>
<Separator />
<div class="flex flex-wrap gap-x-2 gap-y-5">
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label class="block" for="language">Original Language<span class="text-red-500">*</span></Label>
		<Combobox.Root
			type="single"
			bind:value={draft.language}
			onValueChange={() => {
				draft.demographic = null!;
			}}
			{disabled}
		>
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
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label class="block" for="contentRating">Content Rating<span class="text-red-500">*</span></Label>
		<Select.Root type="single" bind:value={draft.content_rating} {disabled}>
			<Select.Trigger class="w-full border-none">
				<span class="truncate">
					{#if draft.content_rating in Constants.Manga.ContentRatingText}
						{Constants.Manga.ContentRatingText[draft.content_rating]}
					{:else}
						Select Content Rating
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries(Constants.Manga.ContentRatingText) as [key, value]}
					<Select.Item value={key}>
						{value}
					</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label class="block" for="status">Publication Status<span class="text-red-500">*</span></Label>
		<Select.Root
			type="single"
			bind:value={draft.status}
			onValueChange={value => {
				if (
					![Constants.Manga.PublicationStatus.Completed, Constants.Manga.PublicationStatus.Cancelled].includes(
						<Constants.Manga.PublicationStatus>value
					)
				) {
					draft.last_volume = "";
					draft.last_chapter = "";
				}
			}}
			{disabled}
		>
			<Select.Trigger class="w-full border-none">
				<span class="truncate">
					{#if draft.status in Constants.Manga.PublicationStatusText}
						{Constants.Manga.PublicationStatusText[draft.status]}
					{:else}
						Select Publication Status
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries(Constants.Manga.PublicationStatusText) as [key, value]}
					<Select.Item value={key}>
						{value}
					</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>
</div>
<div class="flex flex-wrap gap-x-2 gap-y-5">
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label for="finalVolume">Final Volume</Label>
		<Input
			class={cn("truncate border-none !ring-0 !ring-transparent", {
				"text-red-300 placeholder:text-red-300": !isVolumeValid
			})}
			id="finalVolume"
			bind:value={draft.last_volume}
			placeholder="Volume No."
			autocomplete="off"
			disabled={disabled || !isCompletedOrCancelled}
		/>
	</div>
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label for="finalChapter">Final Chapter</Label>
		<Input
			class={cn("truncate border-none !ring-0 !ring-transparent", {
				"text-red-300 placeholder:text-red-300": !isChapterValid
			})}
			id="finalChapter"
			bind:value={draft.last_chapter}
			placeholder="Chapter No."
			autocomplete="off"
			disabled={disabled || !isCompletedOrCancelled}
		/>
	</div>
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label for="demographic">Magazine Demographic</Label>
		<Select.Root
			type="single"
			bind:value={draft.demographic}
			disabled={disabled || draft.language !== "ja"}
			allowDeselect
		>
			<Select.Trigger class="w-full border-none">
				<span class="truncate">
					{#if draft.demographic && draft.demographic in Constants.Manga.DemographicText}
						{Constants.Manga.DemographicText[draft.demographic]}
					{:else}
						Select Demographic
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries( { ...Constants.Manga.DemographicText, [Constants.Manga.Demographic.None]: undefined } ) as [key, value]}
					{#if value}
						<Select.Item value={key}>
							{value}
						</Select.Item>
					{/if}
				{/each}
			</Select.Content>
		</Select.Root>
	</div>
	<div class="w-0 min-w-50 flex-auto space-y-2">
		<Label for="year">Publication Year</Label>
		<Input
			class="truncate border-none !ring-0 !ring-transparent"
			id="year"
			type="number"
			bind:value={draft.year}
			placeholder="Publication Year"
			autocomplete="off"
			min="1900"
			max="2100"
			{disabled}
		/>
	</div>
</div>
