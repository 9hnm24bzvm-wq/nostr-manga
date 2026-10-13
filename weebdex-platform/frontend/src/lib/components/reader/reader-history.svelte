<script lang="ts">
	import API from "$lib/API";
	import { Session } from "$lib/stores";
	import { onMount } from "svelte";
	import { useReader } from "./context.svelte";

	const { chapter } = useReader();
	onMount(async () => {
		if (chapter && (await Session.isValid())) {
			API.ChapterHistory.UpdateBatch(chapter.relationships.manga.id, {
				read_ids: [chapter.id]
			});
		}
	});
</script>
