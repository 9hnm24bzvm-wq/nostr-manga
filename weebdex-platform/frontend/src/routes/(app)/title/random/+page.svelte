<script lang="ts">
	import { goto } from "$app/navigation";
	import { Utils } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { onMount } from "svelte";

	const { data } = $props();
	onMount(() => {
		if (data.manga) {
			goto(`/title/${data.manga.id}${Utils.makeSlug(data.manga.title)}`);
		}
	});
</script>

<Empty.Root>
	<Spinner class="size-10" />
</Empty.Root>
