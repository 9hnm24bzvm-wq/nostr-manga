<script lang="ts">
	import { Constants } from "$lib";
	import { onDestroy, onMount, tick } from "svelte";
	import { useReader } from "./context.svelte";
	import ReaderPageImage from "./reader-page-image.svelte";

	const { spread }: { spread: ReaderPage[] } = $props();
	const page = $derived(spread[0]);

	const reader = useReader();
	const readerPrefs = reader.prefs;

	let isMounted = $state(false);
	let ref = $state<HTMLDivElement>();
	let observer: IntersectionObserver | undefined;

	onMount(async () => {
		await tick();

		if ($readerPrefs.layout === Constants.Reader.Layout.Scroll) {
			attachObserver();
			window.addEventListener("resize", attachObserver);
		}
		isMounted = true;
	});

	onDestroy(() => {
		observer?.disconnect();
		window.removeEventListener("resize", attachObserver);
	});

	const attachObserver = async () => {
		observer?.disconnect();
		if (!ref || $readerPrefs.layout !== Constants.Reader.Layout.Scroll) {
			return;
		}

		observer = new IntersectionObserver(
			async ([entry]) => {
				if (isMounted && entry.isIntersecting && reader.scroll.scrolling && page !== reader.currentPage) {
					if (
						(reader.scroll.direction === "top" && page.index > Number(reader.currentPage?.index)) ||
						(reader.scroll.direction === "bottom" && page.index < Number(reader.currentPage?.index))
					) {
						return;
					}
					reader.mutex.exec(() => (reader.currentPage = page));
				}
			},
			{
				threshold: ref.getBoundingClientRect().height > window.innerHeight ? 0.4 : 0.65
			}
		);
		observer.observe(ref);
	};

	let prevScale = $readerPrefs.scale;
	let prevLayout = $readerPrefs.layout;
	let prevDirection = $readerPrefs.direction;

	$effect(() => {
		if ($readerPrefs.scale !== prevScale || $readerPrefs.layout !== prevLayout) {
			prevScale = $readerPrefs.scale;
			prevLayout = $readerPrefs.layout;
			prevDirection = $readerPrefs.direction;
			tick().then(() => attachObserver());
		}
	});
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="flex flex-auto"
	class:hidden={reader.currentPage !== page && $readerPrefs.layout !== Constants.Reader.Layout.Scroll}
	onclick={reader.listener}
	bind:this={ref}
>
	<div
		class="m-auto grid w-full flex-auto grid-cols-[repeat(2,1fr)] overflow-auto"
		style={[Constants.Reader.Scale.LimitWidth, Constants.Reader.Scale.StretchWidth].includes($readerPrefs.scale)
			? `max-width: ${$readerPrefs.width}%`
			: undefined}
	>
		{#each $readerPrefs.direction === Constants.Reader.Direction.RTL ? spread.toReversed() : spread as page (page)}
			<ReaderPageImage bind:page={reader.pages[reader.pages.indexOf(page)]} isDouble />
		{/each}
	</div>
</div>
