<script lang="ts">
	import { cn } from "$lib/Utils";
	import { onDestroy, onMount } from "svelte";
	import type { HTMLAttributes } from "svelte/elements";
	import Swiper from "swiper";
	import { Autoplay } from "swiper/modules";

	const {
		class: className,
		children,
		slidesPerView,
		spaceBetween = 0,
		breakpoints
	}: {
		class?: string;
		slidesPerView?: number;
		spaceBetween?: number;
		breakpoints?: Record<number, { slidesPerView: number }>;
	} & HTMLAttributes<HTMLDivElement> = $props();
	let ref = $state<HTMLElement>();
	let swiper = $state<Swiper>();

	onMount(() => {
		if (!ref) {
			return;
		}

		swiper = new Swiper(ref, {
			slidesPerView: slidesPerView ?? 1,
			spaceBetween,
			breakpoints,

			loop: true,
			autoplay: {
				delay: 10000
			},

			modules: [Autoplay]
		});
	});

	onDestroy(() => {
		swiper?.destroy();
	});
</script>

<div class={cn("swiper -m-1 overflow-hidden p-1 select-none", className)} bind:this={ref}>
	<div class="swiper-wrapper">
		{@render children?.()}
	</div>
</div>
