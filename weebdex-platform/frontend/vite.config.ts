import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { SvelteKitPWA } from "@vite-pwa/sveltekit";
import { defineConfig } from "vite";

import autoprefixer from "autoprefixer";

export default defineConfig({
	build: {
		minify: "terser"
	},
	css: {
		postcss: {
			plugins: [autoprefixer()]
		}
	},
	plugins: [
		tailwindcss(),
		sveltekit(),

		SvelteKitPWA({
			srcDir: "src",
			filename: "service-worker.ts",

			workbox: {
				maximumFileSizeToCacheInBytes: 5 * 1024 ** 2
			},
			manifest: {
				id: "weebdex",
				name: "WeebDex",
				short_name: "WeebDex",
				description:
					"WeebDex is an ad-free, scanlator-first online manga reader providing high-quality, zero-compression images.",

				start_url: "/?source=pwa",

				theme_color: "#161616",
				background_color: "#161616",
				icons: [
					{
						src: "/images/pwa-64x64.png",
						sizes: "64x64",
						type: "image/png"
					},
					{
						src: "/images/pwa-192x192.png",
						sizes: "192x192",
						type: "image/png"
					},
					{
						src: "/images/pwa-512x512.png",
						sizes: "512x512",
						type: "image/png",
						purpose: "any"
					},
					{
						src: "/images/maskable-icon-512x512.png",
						sizes: "512x512",
						type: "image/png",
						purpose: "maskable"
					}
				]
			}
		})
	]
});
