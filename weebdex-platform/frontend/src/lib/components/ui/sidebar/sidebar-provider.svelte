<script lang="ts">
	import { setSidebar } from "./context.svelte.js";

	let {
		open = $bindable(true),
		onOpenChange = () => {},
		children
	}: {
		open?: boolean;
		onOpenChange?: (open: boolean) => void;
		children?: any;
	} = $props();

	const sidebar = setSidebar({
		open: () => open,
		setOpen: (value: boolean) => {
			open = value;
			onOpenChange(value);
		}
	});
</script>

<svelte:window onkeydown={sidebar.handleShortcutKeydown} />

{@render children?.()}
