// Source-only production-stage association; no build/preparer/runtime is executed.
// Historical reconstruction is comparison data only, never a production input.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { applyPublicUploadOverlay, applyNativeUploadOverlay, nativeUploadOverlayManifest } from '../../platform-integrations/public-api/upload-overlays.mjs';
import { createBoundedReaderOverlays, boundedReaderOverlayManifest, boundedReaderApprovedMarkupExceptions, assertBoundedReaderExceptionScope } from '../../platform-integrations/bounded-reader/overlays.mjs';
import { assertBadgeExceptionScope } from '../../platform-integrations/bounded-reader/badge-overlays.mjs';
import { restoreBoundedReaderSource } from './reviewed-bounded-reader-source-successors.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const hash=value=>createHash('sha256').update(value).digest('hex');
const historical=relative=>restoreBoundedReaderSource('bounded-reader-to-frozen-main:'+relative,'weebdex-platform/'+relative,read(relative));
const shared=createFrontendOverlays(path.join(root,'frontend'));
const spec=JSON.parse(read('platform-integrations/public-api/upload-anchors.json'));
const readerPatches=JSON.parse(read('platform-integrations/bounded-reader/anchors.json'));
const badgePatches=JSON.parse(read('platform-integrations/bounded-reader/badge-anchors.json'));

test('historical comparison alone retains the old upload-last association',()=>{
  for(const name of ['check-composed-frontend','compile-frontend-source']){
    const source=historical(`scripts/${name}.mjs`);
    assert.match(source,/import \{ createPublicUploadOverlays \} from '..\/platform-integrations\/public-api\/upload-overlays.mjs'/);
    assert.match(source,/createPublicApiOverlays\((input|frontend)\), createPublicUploadOverlays\(\1\)\]/);
    assert.ok(!source.includes('createBoundedReaderOverlays'));
  }
  assert.match(historical('scripts/prepare-frontend-integration.mjs'),/\.\.\.publicRepairs, \.\.\.publicUploadRepairs\]\.map/);
  assert.ok(!historical('scripts/vite.platform.config.mjs').includes('$boundedReader'));
});

test('physical checker, compiler and Vite keep the actual eight-stage production order',()=>{
  for(const name of ['check-composed-frontend','compile-frontend-source']){
    const source=read(`scripts/${name}.mjs`);
    assert.match(source,/import \{ createBoundedReaderOverlays \} from '..\/platform-integrations\/bounded-reader\/overlays.mjs'/);
    assert.match(source,/const plugins = \[createFrontendOverlays\((input|frontend)\), createReaderOverlays\(\1\), createLocalLibraryOverlays\(\1\), createPublicApiOverlays\(\1\), createPublicUploadOverlays\(\1\), createNativeUploadOverlays\(\1\), createBoundedReaderOverlays\(\1\), createAccountLibraryOverlays\(\1\)\];/);
  }
  const vite=read('scripts/vite.platform.config.mjs');
  assert.match(vite,/createFrontendOverlays\(frontend\), createReaderOverlays\(frontend\), createLocalLibraryOverlays\(frontend\), createPublicApiOverlays\(frontend\), createPublicUploadOverlays\(frontend\), createNativeUploadOverlays\(frontend\), createBoundedReaderOverlays\(frontend\), createAccountLibraryOverlays\(frontend\), createLocalLibraryResolver/);
  assert.match(vite,/"\$boundedReader": path.join\(root, "platform-integrations\/bounded-reader"\)/);
});

test('preparer keeps the exact preceding upload stage and records the real native upload, bounded reader and final account library stages',()=>{
  const source=read('scripts/prepare-frontend-integration.mjs');
  assert.match(source,/const publicPlugin = createPublicApiOverlays\(frontend, \{ enabled: publicApiGate \}\)/);
  assert.match(source,/const shared = plugin.transform.handler\(original, file\)/);
  assert.match(source,/const reader = readerPlugin.transform.handler\(shared\?\.code \?\? original, file\)/);
  assert.match(source,/const local = localPlugin.transform.handler\(reader\?\.code \?\? shared\?\.code \?\? original, file\)/);
  assert.match(source,/const prePublic = local\?\.code \?\? reader\?\.code \?\? shared\?\.code \?\? original/);
  assert.match(source,/const publicResult = publicPlugin.transform.handler\(prePublic, file\)/);
  assert.match(source,/const uploadPlugin = createPublicUploadOverlays\(frontend, \{ enabled: publicApiGate \}\)/);
  assert.match(source,/const boundedReaderPlugin = createBoundedReaderOverlays\(frontend\)/);
  assert.match(source,/\.\.\.frontendOverlayManifest, \.\.\.readerOverlayManifest, \.\.\.localLibraryOverlayManifest, \.\.\.publicRepairs, \.\.\.publicUploadRepairs, \.\.\.boundedReaderOverlayManifest\]\.map/);
  assert.match(source,/const preUpload = publicResult\?\.code \?\? prePublic/);
  assert.match(source,/const uploadResult = uploadPlugin.transform.handler\(preUpload, file\)/);
  assert.match(source,/const preNativeUpload = uploadResult\?\.code \?\? preUpload/);
  assert.match(source,/const nativeUploadResult = nativeUploadPlugin.transform.handler\(preNativeUpload, file\)/);
  assert.match(source,/const preBoundedReader = nativeUploadResult\?\.code \?\? preNativeUpload/);
  assert.match(source,/const boundedReaderResult = boundedReaderPlugin.transform.handler\(preBoundedReader, file\)/);
  assert.match(source,/const preAccountLibrary = boundedReaderResult\?\.code \?\? preBoundedReader/);
  assert.match(source,/const accountLibraryResult = accountLibraryPlugin.transform.handler\(preAccountLibrary, file\)/);
  assert.match(source,/const result = accountLibraryResult \?\? boundedReaderResult \?\? nativeUploadResult \?\? uploadResult \?\? publicResult \?\? local \?\? reader \?\? shared/);
  assert.match(source,/let normalizedResult = preUpload/);
  assert.match(source,/if \(retainedMarkup\(original\) !== retainedMarkup\(normalizedResult\)\) throw/);
  assert.match(source,/explicit_public_signed_upload_extension: !!uploadResult/);
  assert.match(source,/public_signed_upload_stage: uploadResult \? publicUploadRepairs.find/);
  assert.match(source,/explicit_bounded_reader_extension: !!boundedReaderResult, bounded_reader_previous_sha256: hash\(preBoundedReader\)/);
  assert.match(source,/bounded_reader_repairs: boundedReaderOverlayManifest, approved_bounded_reader_extensions: boundedReaderApprovedMarkupExceptions/);
  assert.match(source,/bounded_reader_source_only: true, bounded_reader_exact_automatic_continue_parity: false/);
  for(const component of ['TitleReadingActions','ChapterSelector','EditionSelector','GroupPage','EditionPage','Status'])assert.ok(source.includes(`"bounded-reader/${component}.svelte"`));
});

test('unchanged bounded-reader dependency pins bind both independent reader and badge guards',()=>{
  const pins={
    'overlays.mjs':'a7d89c85f2fdf435c8606f82c5519e532c2717fed698e72fcca81eaaf201c4aa',
    'anchors.json':'feafb34479f4d81413f44cc24d786beb431e9bebffc6d6be24de1753139d2fae',
    'badge-overlays.mjs':'e36bdf2b2e00c6e6a485f3fbc7d6f63cf05c59f59c4847e87854cdf3561bb960',
    'badge-anchors.json':'a81b80af1b637794ea23834b713ee06c4bfa5161a009708ef01f7badebea0ca4'
  };
  for(const [file,digest] of Object.entries(pins))assert.equal(hash(read('platform-integrations/bounded-reader/'+file)),digest,file);
  assert.equal(readerPatches.length,34);assert.equal(readerPatches.filter(patch=>patch.markup).length,12);
  assert.equal(badgePatches.length,16);assert.equal(badgePatches.filter(patch=>patch.markup).length,5);
  assert.deepEqual(boundedReaderOverlayManifest,[...readerPatches,...badgePatches].map(({before,after,...metadata})=>metadata));
  assert.deepEqual(boundedReaderApprovedMarkupExceptions,[...readerPatches,...badgePatches].filter(patch=>patch.markup));
  assertBadgeExceptionScope();assertBoundedReaderExceptionScope();
  assert.match(read('platform-integrations/bounded-reader/overlays.mjs'),/from '.\/badge-overlays.mjs'/);
  assert.match(read('platform-integrations/bounded-reader/badge-overlays.mjs'),/new URL\('.\/badge-anchors.json', import.meta.url\)/);
});

test('build evidence binds the same selected-instance flag and exact upload stage sources',()=>{
  const prepare=read('scripts/prepare-frontend-integration.mjs'),build=read('scripts/build-frontend.mjs');
  for(const key of ['public_signed_upload_enabled','public_signed_upload_overlay_sha256','public_signed_upload_anchors_sha256','approved_public_signed_upload_extensions','native_account_upload_enabled','native_account_upload_repairs','approved_native_account_upload_exceptions','native_account_contribution_qualification']){
    assert.ok(prepare.includes(key));assert.ok(build.includes(`${key}: integration.${key}`));
  }
  assert.match(prepare,/const publicUploadRepairs = publicApiGate \? publicUploadOverlayManifest : \[\]/);
  assert.match(build,/PUBLIC_MANGADEX_CATALOGUE: process.env.PUBLIC_MANGADEX_CATALOGUE \?\? "0"/);
});

test('reviewed upload outputs preserve styles and the bounded-reader stage never transforms upload targets',()=>{
  const styles=code=>[...code.matchAll(/<style\b[^>]*>[\s\S]*?<\/style>/g)].map(match=>match[0]);
  const bounded=createBoundedReaderOverlays(path.join(root,'frontend'));
  const boundedTargets=new Set(boundedReaderOverlayManifest.map(patch=>patch.path));
  for(const file of spec.files){
    assert.equal(boundedTargets.has(file.path),false,'Reader/badge and upload targets must remain disjoint');
    const original=read('frontend/'+file.path);let composed=original;
    composed=shared.transform.handler(composed,path.join(root,'frontend',file.path))?.code??composed;
    const uploaded=applyPublicUploadOverlay(composed,file.path);
    assert.deepEqual(styles(uploaded),styles(original),file.path);
    const native=applyNativeUploadOverlay(uploaded,file.path),successor=nativeUploadOverlayManifest.find(row=>row.path===file.path);
    if(successor){assert.equal(typeof native,'string');assert.equal(hash(uploaded),successor.predecessor_sha256);assert.equal(hash(native),successor.output_sha256);assert.deepEqual(styles(native),styles(original),file.path+' native successor');}else assert.equal(native,null,'No unregistered native upload transform');
    assert.equal(bounded.transform.handler(composed,path.join(root,'frontend',file.path)),null,file.path+' native gate');
    assert.equal(bounded.transform.handler(uploaded,path.join(root,'frontend',file.path)),null,file.path+' public gate');
  }
});
