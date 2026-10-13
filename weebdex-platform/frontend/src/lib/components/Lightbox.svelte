<script lang="ts" module>
	let isOpen = $state<boolean>(false);
	let urls = $state<string[]>([]);
	let currentIndex = $state<number>(0);

	export const set = (...value: string[]) => {
		urls = value;
		currentIndex = 0;
	};
	export const setIndex = (index: number) => {
		currentIndex = index;
	};

	export const open = (index?: number) => {
		if (!urls.length) {
			return;
		}

		isOpen = true;
		if (index) {
			currentIndex = index;
		}
	};
	export const close = () => {
		isOpen = false;
		urls = [];
		currentIndex = 0;
	};

	export const prev = (e?: MouseEvent) => {
		e?.preventDefault();
		e?.stopPropagation();
		e?.stopImmediatePropagation();
		currentIndex = Math.max(0, currentIndex - 1);
	};
	export const next = (e?: MouseEvent) => {
		e?.preventDefault();
		e?.stopPropagation();
		e?.stopImmediatePropagation();
		currentIndex = Math.min(urls.length - 1, currentIndex + 1);
	};

	window._lightbox_ = {
		open,
		close,
		prev,
		next
	};
</script>

<script lang="ts">
	import { ChevronLeft, ChevronRight } from "@lucide/svelte";
	import { Dialog } from "bits-ui";
	import ImageLoader from "./ui/ImageLoader.svelte";
	import Spinner from "./ui/spinner/spinner.svelte";

	let ref = $state<HTMLImageElement>();
	let width = $derived(0);
	let height = $derived(0);

	const assign = () => {
		width = ref?.clientWidth ?? 0;
		height = ref?.clientHeight ?? 0;
	};
	$effect(assign);
</script>

<svelte:window onresize={assign} />

<Dialog.Root bind:open={isOpen}>
	<Dialog.Portal>
		<Dialog.Overlay class="fixed inset-0 z-50 bg-black/50" />
		<Dialog.Content class="fixed inset-0 z-50 p-5 outline-none" onclick={close}>
			<div class="relative flex size-full items-center justify-center select-none">
				{#key currentIndex}
					<ImageLoader src={urls[currentIndex]}>
						{#snippet child({ src })}
							<!-- svelte-ignore a11y_missing_attribute -->
							<img
								class="max-h-full max-w-full object-contain text-[0] text-transparent"
								{src}
								onload={assign}
								bind:this={ref}
							/>
							{#if width && height && urls.length > 1}
								<div class="absolute inset-0 m-auto flex" style="width: {width}px; height: {height}px">
									{#if currentIndex > 0}
										<button
											class="prev flex-auto text-white opacity-0 transition hover:opacity-100 focus-visible:opacity-100"
											type="button"
											onclick={prev}
										>
											<ChevronLeft size="48" />
										</button>
									{:else}
										<div class="flex-auto"></div>
									{/if}
									{#if currentIndex < urls.length - 1}
										<button
											class="next flex-auto text-white opacity-0 transition hover:opacity-100 focus-visible:opacity-100"
											type="button"
											onclick={next}
										>
											<ChevronRight class="float-right" size="48" />
										</button>
									{:else}
										<div class="flex-auto"></div>
									{/if}
								</div>
							{/if}
						{/snippet}
						{#snippet loader()}
							<Spinner class="size-10" />
						{/snippet}
					</ImageLoader>
				{/key}
			</div>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style lang="scss">
	.prev {
		background-image: linear-gradient(to right, rgba(#000, 0.5), rgba(#000, 0));
	}

	.next {
		background-image: linear-gradient(to left, rgba(#000, 0.5), rgba(#000, 0));
	}
</style>
