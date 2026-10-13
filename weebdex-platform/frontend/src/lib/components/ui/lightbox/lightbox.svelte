<script lang="ts">
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { ChevronLeft, ChevronRight } from "@lucide/svelte";
	import { Dialog } from "bits-ui";
	import { useLightbox } from "./context.svelte";

	const lightbox = useLightbox();
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

<Dialog.Root bind:open={lightbox.visible}>
	<Dialog.Portal>
		<Dialog.Overlay class="fixed inset-0 z-50 bg-black/50" />
		<Dialog.Content class="fixed inset-0 z-50 p-5 outline-none" onclick={lightbox.close}>
			<div class="relative flex size-full items-center justify-center select-none">
				{#key lightbox.index}
					<ImageLoader src={lightbox.urls[lightbox.index]}>
						{#snippet child({ src })}
							<!-- svelte-ignore a11y_missing_attribute -->
							<img
								class="max-h-full max-w-full object-contain text-[0] text-transparent"
								{src}
								onload={assign}
								bind:this={ref}
							/>
							{#if width && height && lightbox.urls.length > 1}
								<div class="absolute inset-0 m-auto flex" style="width: {width}px; height: {height}px">
									{#if lightbox.index > 0}
										<button
											class="prev flex-auto text-white opacity-0 transition hover:opacity-100 focus-visible:opacity-100"
											type="button"
											onclick={lightbox.prev}
										>
											<ChevronLeft size="48" />
										</button>
									{:else}
										<div class="flex-auto"></div>
									{/if}
									{#if lightbox.index < lightbox.urls.length - 1}
										<button
											class="next flex-auto text-white opacity-0 transition hover:opacity-100 focus-visible:opacity-100"
											type="button"
											onclick={lightbox.next}
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
