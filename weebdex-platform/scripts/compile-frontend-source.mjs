#!/usr/bin/env node
import { createAccountLibraryOverlays } from '../platform-integrations/local-library/account-overlays.mjs';
import { createBoundedReaderOverlays } from '../platform-integrations/bounded-reader/overlays.mjs';
// Syntax/source compiler only; no application, browser or database execution.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { prepareFrontendIntegration } from './prepare-frontend-integration.mjs';
import { createFrontendOverlays } from '../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../platform-integrations/frontend-reader-overlays.mjs';
import { createPublicApiOverlays } from '../platform-integrations/public-api/overlays.mjs';
import { createPublicUploadOverlays, createNativeUploadOverlays } from '../platform-integrations/public-api/upload-overlays.mjs';
import { createLocalLibraryOverlays } from '../platform-integrations/local-library/overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const frontend = process.env.WEEBDEX_FRONTEND_ROOT ?? path.join(root, 'frontend');
const require = createRequire(path.join(frontend, 'package.json'));
const { compile } = require('svelte/compiler'), { transform } = require('esbuild');
const integration = prepareFrontendIntegration(frontend);
const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend), createPublicUploadOverlays(frontend), createNativeUploadOverlays(frontend), createBoundedReaderOverlays(frontend), createAccountLibraryOverlays(frontend)];
const failures = [], warnings = []; let components = 0, scripts = 0;
const compileOne = async (file, external = false) => {
  let source = fs.readFileSync(file, 'utf8');
  if (!external) for (const plugin of plugins) source = plugin.transform.handler(source, file)?.code ?? source;
  try {
    if (file.endsWith('.svelte')) { const result = compile(source, { filename: file, generate: 'client' }); components++; for (const warning of result.warnings) warnings.push({ path: path.relative(root, file), code: warning.code, message: warning.message }); }
    else if (/\.(m?[jt]s)$/.test(file)) { await transform(source, { loader: file.endsWith('.ts') ? 'ts' : 'js', sourcefile: file, format: 'esm' }); scripts++; }
  } catch (error) { failures.push({ path: path.relative(root, file), message: error.message }); }
};
for (const entry of integration.transformed_sources) await compileOne(path.join(frontend, entry.path));
async function walk(directory) { for (const entry of fs.readdirSync(directory, { withFileTypes: true })) { const file = path.join(directory, entry.name); if (entry.isDirectory()) await walk(file); else if (!file.endsWith('.fragment.ts') && /\.svelte$|\.ts$/.test(file)) await compileOne(file, true); } }
await walk(path.join(root, 'platform-integrations'));
const result = { status: failures.length ? 'failed' : 'passed', utc: new Date().toISOString(), runtime_tests_executed: false, components_compiled: components, scripts_syntax_compiled: scripts, shared_sha: integration.overlay_module_sha256, reader_sha: integration.reader_overlay_sha256, local_sha: integration.local_library_overlay_sha256, integration_inventory_sha: integration.integration_source_inventory_sha256, warnings, failures };
fs.writeFileSync(path.join(root, 'test-evidence/frontend/composed-svelte-source-compile.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ ...result, warnings: warnings.length }));
if (failures.length) process.exitCode = 1;
