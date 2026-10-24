#!/usr/bin/env node
// Read-only consumer of a completed strict generation AND its actual outer
// qualification receipts. Missing provenance is an error, never an inferred pass.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {frontendRouteAdditions} from '../../platform-integrations/frontend-route-additions.mjs';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
import {createReaderOverlays} from '../../platform-integrations/frontend-reader-overlays.mjs';
import {createLocalLibraryOverlays} from '../../platform-integrations/local-library/overlays.mjs';
import {createPublicApiOverlays} from '../../platform-integrations/public-api/overlays.mjs';
import {createBoundedReaderOverlays,boundedReaderOverlayManifest,boundedReaderApprovedMarkupExceptions,assertBoundedReaderExceptionScope} from '../../platform-integrations/bounded-reader/overlays.mjs';
import {assertBadgeExceptionScope} from '../../platform-integrations/bounded-reader/badge-overlays.mjs';
import {createPublicUploadOverlays,publicUploadOverlayManifest} from '../../platform-integrations/public-api/upload-overlays.mjs';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),repo=path.dirname(root);
assert.ok(process.argv[2]&&process.argv[3],'Usage: test_composed_semantic_projection.mjs GENERATION OUTER_QUALIFICATION');
const work=path.resolve(process.argv[2]),outer=path.resolve(process.argv[3]),evidence=path.join(work,'evidence');
const read=(base,name)=>JSON.parse(fs.readFileSync(path.join(base,name)));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const same=(actual,expected,message)=>assert.deepEqual([...actual].sort(),[...expected].sort(),message);
const below=(file,directory)=>file===directory||file.startsWith(directory+path.sep);
const inside=(base,relative)=>{assert.equal(typeof relative,'string');const file=path.resolve(base,relative);assert.ok(below(file,base),'Artifact path escapes its generation');return file;};
const git=prefix=>{const x=spawnSync('git',['-c','core.hooksPath=/dev/null','-c','core.fsmonitor=false','-c','gc.auto=0','-c','maintenance.auto=false','-c','commit.gpgSign=false','-c','tag.gpgSign=false','-c','rerere.enabled=false','ls-files','-z','--',...prefix],{cwd:root,encoding:'utf8',env:{...process.env,GIT_OPTIONAL_LOCKS:'0'}});assert.equal(x.status,0);return x.stdout.split('\0').filter(Boolean);};
const summary=read(evidence,'summary.json'),manifest=read(evidence,'projection-manifest.json'),graph=read(evidence,'production-module-graph.json'),diagnostics=read(evidence,'diagnostics.json');
const canonical=read(evidence,'canonical-source-hashes.json');
assert.equal(summary.canonical_frontend_files,679);assert.equal(summary.canonical_frontend_unchanged,true);
assert.equal(Object.keys(canonical).length,679,'Actual canonical inventory must contain 679 records');
const matchingGates=[0,1].filter(gate=>hash(fs.readFileSync(path.join(outer,'strict-'+gate+'-summary.json')))===hash(fs.readFileSync(path.join(evidence,'summary.json'))));
assert.equal(matchingGates.length,1,'Generation must match exactly one actual outer strict summary');

// Tracked identities alone do not qualify a sparse checkout as full preparation.
// Every one of the 679 frontend and 263 integration inputs must physically exist.
const currentFrontend=git(['frontend']),currentIntegrations=git(['platform-integrations']);
assert.equal(currentFrontend.length,679);assert.equal(currentIntegrations.length,263);
for(const relative of [...currentFrontend,...currentIntegrations]){
 const file=inside(root,relative);assert.ok(fs.lstatSync(file).isFile(),'Physical source file missing or symlinked: '+relative);
 assert.ok(below(fs.realpathSync(file),fs.realpathSync(root)),'Physical source escapes current product root: '+relative);
}
same(Object.keys(canonical),currentFrontend,'Canonical inventory identities must equal tracked immutable sources');
for(const [file,digest] of Object.entries(canonical)){
 assert.equal(hash(fs.readFileSync(inside(root,file))),digest,file);
 const snapshot=inside(work,'production-input/'+file.slice('frontend/'.length));
 assert.ok(fs.lstatSync(snapshot).isFile(),'Every canonical input needs a physical copied snapshot: '+file);
 assert.ok(below(fs.realpathSync(snapshot),fs.realpathSync(work)),'Canonical snapshot escapes its generation: '+file);
 assert.equal(hash(fs.readFileSync(snapshot)),digest,'Full canonical input snapshot: '+file);
}
const products=read(outer,'strict-product-source-pins.json');
const currentProducts=git(['frontend','platform-integrations','scripts','contracts']).map(file=>'weebdex-platform/'+file);
same(Object.keys(products),currentProducts,'Outer product inventory must cover every current production input');
for(const [file,digest] of Object.entries(products))assert.equal(hash(fs.readFileSync(inside(repo,file))),digest,'Current product source differs: '+file);
assert.equal(summary.production_configuration_sha256,hash(fs.readFileSync(path.join(root,'scripts/vite.platform.config.mjs'))));
// Exact manifest completeness comes from immutable source identities, reviewed
// added routes and the current production graph, not a historical map count.
const sourcePaths=Object.keys(canonical).filter(file=>file.startsWith('frontend/src/')).map(file=>file.slice('frontend/'.length));
for(const addition of frontendRouteAdditions)if(addition.target.startsWith('src/')&&!sourcePaths.includes(addition.target))sourcePaths.push(addition.target);
assert.ok(Array.isArray(manifest)&&manifest.length>0);
same(manifest.filter(x=>x.projection.startsWith('src/')).map(x=>x.projection),sourcePaths,'Every actual frontend source must have one manifest entry');
assert.equal(new Set(manifest.map(x=>x.projection)).size,manifest.length,'Projection source identities must be unique');
const inactive=['src/lib/API/ChapterHistory.ts','src/lib/API/MangaFollow.ts'];same(summary.redirected_inactive_source_roots,inactive,'Only the genuine production resolver redirects are inactive');
const mapped=manifest.filter(x=>x.source_map);
const expectedMapped=[...sourcePaths.filter(file=>/\.(svelte|[cm]?[jt]s)$/.test(file)&&!inactive.includes(file)),...summary.reachable_integration_files];
same(mapped.map(x=>x.projection),expectedMapped,'Mapped identities must exactly cover all active frontend and reachable integration sources');
assert.ok(mapped.some(x=>x.projection==='platform-integrations/frontend-behavior/actor-follow-state.ts'));
assert.ok(graph.length>0,'Source graph must not be empty');assert.equal(graph.filter(x=>x.resolver.startsWith('unresolved')).length,0);
const byProjection=new Map(manifest.map(x=>[x.projection,x]));
// Reexecute only the reviewed pure transform chain, without Vite/Svelte/build.
// Product pins alone cannot prove a claimed composed source is its real output.
const input=path.join(work,'production-input'),enabled=matchingGates[0]===1;
// Only physical current sources participate here. No historical reconstruction,
// normalized graph, relabelled snapshot or old five-stage receipt is admissible.
const boundedDependencyPins={
 'overlays.mjs':'a7d89c85f2fdf435c8606f82c5519e532c2717fed698e72fcca81eaaf201c4aa',
 'anchors.json':'feafb34479f4d81413f44cc24d786beb431e9bebffc6d6be24de1753139d2fae',
 'badge-overlays.mjs':'e36bdf2b2e00c6e6a485f3fbc7d6f63cf05c59f59c4847e87854cdf3561bb960',
 'badge-anchors.json':'a81b80af1b637794ea23834b713ee06c4bfa5161a009708ef01f7badebea0ca4'
};
for(const [file,digest] of Object.entries(boundedDependencyPins))assert.equal(hash(fs.readFileSync(path.join(root,'platform-integrations/bounded-reader',file))),digest,'Current combined sixth-stage dependency: '+file);
const readerPatches=read(root,'platform-integrations/bounded-reader/anchors.json'),badgePatches=read(root,'platform-integrations/bounded-reader/badge-anchors.json');
assert.equal(readerPatches.length,34);assert.equal(readerPatches.filter(patch=>patch.markup).length,12);
assert.equal(badgePatches.length,16);assert.equal(badgePatches.filter(patch=>patch.markup).length,5);
assert.deepEqual(boundedReaderOverlayManifest,[...readerPatches,...badgePatches].map(({before,after,...metadata})=>metadata));
assert.deepEqual(boundedReaderApprovedMarkupExceptions,[...readerPatches,...badgePatches].filter(patch=>patch.markup));
assertBadgeExceptionScope();assertBoundedReaderExceptionScope();
const boundedTargets=new Set(boundedReaderOverlayManifest.map(patch=>patch.path));
for(const upload of publicUploadOverlayManifest)assert.equal(boundedTargets.has(upload.path),false,'Upload and reader/badge targets must stay disjoint');
const plugins=[createFrontendOverlays(input),createReaderOverlays(input),createLocalLibraryOverlays(input),createPublicApiOverlays(input,{enabled}),createPublicUploadOverlays(input,{enabled}),createBoundedReaderOverlays(input)];
assert.deepEqual(plugins.map(plugin=>plugin.name),['weebdex-reviewed-script-overlays','weebdex-reviewed-reader-overlays','weebdex-local-library-overlays','retained-public-uuid-catalogue-staging','recovered-public-signed-upload','bounded-live-semantic-reader']);
same(summary.production_overlay_order,plugins.map(plugin=>plugin.name),'Actual production transform set must agree');
assert.deepEqual(summary.production_overlay_order,plugins.map(plugin=>plugin.name),'Actual production transform order must agree');
for(const entry of manifest){
 if(!entry.projection.startsWith('src/')){
  assert.equal(hash(fs.readFileSync(inside(root,entry.projection))),entry.production_composed_sha256,'Integration snapshot must equal current actual source');continue;
 }
 const installed=frontendRouteAdditions.find(addition=>addition.target===entry.projection),canonicalSource=path.join(root,'frontend',entry.projection);
 const source=fs.existsSync(canonicalSource)?canonicalSource:inside(root,installed?.source);
 const original=fs.readFileSync(source,'utf8'),id=path.join(input,entry.projection);
 assert.equal(hash(original),entry.original_sha256,'Actual transform input differs: '+entry.projection);
 assert.equal(hash(fs.readFileSync(id)),entry.original_sha256,'Actual production input snapshot differs: '+entry.projection);
 let code=original;const stages=[];
 if(/\.(svelte|[cm]?[jt]s)$/.test(id))for(const plugin of plugins){const transformed=plugin.transform.handler(code,id);if(transformed){code=transformed.code;const stage={plugin:plugin.name,sha256:hash(code),snapshot:'production-stages/'+plugin.name+'/'+entry.projection};stages.push(stage);assert.equal(hash(fs.readFileSync(inside(work,stage.snapshot))),stage.sha256,'Actual reviewed transform stage snapshot differs');}}
 assert.deepEqual(entry.stages,stages,'Actual reviewed transform stage identity differs');
 assert.equal(hash(code),entry.production_composed_sha256,'Actual reviewed transform output differs: '+entry.projection);
}

same(manifest.filter(entry=>entry.stages.some(stage=>stage.plugin==='bounded-live-semantic-reader')).map(entry=>entry.projection),boundedTargets,'Every real reader and badge target must retain its sixth-stage snapshot');
for(const relative of boundedTargets){
 const entry=byProjection.get(relative);assert.ok(entry);assert.equal(entry.canonical,relative);
 assert.equal(entry.stages.filter(stage=>stage.plugin==='bounded-live-semantic-reader').length,1);
 assert.equal(entry.stages.at(-1).plugin,'bounded-live-semantic-reader');
 assert.equal(entry.stages.at(-1).sha256,entry.production_composed_sha256);
}
for(const upload of publicUploadOverlayManifest){
 const entry=byProjection.get(upload.path);assert.ok(entry);
 assert.equal(entry.stages.some(stage=>stage.plugin==='bounded-live-semantic-reader'),false);
 if(enabled){assert.equal(entry.stages.at(-1).plugin,'recovered-public-signed-upload');assert.equal(entry.production_composed_sha256,upload.output_sha256);}
 else assert.equal(entry.production_composed_sha256,upload.composed_sha256);
}
if(!enabled)for(const entry of manifest)assert.equal(entry.stages.some(stage=>['retained-public-uuid-catalogue-staging','recovered-public-signed-upload'].includes(stage.plugin)),false,'Public stages remain gated independently of the unconditional sixth stage');
// The full physical integration closure must exist even for non-reachable files.
// Reachable projections retain their own exact recorded projection hashes.
for(const relative of currentIntegrations){
 const digest=hash(fs.readFileSync(inside(root,relative)));
 const snapshot=inside(work,'production-composed/'+relative),projection=inside(work,'project/'+relative);
 for(const file of [snapshot,projection]){assert.ok(fs.lstatSync(file).isFile(),'Physical integration copy missing or symlinked: '+relative);assert.ok(below(fs.realpathSync(file),fs.realpathSync(work)),'Integration copy escapes its generation: '+relative);}
 assert.equal(hash(fs.readFileSync(snapshot)),digest,'Current physical integration snapshot: '+relative);
 assert.equal(hash(fs.readFileSync(projection)),byProjection.get(relative)?.projection_sha256??digest,'Current physical integration projection: '+relative);
}

for(const entry of manifest){
 assert.match(entry.projection,/^(src|platform-integrations)\//);assert.ok(!entry.projection.includes('node_modules'));
 assert.equal(hash(fs.readFileSync(inside(work,entry.production_snapshot))),entry.production_composed_sha256,'Actual composed snapshot: '+entry.projection);
 const output=inside(work,'project/'+entry.projection);
 assert.equal(hash(fs.readFileSync(output)),entry.projection_sha256??entry.production_composed_sha256,'Actual projection: '+entry.projection);
 if(!entry.source_map)continue;
 const mapPath=inside(work,entry.source_map),map=read(work,entry.source_map);
 assert.equal(hash(fs.readFileSync(mapPath)),entry.source_map_sha256);assert.equal(map.sources.length,1);assert.equal(map.sourcesContent.length,1);
 const original=inside(work,path.relative(work,path.resolve(path.dirname(mapPath),map.sources[0])));
 assert.equal(original,inside(work,entry.production_snapshot));assert.equal(hash(fs.readFileSync(original)),entry.production_composed_sha256);
 assert.equal(hash(map.sourcesContent[0]),entry.production_composed_sha256);
 assert.equal(path.resolve(path.dirname(mapPath),map.file),output);
}
const mapValidation=read(evidence,'source-map-validation.json');assert.equal(mapValidation.status,'passed');assert.equal(mapValidation.validated_maps,mapped.length);
same(mapValidation.maps.map(x=>x.map),mapped.map(x=>x.source_map),'Map validation identities must cover every manifest map');
for(const item of mapValidation.maps){const entry=mapped.find(x=>x.source_map===item.map);assert.equal(item.source_sha256,entry.production_composed_sha256);assert.equal(hash(fs.readFileSync(inside(work,item.source))),item.source_sha256);assert.equal(item.embedded_content_matches,true);}
// Guard receipts must cover the real physical package roots, including targets
// behind a container of package symlinks. A pinned symlink is not pinned bytes.
const discovery=read(outer,'physical-dependency-root-discovery.json');
const before=read(outer,'physical-dependencies-before-summary.json'),after=read(outer,'physical-dependencies-after-summary.json');
assert.ok(Object.keys(before).length>0,'Actual dependency inventory must not be empty');assert.deepEqual(before,after,'Physical dependency summaries changed');
same(Object.keys(before),discovery.roots,'Physical dependency discovery must cover every inventory root');assert.deepEqual(discovery.uncoveredSymlinks,[]);
const dependencyRoot=file=>{const real=fs.realpathSync(file),marker=real.indexOf('/node_modules/');assert.ok(marker>=0,'Expected actual dependency path');return real.slice(0,marker+'/node_modules'.length);};
const requiredRoots=new Set();
for(const edge of graph)if(path.isAbsolute(edge.target)&&edge.target.includes('/node_modules/'))requiredRoots.add(dependencyRoot(edge.target));
for(const item of summary.declaration_entrypoints_for_runtime_package_aliases){requiredRoots.add(dependencyRoot(item.same_package_declaration));assert.equal(hash(fs.readFileSync(item.same_package_declaration)),item.declaration_sha256);}
for(const name of ['svelte-kit-sync','svelte-check','service-worker-tsc'])for(const value of read(evidence,name+'-receipt.json').command)if(path.isAbsolute(value)&&value.includes('/node_modules/'))requiredRoots.add(dependencyRoot(value));
assert.ok(requiredRoots.size>0);
for(const actualRoot of requiredRoots)assert.ok(Object.hasOwn(before,actualRoot),'Missing physical dependency root inventory: '+actualRoot);
const auditSummary=read(outer,'cache-audit-summary-physical.json');
const strictRecords=auditSummary.recordsManifest.filter(x=>x.file.startsWith('cache-audit/strict-physical/'));
assert.ok(strictRecords.length>0,'At least one actual strict cache-guard audit is required');
const actualAuditFiles=fs.readdirSync(path.join(outer,'cache-audit/strict-physical')).filter(file=>file.endsWith('.json')).map(file=>'cache-audit/strict-physical/'+file);
same(strictRecords.map(x=>x.file),actualAuditFiles,'Strict audit manifest must cover every actual process record');
assert.equal(new Set(strictRecords.map(x=>x.pid)).size,strictRecords.length,'Strict process identities must be unique');
const resource=read(outer,'strict-resource-receipt.json');assert.equal(resource.exitCode,0);assert.equal(resource.stopReason,null);assert.deepEqual(resource.ownedSurvivors,[]);assert.ok(resource.samples.length>0);
assert.equal(resource.status,'passed','Actual strict owner must pass');
assert.deepEqual(resource.cleanup,{verified_stopped:true,forced:false,remaining_owned:0,unknown_births:0,tracking_errors:0,stdout_eof:true},'Actual strict owner needs unforced, certain cleanup and terminal EOF');
for(const sample of resource.samples)for(const row of sample.owned)assert.ok(Number.isSafeInteger(row.birth)&&row.birth>0,'Owned strict samples require exact process birth identities');
const observedPids=new Set(resource.samples.flatMap(sample=>sample.owned.map(row=>row.pid)));
let activity=0;
for(const record of strictRecords){
 const file=inside(outer,record.file),audit=read(outer,record.file);assert.equal(hash(fs.readFileSync(file)),record.sha256,'Actual guard receipt digest');
 assert.equal(audit.pid,record.pid);assert.equal(audit.protected_roots.length,record.roots);assert.ok(observedPids.has(audit.pid),'Guard process must occur in actual owned strict resource samples');
 assert.ok(Array.isArray(audit.protected_roots)&&audit.protected_roots.length>0,'Each strict audit must declare physical cache roots');
 assert.equal(new Set(audit.protected_roots).size,audit.protected_roots.length);
 for(const required of requiredRoots)assert.ok(audit.protected_roots.includes(required),'Each strict audit must protect every actual physical dependency root: '+required);
 for(const declared of audit.protected_roots)assert.ok(Object.hasOwn(before,declared),'Every declared guard root needs a before/after inventory');
 assert.ok(Number.isInteger(audit.checked_node_mutations)&&audit.checked_node_mutations>=0);assert.equal(audit.checked_node_mutations,record.checkedMutations);activity+=audit.checked_node_mutations;
 assert.equal(audit.protected_write_attempts,0);assert.deepEqual(audit.denied,[]);
}
assert.ok(activity>0,'Actual strict guard must observe filesystem activity');
// Python retains mtimeNs as an exact integer. JSON.parse would round these
// nanosecond values above Number.MAX_SAFE_INTEGER, weakening metadata checks.
const physical=spawnSync(process.env.PYTHON??'python3',[path.join(root,'tests/ci/semantic_dependency_snapshot.py'),outer],{encoding:'utf8',timeout:30000,maxBuffer:1024*1024});
assert.ok(!physical.error,'Physical dependency verifier failed to execute');assert.equal(physical.status,0,'Exact physical dependency snapshot verification failed: '+physical.stderr);
const physicalResult=JSON.parse(physical.stdout);assert.equal(physicalResult.status,'passed');assert.equal(physicalResult.exactNanoseconds,true);assert.equal(physicalResult.byteModeMtimeSymlinkEquality,true);
// Verify graph literal completeness independently of its historical node count.
const tsc=summary.service_worker_receipt.command.find(value=>value.endsWith('/typescript/bin/tsc'));assert.ok(tsc);const require=createRequire(path.join(path.dirname(tsc),'../package.json')),ts=require('./lib/typescript.js');
const aliases=summary.production_aliases;
const boundedAliases=aliases.filter(alias=>alias.find==='$boundedReader');
assert.equal(boundedAliases.length,1,'The real production bounded-reader alias is required');
assert.equal(boundedAliases[0].replacement,path.join(root,'platform-integrations/bounded-reader'));
assert.ok(fs.readFileSync(path.join(root,'scripts/vite.platform.config.mjs'),'utf8').includes('"$boundedReader": path.join(root, "platform-integrations/bounded-reader")'));
const boundedAliasEdges=graph.filter(edge=>edge.specifier.startsWith('$boundedReader/'));
assert.ok(boundedAliasEdges.length>0,'Current reader and badge imports must be present in the actual graph');
for(const edge of boundedAliasEdges){assert.equal(edge.resolver,'production-alias-or-relative');assert.ok(edge.target.startsWith('platform-integrations/bounded-reader/'));}

function included(specifier){return specifier.startsWith('.')||specifier==='$lib'||specifier.startsWith('$lib/')||aliases.some(alias=>typeof alias.find==='string'?(specifier===alias.find||specifier.startsWith(alias.find+'/')):new RegExp(alias.find.regularExpression,alias.find.flags).test(specifier));}
for(const entry of mapped){
 const code=fs.readFileSync(inside(work,entry.production_snapshot),'utf8'),ranges=entry.projection.endsWith('.svelte')?[...code.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)].map(x=>x[1]):[code],references=[];
 for(const range of ranges){const ast=ts.createSourceFile(entry.projection+'.ts',range,ts.ScriptTarget.Latest,true);function visit(node){let literal;if((ts.isImportDeclaration(node)||ts.isExportDeclaration(node))&&node.moduleSpecifier&&ts.isStringLiteral(node.moduleSpecifier))literal=node.moduleSpecifier;if(ts.isCallExpression(node)&&node.expression.kind===ts.SyntaxKind.ImportKeyword&&ts.isStringLiteral(node.arguments[0]))literal=node.arguments[0];if(ts.isNewExpression(node)&&ts.isIdentifier(node.expression)&&node.expression.text==='URL'&&ts.isStringLiteral(node.arguments?.[0])&&node.arguments?.[1]?.getText(ast)==='import.meta.url')literal=node.arguments[0];if(literal&&included(literal.text))references.push(literal.text);ts.forEachChild(node,visit);}visit(ast);}
 same(graph.filter(x=>x.importer===entry.projection).map(x=>x.specifier),references,'Graph must cover every actual resolved import literal: '+entry.projection);
}
const graphIntegrations=new Set();
for(const edge of graph){assert.ok(byProjection.has(edge.importer)&&byProjection.get(edge.importer).source_map,'Graph importer needs an active map');if(!path.isAbsolute(edge.target)){assert.ok(byProjection.has(edge.target),'Graph target must have a manifest entry');if(edge.target.startsWith('platform-integrations/')&&/\.(svelte|[cm]?[jt]s)$/.test(edge.target))graphIntegrations.add(edge.target);}if(edge.importer.startsWith('platform-integrations/'))graphIntegrations.add(edge.importer);}
same(summary.reachable_integration_files,graphIntegrations,'Reachable integration identities must equal the actual production graph');
for(const relative of graphIntegrations)assert.ok(currentIntegrations.includes(relative),'Reachable module must be a physical current integration input: '+relative);
for(const file of ['controller.ts','runtime.svelte.ts','client.ts','history.ts','endpoint.ts','visible-badges.svelte.ts','visible-badges.ts','TitleReadingActions.svelte','ChapterSelector.svelte','EditionSelector.svelte','GroupPage.svelte','EditionPage.svelte','Status.svelte']){
 const relative='platform-integrations/bounded-reader/'+file;assert.ok(graphIntegrations.has(relative),'Current bounded reader/badge integration must be reachable: '+relative);
 const entry=byProjection.get(relative);assert.ok(entry?.source_map);assert.equal(entry.production_composed_sha256,hash(fs.readFileSync(inside(root,relative))));
}
assert.equal(summary.source_diagnostic_suppressions_added,false);assert.equal(summary.strict,true);assert.equal(summary.checkJs,true);assert.equal(summary.status,'passed');assert.equal(summary.errors,0);
assert.equal(summary.service_worker_receipt.exit_code,0);assert.equal(summary.svelte_check_receipt.exit_code,0);
assert.equal(diagnostics.filter(x=>x.severity==='error').length,summary.errors);assert.equal(diagnostics.filter(x=>x.severity==='warning').length,summary.warnings);assert.equal(diagnostics.filter(x=>/Cannot find module/.test(x.message)).length,0);
same(summary.worker_platform_type_bindings.map(x=>x.file),['platform-integrations/local-library/projection-worker.ts','platform-integrations/local-library/protocol/worker.js']);
assert.ok(fs.readFileSync(path.join(work,'project/platform-integrations/local-library/projection-worker.ts'),'utf8').includes('declare const self: DedicatedWorkerGlobalScope;'));assert.ok(fs.readFileSync(path.join(work,'project/platform-integrations/local-library/protocol/worker.js'),'utf8').includes('const __semanticWorkerSelf ='));
console.log(JSON.stringify({status:'passed',canonical_files:Object.keys(canonical).length,physical_integration_files:currentIntegrations.length,production_stages:plugins.length,bounded_reader_and_badge_targets:boundedTargets.size,product_inputs:Object.keys(products).length,valid_source_maps:mapped.length,graph_edges:graph.length,physical_dependency_roots:Object.keys(before).length,required_production_roots:requiredRoots.size,strict_process_audits:strictRecords.length,protected_node_write_attempts:0,semantic_check_status:summary.status,errors:summary.errors,warnings:summary.warnings,physical_dependency_entries:physicalResult.entries,qualification:'Exact byte/mode/mtimeNs/symlink snapshots plus in-process Node mutation denial; not OS syscall enforcement.'}));
