<script lang="ts">
	import { onDestroy, onMount } from "svelte";
	import { useReader } from "./context.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;

	let timeout = 0;
	let lastY = 0;
	let isTop = false;
	let isBottom = false;
	let distance = 0;

	onMount(() => window.requestAnimationFrame(onScroll));
	onDestroy(() => window.clearTimeout(timeout));

	const onWheel = (ev: WheelEvent) => {
		if (reader.scroll.locked || !$readerPrefs.scroll_navigation) {
			return;
		}

		if (isTop && ev.deltaY <= 0) {
			distance += ev.deltaY;
			if (distance < -500 && isTop) {
				distance = 0;
				reader.gotoPreviousPage();
				window.requestAnimationFrame(onScroll);
			}
		} else if (isBottom && ev.deltaY > 0) {
			distance += ev.deltaY;
			if (distance > 500 && isBottom) {
				distance = 0;
				reader.gotoNextPage();
				window.requestAnimationFrame(onScroll);
			}
		}
	};

	const onScroll = () => {
		window.clearTimeout(timeout);
		distance = 0;

		reader.scroll.scrolling = true;
		reader.scroll.direction = window.scrollY > lastY ? "bottom" : "top";

		lastY = window.scrollY;
		timeout = window.setTimeout(() => (reader.scroll.scrolling = false), 100);

		const scrollTop = window.scrollY || document.documentElement.scrollTop;
		isTop = scrollTop <= 2;
		isBottom = scrollTop + window.innerHeight >= document.documentElement.scrollHeight - 1;
	};
</script>

<svelte:window onscroll={onScroll} onwheel={onWheel} />
