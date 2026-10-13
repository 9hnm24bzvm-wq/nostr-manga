<script lang="ts" module>
	const arrayFields = [
		"lang",
		"langx",
		"demographic",
		"status",
		"reading_status",
		"contentRating",
		"availableTranslatedLang",
		"tag",
		"tagx",
		"author",
		"artist"
	];
</script>

<script lang="ts">
	import { goto } from "$app/navigation";
	import { Constants, Locale } from "$lib";
	import type { SearchMangaRequest } from "$lib/API/Manga";
	import { buttonVariants } from "$lib/components/ui/button";
	import Button from "$lib/components/ui/button/button.svelte";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import * as Combobox from "$lib/components/ui/combobox";
	import * as Command from "$lib/components/ui/command";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { Label } from "$lib/components/ui/label";
	import * as Popover from "$lib/components/ui/popover";
	import * as RadioGroup from "$lib/components/ui/radio-group";
	import * as Select from "$lib/components/ui/select";
	import { populateTagList, prefs } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { ChevronDownIcon, SearchIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { onMount } from "svelte";
	import TagComboBox from "./TagComboBox.svelte";
	import ImageLoader from "./ui/ImageLoader.svelte";
	import ViewModeToggles from "./ViewModeToggles.svelte";

	let {
		feed = $bindable(),
		isShowFilter = $bindable()
	}: {
		feed: (MangaFeedResponse | undefined) | (MangaFollowFeedResponse | undefined);
		isShowFilter: boolean;
	} = $props();

	const isLibrary = window.location.pathname.startsWith("/library");
	const defaultSearch: SearchMangaRequest = $derived({
		reading_status: [],

		title: "",
		yearFrom: undefined,
		yearTo: undefined,

		lang: [],
		demographic: [],
		status: [],
		contentRating: [],

		hasChapters: false,
		availableTranslatedLang: [],

		tag: [],
		tmod: "0",
		tagx: [],
		txmod: "0",

		limit: Number($prefs.items_per_page),
		sort: isLibrary ? Constants.MangaFollow.SortBy.FollowedAt : Constants.Manga.SortBy.LastUploadedChapterAt,
		order: "desc"
	});
	let search = $state<SearchMangaRequest>(
		(() => {
			const init = { ...structuredClone($state.snapshot(defaultSearch)) };
			const searchParams = new URLSearchParams(window.location.search);
			searchParams.delete("page");

			if (!isLibrary && !searchParams.toString().length) {
				if ($prefs.show_safe) init.contentRating!.push(Constants.Manga.ContentRating.Safe);
				if ($prefs.show_suggestive) init.contentRating!.push(Constants.Manga.ContentRating.Suggestive);
				if ($prefs.show_erotic) init.contentRating!.push(Constants.Manga.ContentRating.Erotica);
				if ($prefs.show_hentai) init.contentRating!.push(Constants.Manga.ContentRating.Pornographic);
				init.lang = structuredClone($prefs.languages);
				init.tagx = structuredClone($prefs.tags);
			}

			const map: Record<string, any> = {};
			for (const [key, value] of searchParams.entries()) {
				if (arrayFields.includes(key)) (map[key] ??= []).push(value);
				else map[key] = value;
			}
			for (const [key, value] of Object.entries(map)) {
				init[key as keyof SearchMangaRequest] = value;
			}
			return init;
		})()
	);

	onMount(async () => {
		try {
			await populateTagList();
		} catch (e) {
			console.error(e);
		}
	});

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();

		const q = new URLSearchParams();
		for (const [key, value] of Object.entries(search)) {
			if (value === undefined || value === null || (typeof value === "string" && !value.length)) {
				continue;
			}

			if (Array.isArray(value)) {
				value.forEach(v => q.append(key, v));
			} else {
				q.set(key, value);
			}
		}

		if (!search.hasChapters) q.delete("hasChapters");
		if (search.tmod === "AND" || search.tmod === "0" || !search.tag?.length) q.delete("tmod");
		if (search.txmod === "OR" || search.txmod === "0" || !search.tagx?.length) q.delete("txmod");

		if (
			search.sort ===
			(isLibrary ? Constants.MangaFollow.SortBy.FollowedAt : Constants.Manga.SortBy.LastUploadedChapterAt)
		) {
			q.delete("sort");
		}
		if (search.order === "desc") q.delete("order");
		if (search.limit === Number($prefs.items_per_page)) q.delete("limit");

		const target = window.location.pathname + q.toString();
		if (target !== window.location.pathname + window.location.search) {
			feed = undefined;
			goto(target);
		}
	};

	const onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		search = structuredClone(defaultSearch);
		onSubmit();
	};
</script>

<form class="space-y-1" onsubmit={onSubmit} onreset={onReset}>
	<InputGroup.Root class="w-full border-none py-5 !ring-0 !ring-transparent">
		<InputGroup.Input class="truncate" placeholder="Search..." autocomplete="off" bind:value={search.title} />
		<InputGroup.Addon>
			<SearchIcon />
		</InputGroup.Addon>
		{#if feed?.total}
			<InputGroup.Addon align="inline-end">{feed.total.short()}</InputGroup.Addon>
		{/if}
		<InputGroup.Addon align="inline-end">
			<Button
				class="hover:!bg-input/50 hover:text-muted-foreground -mr-0.75"
				type="button"
				variant="ghost"
				size="icon"
				onclick={() => (isShowFilter = !isShowFilter)}
			>
				<ChevronDownIcon class={cn("size-6 transition-all", isShowFilter && "rotate-180")} />
			</Button>
		</InputGroup.Addon>
	</InputGroup.Root>
	{#if isShowFilter}
		<div class="flex flex-wrap gap-1">
			<InputGroup.Root class="flex w-0 min-w-50 flex-auto border-none !ring-0 !ring-transparent">
				<InputGroup.Input
					class="min-w-25 flex-auto"
					placeholder="Year From"
					type="number"
					min={1900}
					max={2100}
					autocomplete="off"
					bind:value={search.yearFrom}
				/>
				<InputGroup.Input
					class="!border-border/75 min-w-25 flex-auto border-l"
					placeholder="Year To"
					type="number"
					min={1900}
					max={2100}
					autocomplete="off"
					bind:value={search.yearTo}
				/>
			</InputGroup.Root>
			{#if isLibrary}
				<Select.Root type="multiple" bind:value={search.reading_status}>
					<Select.Trigger class="flex w-0 min-w-45 flex-auto border-none">
						{#if search.reading_status?.length}
							<span class="w-0 flex-auto truncate text-left">
								{search.reading_status.map(k => Constants.Manga.ReadingStatusText[k]).join(", ")}
							</span>
							<strong class="shrink-0 font-medium text-green-600">+{search.reading_status.length}</strong>
						{:else}
							Reading Status
						{/if}
					</Select.Trigger>
					<Select.Content>
						{#each Object.entries(Constants.Manga.ReadingStatusText) as [key, value]}
							<Select.Item value={key}>{value}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			{/if}
			<Select.Root type="multiple" bind:value={search.status}>
				<Select.Trigger class="flex w-0 min-w-45 flex-auto border-none">
					{#if search.status?.length}
						<span class="w-0 flex-auto truncate text-left">
							{search.status.map(k => Constants.Manga.PublicationStatusText[k]).join(", ")}
						</span>
						<strong class="shrink-0 font-medium text-green-600">+{search.status.length}</strong>
					{:else}
						Publication Status
					{/if}
				</Select.Trigger>
				<Select.Content>
					{#each Object.entries(Constants.Manga.PublicationStatusText) as [key, value]}
						<Select.Item value={key}>{value}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<Select.Root type="multiple" bind:value={search.contentRating}>
				<Select.Trigger class="flex w-0 min-w-45 flex-auto border-none">
					{#if search.contentRating?.length}
						<span class="w-0 flex-auto truncate text-left">
							{search.contentRating.map(k => Constants.Manga.ContentRatingText[k]).join(", ")}
						</span>
						<strong class="shrink-0 font-medium text-green-600">+{search.contentRating.length}</strong>
					{:else}
						Content Rating
					{/if}
				</Select.Trigger>
				<Select.Content>
					{#each Object.entries(Constants.Manga.ContentRatingText) as [key, value]}
						<Select.Item value={key}>{value}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<Select.Root type="multiple" bind:value={search.demographic}>
				<Select.Trigger class="flex w-0 min-w-45 flex-auto border-none">
					{#if search.demographic?.length}
						<span class="w-0 flex-auto truncate text-left">
							{search.demographic.map(k => Constants.Manga.DemographicText[k]).join(", ")}
						</span>
						<strong class="shrink-0 font-medium text-green-600">+{search.demographic.length}</strong>
					{:else}
						Demographic
					{/if}
				</Select.Trigger>
				<Select.Content>
					{#each Object.entries(Constants.Manga.DemographicText) as [key, value]}
						<Select.Item value={key}>{value}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<Select.Root type="single" bind:value={search.sort}>
				<Select.Trigger class="text-muted-foreground flex w-0 min-w-45 flex-auto border-none">
					<span class="truncated">
						Sort: {isLibrary
							? Constants.MangaFollow.SortByText[search.sort as Constants.MangaFollow.SortBy]
							: Constants.Manga.SortByText[search.sort as Constants.Manga.SortBy]}
					</span>
				</Select.Trigger>
				<Select.Content>
					{#each Object.entries(isLibrary ? Constants.MangaFollow.SortByText : Constants.Manga.SortByText) as [key, value]}
						<Select.Item value={key}>{value}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<Select.Root type="single" bind:value={search.order}>
				<Select.Trigger class="text-muted-foreground flex w-0 min-w-45 flex-auto border-none">
					<span class="truncated">
						Order:
						{#if search.order === "asc"}
							Ascending
						{:else}
							Descending
						{/if}
					</span>
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="asc">Ascending</Select.Item>
					<Select.Item value="desc">Descending</Select.Item>
				</Select.Content>
			</Select.Root>
		</div>
		<div class="flex flex-wrap gap-1">
			<Combobox.Root type="multiple" bind:value={search.availableTranslatedLang} disabled={!search.hasChapters}>
				<Combobox.Trigger class="flex w-0 min-w-50 flex-auto border-none">
					{#if search.availableTranslatedLang?.length}
						<span class="flex w-0 flex-auto flex-nowrap items-center gap-1 overflow-hidden text-left">
							{#if search.availableTranslatedLang.length > 1}
								{#each search.availableTranslatedLang as code}
									<ImageLoader
										class="size-5 shrink-0"
										src="/images/flags/{Locale.to3166(code)}.svg"
										alt={Locale.toLanguage(code)}
									/>
								{/each}
							{:else}
								<ImageLoader
									class="size-5 shrink-0"
									src="/images/flags/{Locale.to3166(search.availableTranslatedLang[0])}.svg"
									alt={Locale.toLanguage(search.availableTranslatedLang[0])}
								/>
								{Locale.toLanguage(search.availableTranslatedLang[0])}
							{/if}
						</span>
						<strong class="shrink-0 font-medium text-green-600">+{search.availableTranslatedLang.length}</strong>
					{:else}
						Translated Languages
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
			<Combobox.Root type="multiple" bind:value={search.lang}>
				<Combobox.Trigger class="flex w-0 min-w-50 flex-auto border-none">
					{#if search.lang?.length}
						<span class="flex w-0 flex-auto flex-nowrap items-center gap-1 overflow-hidden text-left">
							{#if search.lang.length > 1}
								{#each search.lang as code}
									<ImageLoader
										class="size-5 shrink-0"
										src="/images/flags/{Locale.to3166(code)}.svg"
										alt={Locale.toLanguage(code)}
									/>
								{/each}
							{:else}
								<ImageLoader
									class="size-5 shrink-0"
									src="/images/flags/{Locale.to3166(search.lang[0])}.svg"
									alt={Locale.toLanguage(search.lang[0])}
								/>
								{Locale.toLanguage(search.lang[0])}
							{/if}
						</span>
						<strong class="shrink-0 font-medium text-green-600">+{search.lang.length}</strong>
					{:else}
						Original Languages
					{/if}
				</Combobox.Trigger>
				<Combobox.Content side="bottom">
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
			<Popover.Root>
				<Popover.Trigger
					class={buttonVariants({
						variant: "outline",
						class:
							"!text-muted-foreground flex w-0 min-w-45 flex-auto items-center items-center justify-start gap-2 !border-none text-left"
					})}
				>
					<div class="flex w-0 flex-auto items-center gap-2">
						<span class="w-0 flex-auto">Tags</span>
						{#if search.tag?.length}
							<strong class="shrink-0 font-medium text-green-600">+{search.tag.length}</strong>
						{/if}
						{#if search.tagx?.length}
							<strong class="text-destructive shrink-0 font-medium">-{search.tagx.length}</strong>
						{/if}
					</div>
					<ChevronDownIcon class="shrink-0 opacity-50" />
				</Popover.Trigger>
				<Popover.Content
					class="w-dvw max-w-[min(var(--bits-floating-available-width),50rem)] overflow-y-auto p-0"
					collisionBoundary={document.getElementById("search")}
				>
					<TagComboBox class="px-3 pb-3" bind:value={search.tag!} bind:indeterminate={search.tagx!}>
						<Command.Group>
							{#snippet child({ props: { class: className, ...restProps } })}
								<section {...restProps}>
									<header class="mb-2">
										<strong class="font-medium">Options</strong>
									</header>
									<div class="text-foreground/85 flex flex-wrap items-center gap-2">
										<Command.Item>
											{#snippet child({ props: { class: className, ...restProps } })}
												<div class="flex flex-auto items-center gap-2" {...restProps}>
													<strong class="font-medium">Inclusion Mode:</strong>
													<RadioGroup.Root class="flex items-center" bind:value={search.tmod}>
														<div class="group flex items-center">
															<RadioGroup.Item class="shrink-0" value="0" id="inc_and" />
															<Label
																class={cn(
																	"group-hover:text-foreground flex-auto cursor-pointer pl-2 font-normal whitespace-nowrap",
																	{
																		"text-foreground": search.tmod === "0"
																	}
																)}
																for="inc_and">AND</Label
															>
														</div>
														<div class="group flex items-center">
															<RadioGroup.Item class="shrink-0" value="1" id="inc_or" />
															<Label
																class={cn(
																	"group-hover:text-foreground flex-auto cursor-pointer pl-2 font-normal whitespace-nowrap",
																	{
																		"text-foreground": search.tmod === "1"
																	}
																)}
																for="inc_or">OR</Label
															>
														</div>
													</RadioGroup.Root>
												</div>
											{/snippet}
										</Command.Item>
										<Command.Item>
											{#snippet child({ props: { class: className, ...restProps } })}
												<div class="flex flex-auto items-center gap-2" {...restProps}>
													<strong class="font-medium">Exclusion Mode:</strong>
													<RadioGroup.Root class="flex items-center" bind:value={search.txmod}>
														<div class="group flex items-center">
															<RadioGroup.Item class="shrink-0" value="0" id="exc_or" />
															<Label
																class={cn(
																	"group-hover:text-foreground flex-auto cursor-pointer pl-2 font-normal whitespace-nowrap",
																	{
																		"text-foreground": search.txmod === "0"
																	}
																)}
																for="exc_or">OR</Label
															>
														</div>
														<div class="group flex items-center">
															<RadioGroup.Item class="shrink-0" value="1" id="exc_and" />
															<Label
																class={cn(
																	"group-hover:text-foreground flex-auto cursor-pointer pl-2 font-normal whitespace-nowrap",
																	{
																		"text-foreground": search.txmod === "1"
																	}
																)}
																for="exc_and">AND</Label
															>
														</div>
													</RadioGroup.Root>
												</div>
											{/snippet}
										</Command.Item>
									</div>
								</section>
							{/snippet}
						</Command.Group>
					</TagComboBox>
				</Popover.Content>
			</Popover.Root>
			<Select.Root
				type="single"
				value={search.limit.toString()}
				onValueChange={value => {
					search.limit = Number(value);
				}}
			>
				<Select.Trigger class="text-muted-foreground flex w-0 min-w-45 flex-auto border-none">
					Items Per Page: {search.limit}
				</Select.Trigger>
				<Select.Content class="max-h-120">
					{#each [28, 42, 56, 70, 84, 98] as value}
						<Select.Item value={value.toString()}>
							{value}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
		<div class="mt-3 flex items-start">
			<Checkbox id="hasChapters" bind:checked={search.hasChapters} />
			<Label class="cursor-pointer pl-2" for="hasChapters">Has Chapters</Label>
		</div>
	{/if}
	<div class="mt-5 flex items-center gap-2">
		<ViewModeToggles class="w-0 flex-auto" />
		<div class="flex shrink-0 items-center gap-1">
			<Button
				class="!text-destructive border-none"
				variant="outline"
				disabled={equal(search, defaultSearch)}
				type="reset"
			>
				Reset
			</Button>
			<Button variant="default" type="submit">
				<SearchIcon />
				Search
			</Button>
		</div>
	</div>
</form>
