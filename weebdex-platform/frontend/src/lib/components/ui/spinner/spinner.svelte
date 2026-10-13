<script lang="ts">
	import { cn } from "$lib/Utils";
	import { Loader2Icon } from "@lucide/svelte";
	import type { ComponentProps } from "svelte";

	let { class: className, ...restProps }: ComponentProps<typeof Loader2Icon> = $props();
</script>

<Loader2Icon role="status" aria-label="Loading" class={cn("size-4 animate-spin", className)} {...restProps} />
