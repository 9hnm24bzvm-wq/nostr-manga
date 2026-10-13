<script lang="ts">
	import { setLightbox } from "./context.svelte";

	const { children } = $props();
	const lightbox = setLightbox();
</script>

{@render children()}
