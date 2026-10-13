<script lang="ts" module>
	let timeout = 0;
</script>

<script lang="ts">
	import { Utils } from "$lib";
	import API from "$lib/API";
	import { onDestroy, onMount } from "svelte";
	import { useReader } from "./context.svelte";

	const { chapter } = useReader();
	onMount(async () => {
		if (!chapter.published_at || chapter.deleted_at || chapter.is_unavailable || !chapter.data?.length) {
			return;
		}

		if (!sessionStorage.getItem(chapter.id) && !Utils.isBot()) {
			try {
				timeout = window.setTimeout(() => {
					if (!sessionStorage.getItem(chapter.id)) {
						API.Analytics.Chapter(chapter.id);
						sessionStorage.setItem(chapter.id, "1");
					}
				}, 1000);
			} catch (e) {
				console.error(e);
			}
		}
	});
	onDestroy(() => window.clearTimeout(timeout));
</script>
