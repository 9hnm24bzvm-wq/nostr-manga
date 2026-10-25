// NEW qualification infrastructure. This is not the production app configuration.
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import path from 'node:path';
const required = name => { const value = process.env[name]; if (!value || !path.isAbsolute(value)) throw Error(`Absolute ${name} required`); return path.resolve(value); };
const frontend = required('WEEBDEX_BADGE_FRONTEND_ROOT');
const output = required('WEEBDEX_BADGE_OUTPUT_ROOT');
const revision = process.env.WEEBDEX_BADGE_SOURCE_REVISION;
if (!['b2817b1c93d37b7286b22146d063fbd1059aaa4d', '14a7bab8a925928f658fbd8ebd160b3388e4b540'].includes(revision)) throw Error('Reviewed source revision required');
if (!['0', '1'].includes(process.env.PUBLIC_MANGADEX_CATALOGUE)) throw Error('Explicit native0/public1 gate required');
export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ pages: path.join(output, 'served'), assets: path.join(output, 'served'), fallback: 'index.html', precompress: false }),
    outDir: '.svelte-kit', // Owned projection path; matches this fixture's tsconfig extends.
    files: { routes: 'src/routes', lib: path.join(frontend, 'src/lib'), assets: 'static', appTemplate: 'src/app.html', hooks: { client: 'absent/hooks.client', server: 'absent/hooks.server', universal: 'absent/hooks' }, serviceWorker: 'absent/service-worker' },
    appDir: '_app', embedded: false,
    paths: { base: '', assets: '', relative: true },
    router: { type: 'pathname', resolution: 'client' },
    version: { name: `badge-${revision}-${process.env.PUBLIC_MANGADEX_CATALOGUE}-${process.env.WEEBDEX_BADGE_GUARD_MODE ?? 'installed'}`, pollInterval: 0 },
    output: { bundleStrategy: 'split', preloadStrategy: 'modulepreload' },
    serviceWorker: { register: false },
    experimental: { remoteFunctions: false, forkPreloads: false, handleRenderingErrors: false, tracing: { server: false }, instrumentation: { server: false } },
    env: { dir: required('WEEBDEX_BADGE_ENV_ROOT'), publicPrefix: 'PUBLIC_', privatePrefix: '' },
    prerender: { entries: [] }
  }
};
