<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { Locale } from "$lib";
	import type { ListChaptersRequest } from "$lib/API/Chapter";
	import { Button } from "$lib/components/ui/button";
	import * as Combobox from "$lib/components/ui/combobox";
	import { getManga } from "$lib/Context";
	import { prefs } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { ArrowDownWideNarrow, ArrowUpNarrowWide } from "@lucide/svelte";
	import equal from "fast-deep-equal";

	let {
		feed = $bindable()
	}: {
		feed: ChapterFeedResponse | undefined;
	} = $props();

	const manga = $derived(getManga()());
	const defaultSearch: ListChaptersRequest = {
		tlang: [],
		group: [],
		order: "desc"
	};
	const initialSearch = {
		tlang: page.url.searchParams.getAll("tlang") ?? [],
		group: page.url.searchParams.getAll("group") ?? [],
		order: page.url.searchParams.get("order") ?? "desc"
	};
	let search = $state<ListChaptersRequest>(structuredClone(initialSearch));

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();

		const q = new URLSearchParams();
		if (search.tlang?.length) search.tlang.forEach(lang => q.append("tlang", lang));
		if (search.group?.length) search.group.forEach(group => q.append("group", group));
		if (search.order === "asc") q.set("order", "asc");

		const target = window.location.pathname + q.toString();
		if (target !== window.location.pathname + window.location.search) {
			if (feed) {
				feed.data = undefined;
			}
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

<form class="flex flex-wrap items-center gap-1" onsubmit={onSubmit} onreset={onReset}>
	{#if feed?.total}
		<Button
			class="shrink-0 border-none"
			type="button"
			variant="outline"
			size="icon"
			onclick={() => {
				search.order = search.order === "asc" ? "desc" : "asc";
				onSubmit();
			}}
		>
			<ArrowDownWideNarrow class={cn("size-5 scale-0 transition", search.order === "desc" && "scale-100")} />
			<ArrowUpNarrowWide class={cn("absolute size-5 scale-0 transition", search.order === "asc" && "scale-100")} />
		</Button>
	{/if}

	{#if manga.relationships.available_languages}
		<Combobox.Root type="multiple" bind:value={search.tlang}>
			<Combobox.Trigger class="!w-0 min-w-38 flex-auto shrink-0 border-none">
				<span class="truncate">
					{#if !search.tlang?.length && $prefs.translated_languages.length}
						[Forced]
						{$prefs.translated_languages.map(Locale.toLanguage).join(", ")}
					{:else if search.tlang?.length}
						{search.tlang.map(Locale.toLanguage).join(", ")}
					{:else}
						Any Language
					{/if}
				</span>
			</Combobox.Trigger>
			<Combobox.Content>
				{#each manga.relationships.available_languages as code}
					<Combobox.Item value={code}>
						<img class="size-4" src="/images/flags/{Locale.to3166(code)}.svg" alt={code} />
						{Locale.toLanguage(code)}
					</Combobox.Item>
				{/each}
			</Combobox.Content>
		</Combobox.Root>
	{/if}
	{#if manga.relationships.available_groups}
		<Combobox.Root type="multiple" bind:value={search.group}>
			<Combobox.Trigger class="!w-0 min-w-38 flex-auto shrink-0 border-none">
				<span class="truncate">
					{#if !search.group?.length && $prefs.groups.length}
						[Forced] Applied group blacklist
					{:else if search.group?.length}
						{search.group.map(id => manga.relationships.available_groups?.find(g => g.id === id)?.name).join(", ")}
					{:else}
						Any Group
					{/if}
				</span>
			</Combobox.Trigger>
			<Combobox.Content>
				{#each manga.relationships.available_groups as group}
					<Combobox.Item value={group.id}>
						{group.name}
					</Combobox.Item>
				{/each}
			</Combobox.Content>
		</Combobox.Root>
	{/if}
	<div class="flex flex-wrap items-center gap-1">
		<Button
			class="!text-destructive shrink-0 border-none"
			type="reset"
			variant="outline"
			disabled={equal(search, defaultSearch)}>Reset</Button
		>
		<Button class="shrink-0" type="submit" disabled={equal(search, initialSearch)}>Apply</Button>
	</div>
</form>
