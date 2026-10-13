<script lang="ts">
	import { isMobile } from "$lib/stores";
	import { useReader } from "./context.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;
</script>

{#if reader.pages.length && !isMobile.current}
	<div
		class="pointer-events-none fixed inset-y-0 z-[100] flex flex-col gap-[1px]"
		class:left-0={$readerPrefs.panel_position === "right"}
		class:right-0={$readerPrefs.panel_position === "left"}
	>
		{#each reader.pages as page}
			<span
				class="relative h-0 flex-auto border-l-4 text-xs opacity-50 transition"
				class:border-gray-500={!page.isLoaded && !page.isLoading && !page.isFailed}
				class:border-green-700={page.isLoaded}
				class:border-sky-700={page.isLoading}
				class:border-red-700={page.isFailed}
				class:!border-green-400={page === reader.currentPage}
			>
			</span>
		{/each}
	</div>
{/if}
