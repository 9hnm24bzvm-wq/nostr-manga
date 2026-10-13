<script lang="ts">
	import { afterNavigate } from "$app/navigation";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import ReportDialog from "$lib/components/ReportDialog.svelte";
	import * as Lightbox from "$lib/components/ui/lightbox";
	import * as Sidebar from "$lib/components/ui/sidebar";
	import { Toaster } from "$lib/components/ui/sonner";
	import { from } from "$lib/stores";
	import { onMount } from "svelte";
	import "swiper/css/bundle";
	import { pwaInfo } from "virtual:pwa-info";
	import "./layout.css";

	const { children } = $props();
	afterNavigate(({ from: f }) => from.set(f?.url));

	const webManifestLink = $derived(pwaInfo ? pwaInfo.webManifest.linkTag : "");
	onMount(async () => {
		if (pwaInfo) {
			const { registerSW } = await import("virtual:pwa-register");
			await registerSW({
				immediate: true,
				onRegistered(r: any) {},
				onRegisterError(error: any) {}
			})();
		}
	});
</script>

<svelte:head>
	<title>{PUBLIC_SITE_NAME}</title>
	{@html webManifestLink}
	<!-- Google tag (gtag.js) -->
	<script async src="https://www.googletagmanager.com/gtag/js?id=G-W464K5049G"></script>
	<script>
		window.dataLayer = window.dataLayer || [];
		function gtag() {
			dataLayer.push(arguments);
		}
		gtag("js", new Date());
		gtag("config", "G-W464K5049G");
	</script>
</svelte:head>

<Lightbox.Provider>
	<Sidebar.Provider>
		{@render children?.()}
	</Sidebar.Provider>

	<Toaster closeButton={true} richColors />
	<ReportDialog />

	<Lightbox.Lightbox />
</Lightbox.Provider>
