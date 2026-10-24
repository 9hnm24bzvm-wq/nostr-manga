// NEW component-fixture build configuration. Source only until separate admission.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { sveltekit } from '@sveltejs/kit/vite';
const fixture = path.dirname(fileURLToPath(import.meta.url));
const required = name => { const value = process.env[name]; if (!value || !path.isAbsolute(value)) throw Error(`Absolute ${name} required`); return fs.realpathSync(value); };
const platform = required('WEEBDEX_BADGE_PLATFORM_ROOT');
const frontend = required('WEEBDEX_BADGE_FRONTEND_ROOT');
const packages = required('WEEBDEX_BADGE_PACKAGE_ROOT');
const noble = required('WEEBDEX_BADGE_NOBLE_ROOT');
const output = required('WEEBDEX_BADGE_OUTPUT_ROOT');
const envRoot = required('WEEBDEX_BADGE_ENV_ROOT');
const revision = process.env.WEEBDEX_BADGE_SOURCE_REVISION;
if (!['b2817b1c93d37b7286b22146d063fbd1059aaa4d', '14a7bab8a925928f658fbd8ebd160b3388e4b540'].includes(revision)) throw Error('Explicit reviewed source revision required');
const gate = process.env.PUBLIC_MANGADEX_CATALOGUE;
if (!['0', '1'].includes(gate)) throw Error('Explicit native0/public1 gate required');
const guard = process.env.WEEBDEX_BADGE_GUARD_MODE;
if (!['installed', 'omitted-control'].includes(guard)) throw Error('Explicit guard profile required');
const publicEnvironment = { PUBLIC_API_URL: 'http://badge-fixture.test/api', PUBLIC_KRATOS_PUBLIC_API: 'http://badge-fixture.test/kratos', PUBLIC_PROXY_URL: 'http://badge-fixture.test/proxy', PUBLIC_MANGADEX_CATALOGUE: gate };
for (const [key, value] of Object.entries(publicEnvironment)) if (process.env[key] !== value) throw Error(`Unexpected ${key}`);
for (const key of Object.keys(process.env)) if (key.startsWith('PUBLIC_') && !Object.hasOwn(publicEnvironment, key)) throw Error(`Undeclared public environment input: ${key}`);
if (fs.readdirSync(envRoot).length) throw Error('Fixture env directory must be empty; never load project secrets');
if (process.env.WEEBDEX_QA_UPLOAD_REFUSALS !== '0') throw Error('Upload refusal QA facade must be explicitly off');
const source = relative => import(pathToFileURL(path.join(platform, relative)).href);
const [frontendOverlay, readerOverlay, localOverlay, publicOverlay, uploadOverlay, boundedOverlay, utility, refusal] = await Promise.all([
  source('platform-integrations/frontend-overlays.mjs'), source('platform-integrations/frontend-reader-overlays.mjs'),
  source('platform-integrations/local-library/overlays.mjs'), source('platform-integrations/public-api/overlays.mjs'),
  source('platform-integrations/public-api/upload-overlays.mjs'), source('platform-integrations/bounded-reader/overlays.mjs'),
  source('platform-integrations/integration-utility-sources.mjs'), source('scripts/public-upload-refusal-qa.mjs')
]);
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const raw = new Map(), transformations = [], virtual = new Map();
let phase = 'unresolved', resolution = null;
function recordText(map, id, code, includeCode = false) {
  if (typeof code !== 'string' || Buffer.byteLength(code) > 2 * 1024 * 1024) throw Error('Unbounded transform source');
  if (map.size >= 8192 && !map.has(id)) throw Error('Source identity count exceeds fixture bound');
  const row = { id, bytes: Buffer.byteLength(code), sha256: sha(code), ...(includeCode ? { code } : {}) };
  const previous = map.get(id); if (previous && previous.sha256 !== row.sha256) throw Error(`Source identity changed within phase: ${id}`);
  map.set(id, row);
}
function observeOverlay(plugin) {
  if (!plugin?.transform) return plugin;
  const hook = plugin.transform, handler = typeof hook === 'function' ? hook : hook.handler;
  const wrapped = async function(code, id, ...args) {
    // The exact copied universal initializer is composed as its production source.
    // No other fixture file may impersonate a frontend overlay target.
    const productionId = id === path.join(fixture, 'src/routes/+layout.ts') ? path.join(frontend, 'src/routes/+layout.ts') : id;
    const value = await handler.call(this, code, productionId, ...args);
    const after = typeof value === 'string' ? value : value?.code;
    if (after !== undefined) {
      if (transformations.length >= 4096) throw Error('Overlay transformation record bound');
      transformations.push({ plugin: plugin.name, id, productionId, before: sha(code), after: sha(after), bytes: Buffer.byteLength(after) });
    }
    return value;
  };
  return { ...plugin, transform: typeof hook === 'function' ? wrapped : { ...hook, handler: wrapped } };
}
const attribution = {
  name: 'badge-fixture-input-attribution', enforce: 'pre',
  configResolved(config) {
    phase = config.build.ssr ? 'server' : 'client';
    // Observe the official plugin's actual per-environment conditions and SSR dependency handling.
    resolution = Object.fromEntries(Object.entries(config.environments ?? {}).map(([name, value]) => [name, { consumer: value.consumer, conditions: value.resolve?.conditions, externalConditions: value.resolve?.externalConditions, noExternal: Array.isArray(value.resolve?.noExternal) ? value.resolve.noExternal.map(String) : value.resolve?.noExternal }]));
  },
  buildStart() { raw.clear(); transformations.length = 0; virtual.clear(); },
  transform: { order: 'pre', handler(code, id) { recordText(raw, id, code); if (id.includes('\0') || id.startsWith('virtual:')) recordText(virtual, id, code, true); return null; } },
  generateBundle: { order: 'post', handler(_options, bundle) {
    const chunks = Object.values(bundle).filter(value => value.type === 'chunk').map(value => ({ file: value.fileName, facade: value.facadeModuleId, imports: value.imports, dynamicImports: value.dynamicImports, modules: Object.keys(value.modules).sort(), bytes: Buffer.byteLength(value.code), sha256: sha(value.code) }));
    if (chunks.length > 1024) throw Error('Emitted chunk bound');
    const evidence = JSON.stringify({ schema: 1, scope: 'real-framework-component-fixture-not-full-application', phase, resolution, revision, profile: gate === '0' ? 'native0' : 'public1', guard, publicEnvironment, rootInitializer: 'unchanged-byte-copy', raw: [...raw.values()].sort((a,b)=>a.id.localeCompare(b.id)), overlayTransforms: transformations, virtualInputs: [...virtual.values()], chunks });
    if (Buffer.byteLength(evidence) > 4 * 1048576) throw Error(`Attribution evidence ${Buffer.byteLength(evidence)} exceeds 4 MiB`);
    this.emitFile({ type: 'asset', fileName: `badge-inputs-${phase}.json`, source: evidence });
  } }
};
const lib = path.join(frontend, 'src/lib');
const repairs = { name: 'attachment-exact-import-case-repair', enforce: 'pre', resolveId(id, importer) {
  if (id === '$lib/utils.js' || id === path.join(lib, 'utils.js')) return path.join(lib, 'Utils.ts');
  if (id === './session' && importer?.split('?')[0] === path.join(lib, 'stores/index.ts')) return path.join(lib, 'stores/Session.ts');
  return null;
} };
const overlays = [
  refusal.createUploadRefusalQaPlugin(platform, { enabled: false, publicUploadEnabled: publicOverlay.readPublicApiGate() }),
  utility.createIntegrationUtilitySources(frontend, path.join(platform, 'platform-integrations')),
  frontendOverlay.createFrontendOverlays(frontend), readerOverlay.createReaderOverlays(frontend),
  localOverlay.createLocalLibraryOverlays(frontend), publicOverlay.createPublicApiOverlays(frontend),
  uploadOverlay.createPublicUploadOverlays(frontend), boundedOverlay.createBoundedReaderOverlays(frontend),
  localOverlay.createLocalLibraryResolver(frontend, path.join(platform, 'platform-integrations/local-library'))
].map(observeOverlay);
export default {
  root: fixture, envDir: envRoot, cacheDir: path.join(output, 'vite-cache'),
  define: { __BADGE_INSTALL_GUARD__: JSON.stringify(guard === 'installed') },
  resolve: { dedupe: ['svelte'], alias: [
    { find: /^@lucide\/svelte$/, replacement: path.join(packages, '@lucide/svelte/dist/lucide-svelte.js') },
    ...Object.entries({ '$badgeHarness': path.join(platform, 'tests/fixtures/bounded-reader/VisibleBadgeHarness.svelte'), '$badgeFixture': path.join(fixture, 'src/lib'), '$boundedReader': path.join(platform, 'platform-integrations/bounded-reader'), '$platform': path.join(platform, 'platform-integrations/frontend-behavior'), '$localLibrary': path.join(platform, 'platform-integrations/local-library'), '$publicApi': path.join(platform, 'platform-integrations/public-api'), 'svelte-sonner': path.join(packages, 'svelte-sonner/dist/index.js'), 'axios': path.join(packages, 'axios/index.js'), 'http-status-codes': path.join(packages, 'http-status-codes/build/es/index.js'), '@noble/curves': path.join(noble, '@noble/curves'), '@noble/hashes': path.join(noble, '@noble/hashes') }).map(([find, replacement]) => ({ find, replacement }))
  ] },
  plugins: [attribution, ...overlays, repairs, sveltekit()],
  build: { target: 'es2022', minify: false, cssMinify: false, sourcemap: false, reportCompressedSize: false, assetsInlineLimit: 0 },
  // No Tailwind root stylesheet/PWA/full app layout: this is a nonvisual component fixture.
  // Reachable component CSS still passes through the actual Svelte/Vite pipeline.
  server: { open: false }, preview: { open: false }, clearScreen: false, logLevel: 'warn'
};
