// Exact path repair only. The original frontend configuration stays unchanged.
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createIntegrationUtilitySources } from "../platform-integrations/integration-utility-sources.mjs";
import { createReaderOverlays } from "../platform-integrations/frontend-reader-overlays.mjs";
import { createFrontendOverlays } from "../platform-integrations/frontend-overlays.mjs";
import { createLocalLibraryOverlays, createLocalLibraryResolver } from "../platform-integrations/local-library/overlays.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const frontend = process.env.WEEBDEX_FRONTEND_ROOT ?? path.join(root, "frontend");
const lib = path.join(frontend, "src/lib");
const storesBarrel = path.join(lib, "stores/index.ts");
const home = path.join(frontend, "src/routes/(app)/+page.svelte");
const reconstruction = path.join(root, "platform-integrations/frontend-reconstruction/announcements");
export default async configEnv => {
  const { mergeConfig, loadConfigFromFile } = await import(pathToFileURL(path.join(frontend, "node_modules/vite/dist/node/index.js")));
  const original = await loadConfigFromFile(configEnv, path.join(frontend, "vite.config.ts"), frontend);
  if (!original) throw new Error("Original attachment Vite configuration could not be loaded");
  const merged = mergeConfig(original.config, {
  // Bound build resources without changing the preserved application minifier.
  build: { terserOptions: { maxWorkers: 1 }, reportCompressedSize: false },
  resolve: { dedupe: ["svelte"], alias: { "$announcements": reconstruction, "$platform": path.join(root, "platform-integrations/frontend-behavior"), "$localLibrary": path.join(root, "platform-integrations/local-library"), "svelte-sonner": path.join(frontend, "node_modules/svelte-sonner/dist/index.js"), "axios": path.join(frontend, "node_modules/axios/index.js"), "http-status-codes": path.join(frontend, "node_modules/http-status-codes/build/es/index.js"), "@noble/curves": path.join(root, ".build/local-library-noble-2.0.1/node_modules/@noble/curves"), "@noble/hashes": path.join(root, ".build/local-library-noble-2.0.1/node_modules/@noble/hashes"), "@scure/base": path.join(root, ".build/local-signing-base-2.0.0/node_modules/@scure/base") } },
  plugins: [{
    name: "attachment-exact-import-case-repair",
    enforce: "pre",
    resolveId(source, importer) {
      if (source === "$lib/utils.js" || source === path.join(lib, "utils.js")) return path.join(lib, "Utils.ts");
      if (source === "./session" && importer?.split("?")[0] === storesBarrel) {
        return path.join(lib, "stores/Session.ts");
      }
      if (importer?.split("?")[0] === home) {
        if (source === "./announcements/announcement.svelte") return path.join(reconstruction, "announcement.svelte.ts");
        if (source === "./announcements/AnnouncementEntry.svelte") return path.join(reconstruction, "AnnouncementEntry.svelte");
      }
      if (source === "$announcements/announcement.svelte" || source === path.join(reconstruction, "announcement.svelte")) return path.join(reconstruction, "announcement.svelte.ts");
      return null;
    },
  }],
  });
  // The immutable-source script repair must run before Svelte compilation.
  merged.plugins.unshift(createIntegrationUtilitySources(frontend, path.join(root, "platform-integrations")), createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createLocalLibraryResolver(frontend, path.join(root, "platform-integrations/local-library")));
  return merged;
};
