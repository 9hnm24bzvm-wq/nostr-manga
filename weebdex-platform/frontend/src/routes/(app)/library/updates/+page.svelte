<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API } from "$lib";
	import ChapterFeed from "$lib/components/ChapterFeed.svelte";
	import { prefs } from "$lib/stores";
</script>

<svelte:head>
	<title>My Updates - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="flex flex-auto flex-col space-y-1">
	<header>
		<h2 class="text-xl font-medium">My Updates</h2>
	</header>
	<ChapterFeed
		onSearch={(n?: number) => {
			const q = new URLSearchParams();
			prefs.applyMangaPreferences(q);
			prefs.applyChapterPreferences(q);

			if (n) q.set("page", n.toString());
			q.set("limit", "100");

			return API.MangaFollow.LatestFeed(q);
		}}
	/>
</section>
