import { createSixPersonaOverlays } from '../platform-integrations/six-persona/overlays.mjs';
import { createAccountLibraryOverlays } from '../platform-integrations/local-library/account-overlays.mjs';
import { createBoundedReaderOverlays } from '../platform-integrations/bounded-reader/overlays.mjs';
// Exact path repair only. The original frontend configuration stays unchanged.
import path from "node:path";
import { createUploadRefusalQaPlugin } from "./public-upload-refusal-qa.mjs";
import { readPublicApiGate } from "../platform-integrations/public-api/overlays.mjs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createPublicApiOverlays } from "../platform-integrations/public-api/overlays.mjs";
import { createPublicUploadOverlays, createNativeUploadOverlays } from "../platform-integrations/public-api/upload-overlays.mjs";
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
  resolve: { dedupe: ["svelte"], alias: [{ find: /^@lucide\/svelte$/, replacement: path.join(frontend, "node_modules/@lucide/svelte/dist/lucide-svelte.js") }, ...Object.entries({ "$staffWorkspace": path.join(root, "platform-integrations/staff-workspace"), "$announcements": reconstruction, "$boundedReader": path.join(root, "platform-integrations/bounded-reader"), "$platform": path.join(root, "platform-integrations/frontend-behavior"), "$localLibrary": path.join(root, "platform-integrations/local-library"), "$publicApi": path.join(root, "platform-integrations/public-api"), "svelte-sonner": path.join(frontend, "node_modules/svelte-sonner/dist/index.js"), "axios": path.join(frontend, "node_modules/axios/index.js"), "http-status-codes": path.join(frontend, "node_modules/http-status-codes/build/es/index.js"), "nostr-tools/nip44": path.join(root, ".build/remote-signer-nostr-tools-2.25.2/node_modules/nostr-tools/lib/esm/nip44.js"), "@noble/ciphers": path.join(root, ".build/remote-signer-nostr-tools-2.25.2/node_modules/@noble/ciphers"), "@noble/curves": path.join(root, ".build/local-library-noble-2.0.1/node_modules/@noble/curves"), "@noble/hashes": path.join(root, ".build/local-library-noble-2.0.1/node_modules/@noble/hashes"), "@scure/base": path.join(root, ".build/local-signing-base-2.0.0/node_modules/@scure/base") }).map(([find, replacement]) => ({ find, replacement }))] },
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
  merged.plugins.unshift(createUploadRefusalQaPlugin(root, { publicUploadEnabled: readPublicApiGate() }), createIntegrationUtilitySources(frontend, path.join(root, "platform-integrations")), createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend), createPublicUploadOverlays(frontend), createNativeUploadOverlays(frontend), createBoundedReaderOverlays(frontend), createAccountLibraryOverlays(frontend), createSixPersonaOverlays(frontend), createLocalLibraryResolver(frontend, path.join(root, "platform-integrations/local-library")));
  // Explicit bounded QA resource profile; default production options are unchanged.
  const qaProfile = process.env.WEEBDEX_QA_BUILD_RESOURCE_PROFILE;
  if (qaProfile !== undefined) {
    if (qaProfile !== "serial-terser") throw Error("Unknown QA build resource profile");
    merged.build = { ...merged.build, reportCompressedSize: false, terserOptions: { ...merged.build?.terserOptions, maxWorkers: 1 } };
  }
  return merged;
};
