<script lang="ts">
	import { cn } from "$lib/Utils";
	import Button, { type ButtonProps } from "./ui/button/button.svelte";

	let {
		class: className,
		links = $bindable(),
		children,
		...restProps
	}: {
		links: ExternalLink[];
	} & ButtonProps = $props();
</script>

<Button
	class={cn("w-full border border-dashed !bg-transparent", className)}
	variant="ghost"
	{...restProps}
	onclick={() => {
		links.push({ code: "", link: "", valid: false });
	}}
>
	{@render children?.()}
</Button>
