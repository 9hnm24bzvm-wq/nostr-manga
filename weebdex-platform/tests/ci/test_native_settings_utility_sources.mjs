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

test('additive route membership is the exact reviewed following/list/editorial/announcements/identity set', () => {
  assert.deepEqual(frontendRouteAdditions.map(row => row.target), [
    'src/routes/(app)/following/manga/+page.svelte',
    'src/routes/(app)/following/group/+page.svelte',
    'src/routes/(app)/following/user/+page.svelte',
    'src/routes/(app)/list/[id]/+page.svelte',
    'src/routes/(app)/user/[id]/lists/+page.svelte',
    'src/routes/(app)/following/lists/+page.svelte',
    'src/routes/(app)/catalogue-tools/+page.svelte',
    'src/routes/(app)/author/[id]/settings/+page.ts',
    'src/routes/(app)/group/[id]/settings/+page.ts',
    'src/routes/(app)/create/group/+page.ts',
    'src/routes/(app)/announcements/+page.svelte',
    'src/routes/(app)/announcements/[id]/+page.svelte',
    'src/routes/(app)/announcements/manage/+page.svelte',
    'src/routes/(app)/settings/identity/+page.svelte',
  ]);
  for (const addition of frontendRouteAdditions) assert(fs.statSync(path.join(root, addition.source)).isFile());
  assert.equal(frontendRouteAdditions.at(-1).additive_settings_route, true);
  for (const script of ['prepare-frontend-integration.mjs', 'check-composed-frontend.mjs']) assert.match(fs.readFileSync(path.join(root, 'scripts', script), 'utf8'), /for \(const addition of frontendRouteAdditions\)/);
});
