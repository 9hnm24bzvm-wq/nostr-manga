<script lang="ts">
	import { cn } from "$lib/Utils";
	import { Button, type ButtonProps } from "$lib/components/ui/button";

	let {
		class: className,
		titles = $bindable(),
		children,
		...restProps
	}: {
		titles: AltTitle[];
	} & ButtonProps = $props();
</script>

<Button
	class={cn("w-full border border-dashed !bg-transparent", className)}
	variant="ghost"
	{...restProps}
	onclick={() => {
		titles.push({ language: "", title: "", valid: false });
	}}
>
	{@render children?.()}
</Button>
