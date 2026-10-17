// Source-only production-stage association; no build/preparer/runtime is executed.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyPublicUploadOverlay } from '../../platform-integrations/public-api/upload-overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const nativeText=read('platform-integrations/frontend-overlays.mjs');
const native=JSON.parse(nativeText.slice(nativeText.indexOf('const patches = [')+'const patches = '.length,nativeText.indexOf('\n];\nconst hash')+2));
const spec=JSON.parse(read('platform-integrations/public-api/upload-anchors.json'));
test('strict checker and syntax compiler include the same public upload production stage after public catalogue',()=>{
  for(const name of ['check-composed-frontend','compile-frontend-source']){
    const source=read(`scripts/${name}.mjs`);
    assert.match(source,/import \{ createPublicUploadOverlays \} from '..\/platform-integrations\/public-api\/upload-overlays.mjs'/);
    assert.match(source,/createPublicApiOverlays\((input|frontend)\), createPublicUploadOverlays\(\1\)\]/);
  }
});
test('preparer retains existing markup verifier on the exact preceding stage and records signed exceptions',()=>{
  const source=read('scripts/prepare-frontend-integration.mjs');
  assert.match(source,/const uploadPlugin = createPublicUploadOverlays\(frontend, \{ enabled: publicApiGate \}\)/);
  assert.match(source,/\.\.\.publicRepairs, \.\.\.publicUploadRepairs\]\.map/);
  assert.match(source,/const preUpload = publicResult\?\.code \?\? prePublic/);
  assert.match(source,/const uploadResult = uploadPlugin.transform.handler\(preUpload, file\)/);
  assert.match(source,/let normalizedResult = preUpload/);
  assert.match(source,/if \(retainedMarkup\(original\) !== retainedMarkup\(normalizedResult\)\) throw/);
  assert.match(source,/explicit_public_signed_upload_extension: !!uploadResult/);
  assert.match(source,/public_signed_upload_stage: uploadResult \? publicUploadRepairs.find/);
});
test('build evidence binds the same selected-instance flag and exact upload stage sources',()=>{
  const prepare=read('scripts/prepare-frontend-integration.mjs'),build=read('scripts/build-frontend.mjs');
  for(const key of ['public_signed_upload_enabled','public_signed_upload_overlay_sha256','public_signed_upload_anchors_sha256','approved_public_signed_upload_extensions']){
    assert.ok(prepare.includes(key));assert.ok(build.includes(`${key}: integration.${key}`));
  }
  assert.match(prepare,/const publicUploadRepairs = publicApiGate \? publicUploadOverlayManifest : \[\]/);
  assert.match(build,/PUBLIC_MANGADEX_CATALOGUE: process.env.PUBLIC_MANGADEX_CATALOGUE \?\? "0"/);
});
test('reviewed signed UI exceptions preserve every original style block',()=>{
  const styles=code=>[...code.matchAll(/<style\b[^>]*>[\s\S]*?<\/style>/g)].map(match=>match[0]);
  for(const file of spec.files){
    const original=read('frontend/'+file.path);let composed=original;
    for(const patch of native.filter(patch=>patch.path===file.path)){
      assert.equal(composed.split(patch.before).length-1,patch.count);composed=composed.split(patch.before).join(patch.after);
    }
    assert.deepEqual(styles(applyPublicUploadOverlay(composed,file.path)),styles(original),file.path);
  }
});
