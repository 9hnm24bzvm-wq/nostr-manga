#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createIntegrationUtilitySources } from '../../platform-integrations/integration-utility-sources.mjs';
import { frontendRouteAdditions } from '../../platform-integrations/frontend-route-additions.mjs';
import { sixPersonaRouteAdditions } from '../../platform-integrations/six-persona/overlays.mjs';
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
  assert.match(config, /merged\.plugins\.unshift\(createUploadRefusalQaPlugin\(root, \{ publicUploadEnabled: readPublicApiGate\(\) \}\), createIntegrationUtilitySources\(frontend, path\.join\(root, "platform-integrations"\)\), createFrontendOverlays\(frontend\)/);
});

test('additive route membership is the exact reviewed moderation/reports/closure/following/list/editorial/announcements/identity set', () => {
  assert.deepEqual(frontendRouteAdditions[1], {source:'platform-integrations/frontend-reconstruction/reports/signed/native/+page.svelte',target:'src/routes/(app)/reports/signed/native/+page.svelte',reconstruction:false,additive_signed_native_report_inspection_route:true});
  assert.deepEqual(frontendRouteAdditions[0], {source:'platform-integrations/frontend-reconstruction/reports/moderation/+page.svelte',target:'src/routes/(app)/reports/moderation/+page.svelte',reconstruction:false,additive_signed_moderator_reports_route:true});
  assert.deepEqual(frontendRouteAdditions.map(row => row.target), [
    'src/routes/(app)/reports/moderation/+page.svelte',
    'src/routes/(app)/reports/signed/native/+page.svelte',
    'src/routes/(app)/account/closure/+page.svelte',
    'src/routes/(app)/reports/signed/+page.svelte',
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
  assert.deepEqual(frontendRouteAdditions[2], {source:'platform-integrations/frontend-reconstruction/account/closure/+page.svelte',target:'src/routes/(app)/account/closure/+page.svelte',reconstruction:false,additive_account_disable_route:true});
  for (const addition of frontendRouteAdditions) assert(fs.statSync(path.join(root, addition.source)).isFile());
  assert.equal(frontendRouteAdditions.at(-1).additive_settings_route, true);
  for (const script of ['prepare-frontend-integration.mjs', 'check-composed-frontend.mjs']) assert.match(fs.readFileSync(path.join(root, 'scripts', script), 'utf8'), /for \(const addition of \[\.\.\.frontendRouteAdditions,\s*\.\.\.sixPersonaRouteAdditions\]\)/);
});

test('current seven additive workflow routes are regular sources and do not replace the eighteen historical additions', () => {
  assert.deepEqual(sixPersonaRouteAdditions.map(row => row.target), [
    'src/routes/(app)/staff/+page.svelte', 'src/routes/(app)/staff/reports/+page.svelte',
    'src/routes/(app)/staff/operations/+page.svelte', 'src/routes/(app)/staff/users/+page.svelte',
    'src/routes/(app)/staff/users/[id]/profile/+page.svelte',
    'src/routes/(app)/upload/+page.svelte', 'src/routes/(app)/teams/+page.svelte'
  ]);
  assert.equal(new Set([...frontendRouteAdditions, ...sixPersonaRouteAdditions].map(row => row.target)).size, 25);
  for (const addition of sixPersonaRouteAdditions) {
    assert.equal(addition.reconstruction, false); assert.equal(addition.additive_six_persona_route, true);
    assert(fs.lstatSync(path.join(root, addition.source)).isFile());
  }
});
