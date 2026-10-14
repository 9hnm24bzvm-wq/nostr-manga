#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createIntegrationUtilitySources } from '../../platform-integrations/integration-utility-sources.mjs';
import { frontendRouteAdditions } from '../../platform-integrations/frontend-route-additions.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('utility scan includes the reviewed external integrations without rewriting original CSS', () => {
  const frontend = path.join(root, 'frontend'), integrations = path.join(root, 'platform-integrations');
  const plugin = createIntegrationUtilitySources(frontend, integrations), target = path.join(frontend, 'src/routes/layout.css');
  const original = fs.readFileSync(target, 'utf8');
  assert.equal(plugin.enforce, 'pre');
  const result = plugin.transform(original, target);
  assert.equal(result.code, original + '\n@source ' + JSON.stringify(integrations) + ';\n');
  assert.equal(plugin.transform(original, target + '?direct').code, result.code);
  assert.equal(plugin.transform(original, path.join(root, 'unrelated.css')), null);
  assert.equal(fs.readFileSync(target, 'utf8'), original);
  const config = fs.readFileSync(path.join(root, 'scripts/vite.platform.config.mjs'), 'utf8');
  assert.match(config, /merged\.plugins\.unshift\(createIntegrationUtilitySources\(frontend, path\.join\(root, "platform-integrations"\)\)/);
});

test('additive route membership is exactly existing announcements plus anonymous identity', () => {
  assert.deepEqual(frontendRouteAdditions.map(row => row.target), [
    'src/routes/(app)/announcements/+page.svelte',
    'src/routes/(app)/announcements/[id]/+page.svelte',
    'src/routes/(app)/announcements/manage/+page.svelte',
    'src/routes/(app)/settings/identity/+page.svelte',
  ]);
  for (const addition of frontendRouteAdditions) assert(fs.statSync(path.join(root, addition.source)).isFile());
  assert.equal(frontendRouteAdditions.at(-1).additive_settings_route, true);
  for (const script of ['prepare-frontend-integration.mjs', 'check-composed-frontend.mjs']) assert.match(fs.readFileSync(path.join(root, 'scripts', script), 'utf8'), /for \(const addition of frontendRouteAdditions\)/);
});

test('actual generated CSS contains the integration-only native dialog width utility', () => {
  const assets = path.join(root, 'artifacts/frontend/_app/immutable/assets');
  const css = fs.readdirSync(assets).filter(name => name.endsWith('.css')).map(name => fs.readFileSync(path.join(assets, name), 'utf8')).join('\n');
  // This 42rem selector exists only in external integration components. Testing
  // the emitted selector and declaration catches an omitted Tailwind source scan.
  assert(css.includes('.\\!max-w-\\[min\\(42rem\\,calc\\(100dvw-2rem\\)\\)\\]'), 'Missing external integration utility selector');
  assert.match(css, /max-width:min\(42rem,100dvw - 2rem\)!important/);
  assert(!css.includes('/workspace/'), 'Build-local scan paths must not leak into static CSS');
});
