<script lang="ts">
	import { goto } from "$app/navigation";
	import { onMount } from "svelte";

	onMount(() => {
		if (window.location.pathname !== "/settings/preferences") {
			goto("/settings/preferences", { replaceState: true });
		}
	});
</script>
