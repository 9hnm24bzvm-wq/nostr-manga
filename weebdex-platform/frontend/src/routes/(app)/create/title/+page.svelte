<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import MangaCreate from "$lib/components/draft/manga/MangaCreate.svelte";
</script>

<svelte:head>
	<title>Create Title Draft - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="mx-auto flex w-full max-w-270 flex-auto flex-col space-y-5">
	<header class="space-y-5">
		<h2 class="text-xl font-medium">Create Title Draft</h2>
	</header>
	<MangaCreate />
</section>
