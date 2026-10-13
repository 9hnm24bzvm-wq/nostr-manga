<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
</script>

<svelte:head>
	<title>About - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="max-w-200 space-y-5">
	<header>
		<h2 class="text-2xl font-semibold">About</h2>
		<span class="text-foreground/85">Last Revised: February 10, 2026</span>
	</header>
	<div class="space-y-5 text-base">
		<div class="typography space-y-2">
			<p>
				WeebDex is an ad-free, scanlator-first online manga reader that delivers high-quality, zero-compression images.
			</p>
			<p>
				Our primary goal is to preserve <strong>manga scanlations</strong> and provide a platform where scanlators can share
				their work without worrying about DMCA takedowns. We strive to ensure these works don't disappear at the whim of large
				companies.
			</p>
			<p>
				We welcome all scanlators, regardless of country of origin or language. Scanlators are free to upload their work
				to WeebDex and retain full control over their releases. WeebDex generally does not police content or scanlators
				unless they violate our Terms of Service.
			</p>
			<p>
				The site is still in its early stages, and we are working hard to deliver a better experience for our users.
			</p>
			<p>
				We also provide OpenAPI specifications for our API, which you can find <a href="https://api.weebdex.org/docs"
					>here</a
				>.
			</p>
		</div>
	</div>
</section>
