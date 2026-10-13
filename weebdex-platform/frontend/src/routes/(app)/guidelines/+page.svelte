<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import Guidelines from "$lib/components/Guidelines.svelte";
</script>

<svelte:head>
	<title>Guidelines - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="space-y-5">
	<header>
		<h2 class="text-2xl font-semibold">Guidelines</h2>
		<span class="text-foreground/85">Last Revised: January 11, 2026</span>
	</header>
	<div class="space-y-5 text-base">
		<Guidelines />
	</div>
</section>
