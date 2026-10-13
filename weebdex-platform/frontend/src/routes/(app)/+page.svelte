<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import LatestUpdatesSection from "$lib/components/LatestUpdatesSection.svelte";
	import MostReadCarousel from "$lib/components/MostReadCarousel.svelte";
	import MyUpdatesCarousel from "$lib/components/MyUpdatesCarousel.svelte";
	import RecentlyAddedCarousel from "$lib/components/RecentlyAddedCarousel.svelte";
	import { Session } from "$lib/stores";
	import announcements from "./announcements/announcement.svelte";
	import AnnouncementEntry from "./announcements/AnnouncementEntry.svelte";
</script>

<svelte:head>
	<title>{PUBLIC_SITE_NAME} - High quality images. No ads</title>
</svelte:head>

<div class="space-y-5">
	{#if announcements.some(e => e.pinned)}
		<div class="space-y-1">
			{#each announcements as announcement}
				{#if announcement.pinned}
					<AnnouncementEntry {announcement} />
				{/if}
			{/each}
		</div>
	{/if}

	<MostReadCarousel />

	<div class="rounded-md border border-dashed border-green-500/50 bg-green-500/15 p-3 text-center text-green-300">
		<p>
			The site is still in its early stages. Please report any bugs or issues you encounter.
			<br />
			Reports of duplicates or incorrect information would also be appreciated.
		</p>
	</div>

	{#if $Session}
		<MyUpdatesCarousel />
	{/if}
	<LatestUpdatesSection />
	<RecentlyAddedCarousel />
</div>
