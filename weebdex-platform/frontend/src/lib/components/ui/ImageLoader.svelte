<script lang="ts">
	import { onMount, type Snippet } from "svelte";
	import type { HTMLImgAttributes } from "svelte/elements";

	let {
		src,
		child,
		loader,
		onLoad,
		...restProps
	}: HTMLImgAttributes & {
		src: string;
		child?: Snippet<[{ src: string; width: number; height: number }]>;
		loader?: Snippet;
		onLoad?: () => void;
	} = $props();

	let isLoaded = $state<boolean>();
	let width = $state<number>(0);
	let height = $state<number>(0);

	onMount(() => {
		load();
	});

	let count = 0;
	const load = () => {
		if (isLoaded) {
			return;
		}

		const img = new Image();
		img.addEventListener("load", () => {
			isLoaded = true;
			width = img.width;
			height = img.height;

			if (onLoad) {
				window.requestAnimationFrame(() => {
					onLoad();
				});
			}
		});
		img.addEventListener("error", () => {
			if (count > 3) {
				return;
			}

			count++;
			setTimeout(load, 3000);
		});
		img.src = src;
	};
</script>

{#if child}
	{#if isLoaded}
		{@render child({ src, width, height })}
	{:else if loader}
		{@render loader?.()}
	{/if}
{:else if isLoaded}
	<img
		{src}
		{width}
		{height}
		alt={restProps.title ?? restProps.alt}
		title={restProps.title ?? restProps.alt}
		{...restProps}
	/>
{:else if loader}
	{@render loader?.()}
{/if}
