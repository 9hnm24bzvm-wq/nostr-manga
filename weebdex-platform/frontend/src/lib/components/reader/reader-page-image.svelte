<script lang="ts" module>
	const placeholder = (w: number, h: number) =>
		`data:image/svg+xml;base64,${btoa(`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"></svg>`)}`;
</script>

<script lang="ts">
	import { Constants } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import { RefreshCw, TriangleAlert } from "@lucide/svelte";
	import { onMount } from "svelte";
	import { Button } from "../ui/button";
	import Spinner from "../ui/spinner/spinner.svelte";
	import { useReader } from "./context.svelte";

	let {
		page = $bindable(),
		isDouble
	}: {
		page: ReaderPage;
		isDouble?: boolean;
	} = $props();

	const reader = useReader();
	const readerPrefs = reader.prefs;

	const { ref } = $derived(page);
	const [width, height] = $derived(page.data.dimensions);

	onMount(async () => {
		if (page === reader.currentPage) {
			reader.scrollIntoView(page);
		}
	});

	const retry = (ev: MouseEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		reader.pages.forEach(p => (p.isFailed = false));
		reader.pages = reader.pages;
		reader.key++;
	};
</script>

<div class="group relative" bind:this={$ref}>
	<!-- svelte-ignore a11y_missing_attribute -->
	<img
		class="object-contain"
		class:opacity-0={page.isLoading || page.isFailed}
		class:group-first:object-right={isDouble}
		class:group-last:object-left={isDouble}
		class:w-auto={[Constants.Reader.Scale.LimitHeight, Constants.Reader.Scale.StretchHeight].includes(
			$readerPrefs.scale
		)}
		class:h-auto={[Constants.Reader.Scale.LimitWidth, Constants.Reader.Scale.StretchWidth].includes($readerPrefs.scale)}
		class:w-full={[Constants.Reader.Scale.StretchAll, Constants.Reader.Scale.StretchWidth].includes($readerPrefs.scale)}
		class:h-dvh={[Constants.Reader.Scale.StretchAll, Constants.Reader.Scale.StretchHeight].includes($readerPrefs.scale)}
		class:max-w-full={[Constants.Reader.Scale.LimitAll, Constants.Reader.Scale.LimitWidth].includes($readerPrefs.scale)}
		class:max-h-dvh={[Constants.Reader.Scale.LimitAll, Constants.Reader.Scale.LimitHeight].includes($readerPrefs.scale)}
		class:max-w-[unset]={[
			Constants.Reader.Scale.Original,
			Constants.Reader.Scale.LimitHeight,
			Constants.Reader.Scale.StretchHeight
		].includes($readerPrefs.scale)}
		class:max-h-[unset]={[
			Constants.Reader.Scale.Original,
			Constants.Reader.Scale.LimitWidth,
			Constants.Reader.Scale.StretchWidth
		].includes($readerPrefs.scale)}
		class:group-first:ml-auto={isDouble}
		class:group-last:mr-auto={isDouble}
		src={page.blob ?? placeholder(width, height)}
		style="aspect-ratio: {width}/{height}"
		{width}
		{height}
	/>

	{#if !page.isLoaded}
		<Empty.Root class="absolute inset-0">
			{#if page.isFailed}
				<Empty.Header>
					<Empty.Media>
						<TriangleAlert class="size-16" strokeWidth="1" />
					</Empty.Media>
					<Empty.Title class="text-sm tracking-wider uppercase">Failed to load page {page.index + 1}</Empty.Title>
				</Empty.Header>
				<Empty.Content>
					<Button class="w-30 border-none tracking-wider uppercase" type="button" onclick={retry}>
						<RefreshCw />
						Retry
					</Button>
				</Empty.Content>
			{:else}
				<Spinner class="size-6" />
			{/if}
		</Empty.Root>
	{/if}
</div>
