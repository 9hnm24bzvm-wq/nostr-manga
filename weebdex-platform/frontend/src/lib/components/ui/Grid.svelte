<script lang="ts">
	import { cn } from "$lib/Utils";
	import type { Snippet } from "svelte";

	const {
		class: className,
		breakpoints = $bindable(),
		children
	}: {
		class?: string;
		breakpoints: Record<number, number>;
		children: Snippet<[{ columns: number }]>;
	} = $props();

	const calcColumns = () => {
		for (const k in breakpoints) {
			const w = Number(k);
			const v = breakpoints[w];

			if (window.innerWidth <= w) {
				return v;
			}
		}
		return breakpoints[0];
	};
	let columns = $derived<number>(calcColumns());
</script>

<svelte:window
	on:resize={() => {
		const newColumns = calcColumns();
		if (columns !== newColumns) {
			columns = newColumns;
		}
	}}
/>

<div class={cn("grid grid-cols-[repeat(var(--columns),minmax(0,1fr))]", className)} style="--columns: {columns}">
	{@render children?.({ columns })}
</div>
