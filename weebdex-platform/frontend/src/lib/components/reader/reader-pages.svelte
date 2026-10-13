<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { onMount, tick } from "svelte";
	import { useReader } from "./context.svelte";
	import ReaderPagesDouble from "./reader-pages-double.svelte";
	import ReaderPagesSingle from "./reader-pages-single.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;
	let isMounted = $state(false);

	onMount(async () => {
		await tick();
		isMounted = true;
	});

	$effect(() => {
		if (isMounted && reader.currentPage) {
			const q = new URLSearchParams(window.location.search);
			const path = `/chapter/${reader.chapter.id}/${reader.currentPage.index + 1}${q.toString()}${window.location.hash}`;
			tick().then(() => {
				replaceState(path, {
					...page.state,
					page: reader.currentPage!.index + 1
				});
			});
		}
	});
</script>

{#if $readerPrefs.double}
	{#each reader.spreads as spread (spread)}
		{#if spread.length === 2}
			<ReaderPagesDouble {spread} />
		{:else}
			<ReaderPagesSingle bind:page={reader.pages[reader.pages.indexOf(spread[0])]} />
		{/if}
	{/each}
{:else}
	{#each reader.pages as page, i (page)}
		<ReaderPagesSingle bind:page={reader.pages[i]} />
	{/each}
{/if}
