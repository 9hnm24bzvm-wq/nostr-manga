import {nativeSiteClientBindingStage,nativeSiteClientBindingPath,restoreNativeSiteClientBindingSource} from './reviewed-native-source-successors.mjs';
import {restoreBoundedReaderSource} from './reviewed-bounded-reader-source-successors.mjs';
import {integrationSuccessorPaths,restoreIntegratedSource} from './reviewed-integration-source-successors.mjs';
import {historySuccessorPaths,restoreHistorySource} from './reviewed-history-source-successors.mjs';
// Source/recipe checks only. No app, DOM, browser, database, socket or load run.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {nativeReadSuccessorPaths,restoreNativeReadSource} from './reviewed-native-source-successors.mjs';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
import {createReaderOverlays} from '../../platform-integrations/frontend-reader-overlays.mjs';
import {createLocalLibraryOverlays,createLocalLibraryResolver} from '../../platform-integrations/local-library/overlays.mjs';
import {createPublicApiOverlays} from '../../platform-integrations/public-api/overlays.mjs';
import {createPublicUploadOverlays} from '../../platform-integrations/public-api/upload-overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const frontend=path.join(root,'weebdex-platform/frontend');
const plan=JSON.parse(fs.readFileSync(path.join(root,'weebdex-platform/docs/fixtures/integrated-read-fanout-v1.json')));
const associationBytes=fs.readFileSync(path.join(root,'weebdex-platform/docs/fixtures/integrated-read-fanout-current-source.json'));
const association=JSON.parse(associationBytes);
const predecessorBytes=fs.readFileSync(path.join(root,'weebdex-platform/docs/fixtures/integrated-read-fanout-pre-language-source.json'));
const predecessor=JSON.parse(predecessorBytes);
const addedRuntimeSources=['language.ts','manga-search-query.ts','PublicLanguageFilter.svelte'].map(name=>'weebdex-platform/platform-integrations/public-api/'+name);
const hash=data=>createHash('sha256').update(data).digest('hex');
const plugins=[createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend),createPublicApiOverlays(frontend,{enabled:true}),createPublicUploadOverlays(frontend,{enabled:true})];
function historicalAggregateComposition(relative){const id=path.join(frontend,relative);let code=fs.readFileSync(id,'utf8');for(const plugin of plugins){const result=plugin.transform.handler(code,id);if(result)code=result.code;}return code;}
const scenario=id=>{const s=plan.scenarios.find(s=>s.id===id);assert.ok(s,id);return s;};
// Compare only the exact reviewed native successor against the immutable historical
// association. The five-stage composition above is the retained historical aggregate
// model, not current six-stage bounded-reader evidence. Its modules stay physical.
// Actual bounded-stage replay is covered by the physical-current composition suites.
function associatedSource(file,bytes){if(file===nativeSiteClientBindingPath)bytes=restoreNativeSiteClientBindingSource(nativeSiteClientBindingStage,file,bytes);if(file==='weebdex-platform/platform-integrations/local-library/library.ts')bytes=restoreBoundedReaderSource('bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts',file,bytes);if(integrationSuccessorPaths.includes(file))bytes=restoreIntegratedSource(file,bytes);return historySuccessorPaths.includes(file)?restoreHistorySource(file,bytes):nativeReadSuccessorPaths.includes(file)?restoreNativeReadSource(file,bytes):bytes;}
function validateCurrentAssociation(bytes,read=file=>fs.readFileSync(path.join(root,file))){
 assert.equal(hash(bytes),'1ad1dc4cc04b2bad3321a579877afe2e53f13860e950a1e0c17dab67b387d65b','Unknown current source-association bytes');
 const value=JSON.parse(bytes);
 for(const [file,pin]of Object.entries(value.sourcePins))assert.equal(hash(associatedSource(file,read(file))),pin,'Unknown current source bytes: '+file);
 return value;
}

test('plan stays source-pinned and explicitly unmeasured/unadmitted',()=>{
 assert.equal(plan.sourceCommit,'5cbd05d91c38e22eb6a7509aa6e9f8c21b12e3b4');
 assert.equal(association.schema,'integrated-read-fanout-source-association-v1');
 assert.equal(association.status,'current-source-association-only-not-executed-app-load');
 assert.match(association.sourceCommit,/^[a-f0-9]{40}$/);
 assert.equal(association.recipe_sha256,hash(fs.readFileSync(path.join(root,'weebdex-platform/docs/fixtures/integrated-read-fanout-v1.json'))));
 assert.equal(association.historical_recipe_sourceCommit,plan.sourceCommit);
 assert.equal(association.browser_measured,false);assert.equal(association.capacity_qualified,false);
 assert.deepEqual(validateCurrentAssociation(associationBytes),association);
 assert.equal(Object.keys(association.sourcePins).length,40);
 assert.deepEqual(Object.keys(association.sourcePins).sort(),[...Object.keys(plan.sourcePins),...addedRuntimeSources].sort());
 const changed=Object.keys(plan.sourcePins).filter(file=>plan.sourcePins[file]!==association.sourcePins[file]);
 assert.deepEqual(Object.keys(association.changes_from_historical_recipe).sort(),changed.sort());
 for(const file of changed){const row=association.changes_from_historical_recipe[file];assert.equal(row.original_sha256,plan.sourcePins[file]);assert.equal(row.current_sha256,association.sourcePins[file]);assert.ok(row.reason.length>40);}
 for(const [file,pin]of Object.entries(association.sourcePins))assert.equal(hash(associatedSource(file,fs.readFileSync(path.join(root,file)))),pin,file);
 assert.equal(plan.dispatch_ready,false);assert.equal(plan.browser_measured,false);assert.equal(plan.capacity_qualified,false);assert.equal(plan.default_api_calls_per_page,null);
});
test('every explicit recipe accounts all actual attempts and names reviewed transport lanes',()=>{
 const ids=new Set();for(const s of plan.scenarios){assert(!ids.has(s.id));ids.add(s.id);assert.equal(s.logical_actions,1);assert.equal(s.conditional_lanes_separate,true);assert.equal(s.expected_http_attempts,s.steps.reduce((n,step)=>{assert(plan.endpoints[step.endpoint]);assert(Number.isSafeInteger(step.attempts)&&step.attempts>0);return n+step.attempts;},0));}
 for(const row of Object.values(plan.endpoints))assert(['setup','catalogue','reader','media','analytics'].includes(row.lane));
 assert.equal(plan.endpoints.forum_context.method,'POST');assert.equal(plan.endpoints.forum_context.effect,'read');assert.equal(plan.accounting.read_only_post_queries_consume_permit,true);
});
test('UUID detail and every mounted selector recipe retain actual conditional fanout',()=>{
 assert.equal(scenario('uuid-chapter-detail').expected_http_attempts,4);assert.equal(scenario('uuid-reader-cold-three-chapters').expected_http_attempts,8);assert.equal(scenario('uuid-reader-cold-101-chapters').expected_http_attempts,11);assert.equal(scenario('uuid-reader-same-title-new-mount').expected_http_attempts,8);
 const api=historicalAggregateComposition('src/lib/API/Chapter.ts');assert(api.includes('publicCatalogueRead<Chapter>("chapter.get"'));assert(api.includes('"chapter.aggregate"'));
 const provider=historicalAggregateComposition('src/lib/components/reader/reader-provider.svelte');assert(!provider.includes('lastMangaId'));assert(provider.includes('Every mounted reader rechecks current authority'));assert(provider.includes('API.Chapter.Aggregate(mangaId, q)'));
});
test('mounted media uses actual at-home URL and initial plus three retries',()=>{
 const queue=historicalAggregateComposition('src/lib/components/reader/reader-queue.svelte');assert(queue.includes('retainedPageUrl('));assert(queue.includes('const maxAttempts = 3'));assert(queue.includes('attempts < maxAttempts'));assert(queue.includes('load(page, attempts + 1)'));assert(queue.includes('$readerPrefs.preload'));assert(queue.includes('$readerPrefs.parallel'));
 assert.equal(scenario('uuid-reader-image-failure-cycle').expected_http_attempts,4);assert.equal(scenario('uuid-reader-image-failure-cycle').outcome,'expected-image-failure');
});
test('cold tags are paginated while a nonempty grouped cache is reusable',()=>{
 const code=historicalAggregateComposition('src/lib/stores/global.ts');assert(code.includes('Object.keys(get(tagList) ?? {}).length'));assert(code.includes('page <= 100'));assert(code.includes('query.set("limit", "100")'));assert(code.includes('tags.length >= res.body.total'));
 assert.equal(scenario('uuid-search-warm-tags').expected_http_attempts,1);assert.equal(scenario('uuid-search-cold-101-tags').expected_http_attempts,3);
});
test('library resolver and candidate map preserve device-global hydration rather than displayed page size',()=>{
 const resolver=createLocalLibraryResolver(frontend,path.join(root,'weebdex-platform/platform-integrations/local-library'));
 assert.equal(resolver.resolveId('$lib/API/MangaFollow'),path.join(root,'weebdex-platform/platform-integrations/local-library/MangaFollow.ts'));
 assert.equal(scenario('uuid-library-empty').expected_http_attempts,0);assert.equal(scenario('uuid-library-101-saved').expected_http_attempts,2);
 const code=fs.readFileSync(path.join(root,'weebdex-platform/platform-integrations/public-api/library-catalogue.ts'),'utf8');assert(code.includes('if(!ids.length)return []'));assert(code.includes('Math.ceil(unique.length/100)'));assert(code.includes("query.set('offset',String(offset))"));
});
test('mounted history and typed view observation remain separate destinations',()=>{
 const history=historicalAggregateComposition('src/lib/components/reader/reader-history.svelte');assert(history.includes('timer = window.setTimeout(() => flush(), 350)'));assert(history.includes('saveProgress('));
 const analytics=historicalAggregateComposition('src/lib/components/reader/reader-analytics.svelte');assert(analytics.includes('public-view:chapter:'));assert(analytics.includes('response?.status === 204'));assert(analytics.includes('1000'));
 for(const id of ['uuid-reader-loaded-page-turn','device-progress-many-local-saves'])assert.equal(scenario(id).expected_http_attempts,0);
 assert.equal(plan.accounting.default_device_progress_http_writes,0);assert.equal(plan.endpoints.chapter_view.effect,'analytics-write');
});

test('exact five-row and three-runtime-source successor preserves the entire historical37 association',()=>{
 assert.equal(hash(predecessorBytes),'3f6f477da56e35a77112923e0ce0177286bd4f7c3e045f4b5d0d5d2936adcbe2');
 assert.equal(Object.keys(predecessor.sourcePins).length,37);
 assert.deepEqual(Object.keys(predecessor.sourcePins).sort(),Object.keys(plan.sourcePins).sort());
 assert.deepEqual(association.predecessor_association,{path:'integrated-read-fanout-pre-language-source.json',sha256:hash(predecessorBytes),sourceCommit:predecessor.sourceCommit});
 const changed=Object.keys(predecessor.sourcePins).filter(file=>predecessor.sourcePins[file]!==association.sourcePins[file]);
 assert.equal(changed.length,5);
 assert.deepEqual(Object.keys(association.changes_from_predecessor).sort(),changed.sort());
 assert.deepEqual(Object.keys(association.added_runtime_sources).sort(),addedRuntimeSources.sort());
 const reconstructed=structuredClone(association);
 reconstructed.sourceCommit=predecessor.sourceCommit;
 delete reconstructed.predecessor_association;delete reconstructed.changes_from_predecessor;delete reconstructed.added_runtime_sources;
 for(const file of addedRuntimeSources){assert.equal(association.added_runtime_sources[file].current_sha256,association.sourcePins[file]);assert.ok(association.added_runtime_sources[file].reason.length>40);delete reconstructed.sourcePins[file];}
 for(const file of changed){const row=association.changes_from_predecessor[file];assert.equal(row.original_sha256,predecessor.sourcePins[file]);assert.equal(row.current_sha256,association.sourcePins[file]);assert.ok(row.reason.length>40);reconstructed.sourcePins[file]=row.original_sha256;if(Object.hasOwn(predecessor.changes_from_historical_recipe,file))reconstructed.changes_from_historical_recipe[file]=predecessor.changes_from_historical_recipe[file];else delete reconstructed.changes_from_historical_recipe[file];}
 assert.deepEqual(reconstructed,predecessor);
 assert.equal(hash(JSON.stringify(reconstructed,null,2)+'\n'),hash(predecessorBytes));
 assert.equal(predecessor.browser_measured,false);assert.equal(predecessor.capacity_qualified,false);
});
test('all40 current sources and exact successor metadata reject unknown or historical input',()=>{
 const sources=new Map(Object.keys(association.sourcePins).map(file=>[file,fs.readFileSync(path.join(root,file))]));
 for(const modified of sources.keys())assert.throws(()=>validateCurrentAssociation(associationBytes,file=>file===modified?Buffer.concat([sources.get(file),Buffer.from(' ')]):sources.get(file)),/Unknown current source bytes/);
 assert.throws(()=>validateCurrentAssociation(predecessorBytes),/Unknown current source-association bytes/);
 for(const mutate of [value=>value.sourceCommit=predecessor.sourceCommit,value=>value.browser_measured=true,value=>delete value.added_runtime_sources[addedRuntimeSources[0]],value=>value.changes_from_predecessor[Object.keys(value.changes_from_predecessor)[0]].reason+='unknown',value=>value.sourcePins.unknown='0'.repeat(64)]){
  const value=structuredClone(association);mutate(value);assert.throws(()=>validateCurrentAssociation(Buffer.from(JSON.stringify(value,null,2)+'\n')),/Unknown current source-association bytes/);
 }
});
