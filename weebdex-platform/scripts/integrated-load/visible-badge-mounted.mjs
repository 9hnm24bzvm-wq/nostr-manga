// NEW SOURCE-ONLY RUNNER. It neither generates nor builds anything. Execution
// requires a separately admitted owner and separately reviewed generated inputs.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, realpath, stat } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createBadgeFrameworkDom } from '../../tests/fixtures/bounded-reader/badge-framework-dom-driver.mjs';
import { createBadgeFixture } from '../../tests/fixtures/bounded-reader/badge-mounted-fixture.mjs';
import { runVisibleBadgeMountedCases, runPublicDeviceBadgeCases, runBadgeFrameworkCases, runNativeBadgeTerminalSupplements, NATIVE_BADGE_TERMINAL_SUPPLEMENTS, VISIBLE_BADGE_MOUNTED_CASES, PUBLIC_DEVICE_BADGE_CASES } from './visible-badge-mounted-cases.mjs';
const CURRENT = 'b2817b1c93d37b7286b22146d063fbd1059aaa4d';
const OLD = '14a7bab8a925928f658fbd8ebd160b3388e4b540';
const sha = value => createHash('sha256').update(value).digest('hex');
const requireValue = (value, message) => { if (!value) throw Error(message); };
async function verifiedJson(root, identity, maxBytes) {
  requireValue(identity && typeof identity.path === 'string' && !path.isAbsolute(identity.path) && identity.path.split('/').every(part => part && part !== '.' && part !== '..') && !identity.path.includes('\\'), 'Noncanonical manifest identity');
  requireValue(/^[a-f0-9]{64}$/.test(identity.sha256) && Number.isSafeInteger(identity.bytes) && identity.bytes > 0 && identity.bytes <= maxBytes, 'Invalid manifest identity bound');
  const base = await realpath(root), file = await realpath(path.join(base, identity.path));
  requireValue(file.startsWith(base + path.sep), 'Manifest escapes declared root');
  const info = await stat(file); requireValue(info.isFile() && info.size === identity.bytes, 'Manifest stat mismatch');
  const bytes = await readFile(file); requireValue(bytes.length === identity.bytes && sha(bytes) === identity.sha256, 'Manifest content mismatch');
  return JSON.parse(bytes.toString('utf8'));
}
/**
 * Owner-created specification schema 1:
 * {schema:1, source:{root,identity}, packageClosure:{root,identity}, packageRoot, timeoutMs:15000,
 *  profiles:[
 *   {profile:'native0',guardMode:'installed',assetRoot,manifestIdentity,
 *    comparisons:{[OLD]:{revision:OLD,profile:'native0',assetRoot,manifestIdentity}}},
 *   {profile:'public1',guardMode:'installed',assetRoot,manifestIdentity},
 *   {profile:'native0',guardMode:'omitted-control',assetRoot,manifestIdentity}
 *  ]}
 * The verified source JSON has {revision:CURRENT, files:[{path,bytes,sha256}],
 * claim:'source-comparison-only-unqualified'}. This historical claim describes
 * inputs, not the eventual result. Full source/package verification and process
 * resource ownership belong to the admitted outer owner. This file refuses
 * placeholder, missing, wrong-profile, wrong-revision and unattributed output.
 * All profiles and old comparison are prebuilt and separately reviewed. No
 * implicit generation, installation, download, external service or fallback.
 */
export async function runVisibleBadgeMounted(spec, write = value => process.stdout.write(JSON.stringify(value) + '\n')) {
  requireValue(spec?.schema === 1 && Array.isArray(spec.profiles), 'Invalid owner specification');
  requireValue(Number.isSafeInteger(spec.timeoutMs) && spec.timeoutMs >= 1000 && spec.timeoutMs <= 60000, 'Explicit bounded per-operation timeout required');
  const expectedProfiles = ['native0:installed','public1:installed','native0:omitted-control'];
  assert.deepEqual(spec.profiles.map(row => `${row.profile}:${row.guardMode}`), expectedProfiles, 'One serial exact profile sequence required');
  const source = await verifiedJson(spec.source.root, spec.source.identity, 262144);
  requireValue(source.revision === CURRENT && source.claim === 'source-comparison-only-unqualified' && Array.isArray(source.files), 'Wrong current source descriptor');
  const files = source.files.map(row => row.path.replace(/^weebdex-platform\//, ''));
  requireValue(files.includes('platform-integrations/bounded-reader/badge-anchors.json'), 'Current badge anchor not attributed');
  const currentSource = { claim: source.claim, sha256: spec.source.identity.sha256, files };
  const packageClosure = await verifiedJson(spec.packageClosure.root, spec.packageClosure.identity, 1048576);
  requireValue(packageClosure.schema === 1 && packageClosure.complete === true && Array.isArray(packageClosure.files), 'Complete outer-owner package closure descriptor required');
  let evidenceBytes = 0, eventCount = 0;
  const record = event => {
    const encoded = JSON.stringify(event), bytes = Buffer.byteLength(encoded) + 1;
    requireValue(bytes <= 131072 && ++eventCount <= 30000 && evidenceBytes + bytes <= 3 * 1024 * 1024, 'Mounted evidence envelope exhausted');
    evidenceBytes += bytes; write(event);
  };
  const result = { schema: 1, kind: 'actual-framework-component-fixture', outcome: 'started', sourceRevision: CURRENT, sourceManifestSha256: spec.source.identity.sha256, nativeGroupsExpected: [...VISIBLE_BADGE_MOUNTED_CASES], publicDeviceGroupsExpected: [...PUBLIC_DEVICE_BADGE_CASES], nativeTerminalSupplementsExpected: [...NATIVE_BADGE_TERMINAL_SUPPLEMENTS], profiles: [], excludedClaims: ['full Manga/application acceptance','full browser rendering/accessibility qualification','historical public10/native6 or 434 VM control requalification','unobserved hard resource peak'] };
  record({ type: 'badge-run-start', sourceManifestSha256: spec.source.identity.sha256, profiles: expectedProfiles });
  for (const profile of spec.profiles) {
    const manifest = await verifiedJson(profile.assetRoot, profile.manifestIdentity, 1048576);
    requireValue(manifest.schema === 1 && manifest.sourceRevision === CURRENT && manifest.profile === profile.profile && manifest.guardMode === profile.guardMode, 'Generated manifest does not match requested source/profile/guard');
    requireValue(manifest.sourceManifestSha256 === spec.source.identity.sha256, 'Generated output lacks current source manifest provenance');
    // These are content/provenance checks, not self-issued execution authority.
    // The outer owner must independently inspect and admit the exact review.
    const review = await verifiedJson(profile.assetRoot, manifest.frameworkReview, 262144);
    const generatedInventorySha256 = sha(JSON.stringify({ sourceRevision: manifest.sourceRevision, sourceManifestSha256: manifest.sourceManifestSha256, profile: manifest.profile, guardMode: manifest.guardMode, fallback: manifest.fallback, bootstrap: manifest.bootstrap, assets: manifest.assets }));
    requireValue(review.schema === 1 && review.verdict === 'accepted-static-closure' && review.sourceRevision === CURRENT && review.profile === profile.profile && review.guardMode === profile.guardMode && review.sourceManifestSha256 === spec.source.identity.sha256 && review.generatedInventorySha256 === generatedInventorySha256 && review.packageClosureSha256 === spec.packageClosure.identity.sha256, 'Exact generated closure review is missing or does not cover these outputs/packages');
    if (profile.profile === 'native0' && profile.guardMode === 'installed') {
      const old = profile.comparisons?.[OLD];
      requireValue(old?.revision === OLD && old.profile === 'native0', 'Fresh old-source comparison manifest is required; old snapshots are not reusable');
      const oldManifest = await verifiedJson(old.assetRoot, old.manifestIdentity, 1048576);
      const oldSource = await verifiedJson(old.assetRoot, oldManifest.inputSource, 262144);
      const oldReview = await verifiedJson(old.assetRoot, oldManifest.frameworkReview, 262144);
      const oldInventorySha256 = sha(JSON.stringify({ sourceRevision: oldManifest.sourceRevision, sourceManifestSha256: oldManifest.sourceManifestSha256, profile: oldManifest.profile, guardMode: oldManifest.guardMode, fallback: oldManifest.fallback, bootstrap: oldManifest.bootstrap, assets: oldManifest.assets }));
      requireValue(oldManifest.sourceRevision === OLD && oldManifest.guardMode === 'installed' && oldSource.revision === OLD && oldManifest.inputSource.sha256 === oldManifest.sourceManifestSha256 && oldReview.schema === 1 && oldReview.verdict === 'accepted-static-closure' && oldReview.sourceRevision === OLD && oldReview.profile === 'native0' && oldReview.guardMode === 'installed' && oldReview.sourceManifestSha256 === oldManifest.sourceManifestSha256 && oldReview.generatedInventorySha256 === oldInventorySha256 && oldReview.packageClosureSha256 === spec.packageClosure.identity.sha256, 'Old comparison lacks exact independently reviewed source/framework/package provenance');
    }
    const item = { profile: profile.profile, guardMode: profile.guardMode, manifestSha256: profile.manifestIdentity.sha256, outcome: 'started', groups: [], terminalSupplements: [], framework: [], cleanup: null };
    result.profiles.push(item);
    const fixture = createBadgeFixture({ profile: profile.profile, record, timeoutMs: spec.timeoutMs });
    let driver, operationFailure;
    try {
      driver = await createBadgeFrameworkDom({ manifest, manifestIdentity: profile.manifestIdentity, assetRoot: profile.assetRoot, packageRoot: spec.packageRoot, fixture, record, timeoutMs: spec.timeoutMs, comparisons: profile.comparisons ?? {} });
      fixture.attachDriver(driver);
      await driver.boot({ href: 'http://badge-fixture.test/__badge_outside' });
      if (profile.guardMode === 'installed') {
        item.groups = profile.profile === 'native0'
          ? await runVisibleBadgeMountedCases({ page: driver.page, fixture, currentSource, record })
          : await runPublicDeviceBadgeCases({ page: driver.page, fixture, record });
      }
      if (profile.profile === 'native0' && profile.guardMode === 'installed') item.terminalSupplements = await runNativeBadgeTerminalSupplements({ page: driver.page, fixture, record });
      item.framework = await runBadgeFrameworkCases({ page: driver.page, fixture, driver, guardMode: profile.guardMode, record });
      await fixture.assertIdle();
      item.outcome = 'passed';
    } catch (error) {
      operationFailure = error;
      item.outcome = error?.infrastructure || /INFRASTRUCTURE/.test(error?.code ?? '') ? 'infrastructure-failed' : 'assertion-or-application-failed';
      item.failure = { name: error?.name ?? 'Error', code: error?.code ?? null, message: String(error?.message ?? error).slice(0,4096) };
    } finally {
      try { item.cleanup = driver ? await driver.dispose() : await fixture.close(); }
      catch (error) { item.cleanup = { outcome: 'failed', message: String(error?.message ?? error).slice(0,4096) }; item.outcome = 'cleanup-failed'; operationFailure ??= error; }
      record({ type: 'badge-profile-terminal', ...item });
    }
    if (operationFailure) { result.outcome = item.outcome; record({ type: 'badge-run-terminal', ...result, evidenceBytes, eventCount }); throw operationFailure; }
  }
  result.outcome = 'passed'; record({ type: 'badge-run-terminal', ...result, evidenceBytes, eventCount }); return result;
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    requireValue(process.argv.length === 3, 'Usage: pinned node --experimental-vm-modules visible-badge-mounted.mjs OWNER_SPEC.json');
    const info = await stat(process.argv[2]); requireValue(info.isFile() && info.size <= 65536, 'Owner specification must be a regular file <=64 KiB');
    const bytes = await readFile(process.argv[2]); requireValue(bytes.length <= 65536, 'Owner specification grew beyond bound');
    await runVisibleBadgeMounted(JSON.parse(bytes.toString('utf8')));
  } catch (error) { process.stderr.write(JSON.stringify({ outcome: 'failed', name: error?.name ?? 'Error', message: String(error?.message ?? error).slice(0,4096) }) + '\n'); process.exitCode = 1; }
}
