<script lang="ts" module>
	let timeout = 0;
</script>

<script lang="ts">
	import { Constants, Utils } from "$lib";
	import API from "$lib/API";
	import { getManga } from "$lib/Context";
	import { onDestroy, onMount } from "svelte";

	const manga = getManga()();
	onMount(async () => {
		if (manga.state !== Constants.State.Published) {
			return;
		}

		if (!sessionStorage.getItem(manga.id) && !Utils.isBot()) {
			try {
				timeout = window.setTimeout(() => {
					if (!sessionStorage.getItem(manga.id)) {
						API.Analytics.Manga(manga.id);
						sessionStorage.setItem(manga.id, "1");
					}
				}, 1000);
			} catch (e) {
				console.error(e);
			}
		}
	});
	onDestroy(() => window.clearTimeout(timeout));
</script>
