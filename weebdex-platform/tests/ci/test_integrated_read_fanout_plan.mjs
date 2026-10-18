// Source/recipe checks only. No app, DOM, browser, database, socket or load run.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
import {createReaderOverlays} from '../../platform-integrations/frontend-reader-overlays.mjs';
import {createLocalLibraryOverlays,createLocalLibraryResolver} from '../../platform-integrations/local-library/overlays.mjs';
import {createPublicApiOverlays} from '../../platform-integrations/public-api/overlays.mjs';
import {createPublicUploadOverlays} from '../../platform-integrations/public-api/upload-overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const frontend=path.join(root,'weebdex-platform/frontend');
const plan=JSON.parse(fs.readFileSync(path.join(root,'weebdex-platform/docs/fixtures/integrated-read-fanout-v1.json')));
const hash=data=>createHash('sha256').update(data).digest('hex');
const plugins=[createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend),createPublicApiOverlays(frontend,{enabled:true}),createPublicUploadOverlays(frontend,{enabled:true})];
function composed(relative){const id=path.join(frontend,relative);let code=fs.readFileSync(id,'utf8');for(const plugin of plugins){const result=plugin.transform.handler(code,id);if(result)code=result.code;}return code;}
const scenario=id=>{const s=plan.scenarios.find(s=>s.id===id);assert.ok(s,id);return s;};

test('plan stays source-pinned and explicitly unmeasured/unadmitted',()=>{
 assert.equal(plan.sourceCommit,'5cbd05d91c38e22eb6a7509aa6e9f8c21b12e3b4');
 for(const [file,pin]of Object.entries(plan.sourcePins))assert.equal(hash(fs.readFileSync(path.join(root,file))),pin,file);
 assert.equal(plan.dispatch_ready,false);assert.equal(plan.browser_measured,false);assert.equal(plan.capacity_qualified,false);assert.equal(plan.default_api_calls_per_page,null);
});
test('every explicit recipe accounts all actual attempts and names reviewed transport lanes',()=>{
 const ids=new Set();for(const s of plan.scenarios){assert(!ids.has(s.id));ids.add(s.id);assert.equal(s.logical_actions,1);assert.equal(s.conditional_lanes_separate,true);assert.equal(s.expected_http_attempts,s.steps.reduce((n,step)=>{assert(plan.endpoints[step.endpoint]);assert(Number.isSafeInteger(step.attempts)&&step.attempts>0);return n+step.attempts;},0));}
 for(const row of Object.values(plan.endpoints))assert(['setup','catalogue','reader','media','analytics'].includes(row.lane));
 assert.equal(plan.endpoints.forum_context.method,'POST');assert.equal(plan.endpoints.forum_context.effect,'read');assert.equal(plan.accounting.read_only_post_queries_consume_permit,true);
});
test('UUID detail and every mounted selector recipe retain actual conditional fanout',()=>{
 assert.equal(scenario('uuid-chapter-detail').expected_http_attempts,4);assert.equal(scenario('uuid-reader-cold-three-chapters').expected_http_attempts,8);assert.equal(scenario('uuid-reader-cold-101-chapters').expected_http_attempts,11);assert.equal(scenario('uuid-reader-same-title-new-mount').expected_http_attempts,8);
 const api=composed('src/lib/API/Chapter.ts');assert(api.includes('publicCatalogueRead<Chapter>("chapter.get"'));assert(api.includes('"chapter.aggregate"'));
 const provider=composed('src/lib/components/reader/reader-provider.svelte');assert(!provider.includes('lastMangaId'));assert(provider.includes('Every mounted reader rechecks current authority'));assert(provider.includes('API.Chapter.Aggregate(mangaId, q)'));
});
test('mounted media uses actual at-home URL and initial plus three retries',()=>{
 const queue=composed('src/lib/components/reader/reader-queue.svelte');assert(queue.includes('retainedPageUrl('));assert(queue.includes('const maxAttempts = 3'));assert(queue.includes('attempts < maxAttempts'));assert(queue.includes('load(page, attempts + 1)'));assert(queue.includes('$readerPrefs.preload'));assert(queue.includes('$readerPrefs.parallel'));
 assert.equal(scenario('uuid-reader-image-failure-cycle').expected_http_attempts,4);assert.equal(scenario('uuid-reader-image-failure-cycle').outcome,'expected-image-failure');
});
test('cold tags are paginated while a nonempty grouped cache is reusable',()=>{
 const code=composed('src/lib/stores/global.ts');assert(code.includes('Object.keys(get(tagList) ?? {}).length'));assert(code.includes('page <= 100'));assert(code.includes('query.set("limit", "100")'));assert(code.includes('tags.length >= res.body.total'));
 assert.equal(scenario('uuid-search-warm-tags').expected_http_attempts,1);assert.equal(scenario('uuid-search-cold-101-tags').expected_http_attempts,3);
});
test('library resolver and candidate map preserve device-global hydration rather than displayed page size',()=>{
 const resolver=createLocalLibraryResolver(frontend,path.join(root,'weebdex-platform/platform-integrations/local-library'));
 assert.equal(resolver.resolveId('$lib/API/MangaFollow'),path.join(root,'weebdex-platform/platform-integrations/local-library/MangaFollow.ts'));
 assert.equal(scenario('uuid-library-empty').expected_http_attempts,0);assert.equal(scenario('uuid-library-101-saved').expected_http_attempts,2);
 const code=fs.readFileSync(path.join(root,'weebdex-platform/platform-integrations/public-api/library-catalogue.ts'),'utf8');assert(code.includes('if(!ids.length)return []'));assert(code.includes('Math.ceil(unique.length/100)'));assert(code.includes("query.set('offset',String(offset))"));
});
test('mounted history and typed view observation remain separate destinations',()=>{
 const history=composed('src/lib/components/reader/reader-history.svelte');assert(history.includes('timer = window.setTimeout(() => flush(), 350)'));assert(history.includes('saveProgress('));
 const analytics=composed('src/lib/components/reader/reader-analytics.svelte');assert(analytics.includes('public-view:chapter:'));assert(analytics.includes('response?.status === 204'));assert(analytics.includes('1000'));
 for(const id of ['uuid-reader-loaded-page-turn','device-progress-many-local-saves'])assert.equal(scenario(id).expected_http_attempts,0);
 assert.equal(plan.accounting.default_device_progress_http_writes,0);assert.equal(plan.endpoints.chapter_view.effect,'analytics-write');
});
