/// <reference lib="webworker" />
import { CacheableResponsePlugin } from "workbox-cacheable-response";
import { clientsClaim } from "workbox-core";
import { ExpirationPlugin } from "workbox-expiration";
import { registerRoute } from "workbox-routing";
import { CacheFirst, StaleWhileRevalidate } from "workbox-strategies";

declare let self: ServiceWorkerGlobalScope;
self.skipWaiting();
clientsClaim();

registerRoute(
	({ request, url }) =>
		url.pathname.startsWith("/assets/") ||
		url.pathname.startsWith("/_app/") ||
		(request.url.startsWith("/fonts/") && request.destination === "font") ||
		(request.url.startsWith("/images/") && request.destination === "image"),
	new CacheFirst({
		cacheName: "assets",
		plugins: [new CacheableResponsePlugin({ statuses: [200] })]
	})
);

registerRoute(
	({ request }) => request.url.includes("/favicons") || request.url.includes("faviconV2"),
	new StaleWhileRevalidate({
		cacheName: "favicons",
		plugins: [
			new CacheableResponsePlugin({ statuses: [200] }),
			new ExpirationPlugin({
				maxEntries: 1000,
				maxAgeSeconds: 3600 * 24 * 30
			})
		]
	}),
	"GET"
);

registerRoute(
	({ request }) => request.url.includes("/covers/") && request.destination === "image",
	new CacheFirst({
		cacheName: "covers",
		plugins: [
			new CacheableResponsePlugin({ statuses: [200] }),
			new ExpirationPlugin({
				maxEntries: 1000,
				maxAgeSeconds: 3600 * 24 * 7
			}),
			{
				cacheWillUpdate: async ({ response }) => {
					try {
						if (response?.status !== 200) {
							return null;
						}

						const blob = await response.clone().blob();
						if (blob.size < 1024) {
							return null;
						}
					} catch {
						return null;
					}
					return response;
				}
			}
		]
	}),
	"GET"
);
