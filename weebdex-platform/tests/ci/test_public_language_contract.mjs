// Source-bound finite contract checks. No browser, service, Rust build or keys.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { createVmModuleLoader } from './vm-module-fixture.mjs';
import { createPublicSearchModeFixture } from './public-search-mode-fixture.mjs';
import { title, editorFields, plain, id } from './title-edit-route-fixture.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
import { createPublicUploadOverlays } from '../../platform-integrations/public-api/upload-overlays.mjs';
const root=path.resolve(new URL('../../',import.meta.url).pathname),frontend=path.join(root,'frontend'),directory=path.join(root,'platform-integrations/public-api');
const context=vm.createContext({URL,URLSearchParams,TextEncoder,TextDecoder,Uint8Array,ArrayBuffer,structuredClone,console});
const searchMode=createPublicSearchModeFixture(context);
const loader=createVmModuleLoader({
 createModule(file,importModuleDynamically){return searchMode.module(file)??new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform'}),{context,identifier:file,importModuleDynamically});},
 resolve(specifier,importer){return searchMode.resolve(specifier,importer)??(specifier.startsWith('$lib/')?path.join(frontend,'src/lib',specifier.slice(5)+'.ts'):path.resolve(path.dirname(importer.identifier),specifier+'.ts'));}
});
const namespace=relative=>loader.namespace(path.join(root,relative));
const language=await namespace('platform-integrations/public-api/language.ts'),query=await namespace('platform-integrations/public-api/query.ts'),cover=await namespace('platform-integrations/public-api/cover-multipart.ts'),publicSearch=await namespace('platform-integrations/public-api/manga-search-query.ts'),nativeSearch=await namespace('platform-integrations/frontend-behavior/manga-search-query.ts');
for(const code of ['en','ta','pt-br','zh-hk','aa-zz'])test('public write accepts syntax outside the native enum: '+code,()=>assert.equal(language.publicWriteLanguage(code),code));
for(const code of ['tam','und','zh-hant','EN','en-US',' ta','ta ','en\n','en\r\n','','e','abcd','en--us','en-1a',null])test('public write rejects, without normalization: '+String(code),()=>assert.throws(()=>language.publicWriteLanguage(code)));
for(const label of ['tam','und','zh-hant','EN'])test('unrelated title edits do not validate or rewrite imported '+label,()=>{
 assert.deepEqual(plain(title.titleMetadata({version:7,status:'completed'},label)),{version:7,status:'completed'});
 assert.deepEqual(plain(title.titleMetadata({version:7,title:'Changed'},label)),{version:7,title:{[label]:'Changed'}});
 assert.throws(()=>title.titleMetadata({version:7,language:label},label));
 assert.deepEqual(plain(title.titleMetadata({version:7,language:'ta'},label)),{version:7,originalLanguage:'ta'});
});
test('localized maps are bounded opaque keys, not a language registry',()=>{
 const map=JSON.parse('{"tam":["Tamil title"],"__proto__":["Opaque title"]}');
 assert.deepEqual(plain(title.titleMetadata({alt_titles:map})).altTitles,[{tam:'Tamil title'},JSON.parse('{"__proto__":"Opaque title"}')]);
});
for(const label of ['tam','und','zh-hant','EN'])test('chapter metadata patch omits unchanged imported '+label,()=>{
 const row={version:9,title:'Old',volume:null,chapter:'1',language:label,relationships:{groups:[]}},fields={title:'Edited',volume:'',chapter:'1',language:label,groups:[]};
 assert.deepEqual(plain(language.chapterMetadataPatch(row,fields)),{version:9,title:'Edited'});
 assert.deepEqual(plain(language.chapterMetadataPatch(row,{...fields,language:'ta'})),{version:9,title:'Edited',language:'ta'});
 assert.throws(()=>language.chapterMetadataPatch(row,{...fields,language:'en-US'}));assert.equal(row.language,label);
});
for(const label of ['tam','und','zh-hant','EN',null])test('cover patch preserves untouched imported '+label,()=>{
 const previous={locale:label,volume:'1',description:'Old',main:false},fields={...previous,locale:label??'',description:'Edited'};
 assert.deepEqual(plain(cover.coverMetadataPatch(fields,previous)),{description:'Edited'});
 assert.deepEqual(plain(cover.coverMetadataPatch({...fields,locale:'ta'},previous)),{locale:'ta',description:'Edited'});
 assert.throws(()=>cover.coverMetadataPatch({...fields,locale:'en-US'},previous));
});
test('cover upload and changed group focusedLanguages accept ta but reject tam',()=>{
 assert.equal(cover.coverMetadata({locale:'ta',volume:'',description:'',main:true}).locale,'ta');
 assert.throws(()=>cover.coverMetadata({locale:'tam',volume:'',description:'',main:true}));
 const row={id,type:'scanlation_group',attributes:{version:3,name:'Group',description:'Old',focusedLanguage:['zh-hant']},relationships:[]},fields=editorFields.editorialFields('group',row);
 fields.description='Edited';assert.deepEqual(plain(editorFields.editorialPayload('group',fields,row)),{version:3,description:'Edited'});
 fields.focusedLanguages='ta, pt-br';assert.deepEqual(plain(editorFields.editorialPayload('group',fields,row)).focusedLanguages,['ta','pt-br']);
 fields.focusedLanguages='tam';assert.throws(()=>editorFields.editorialPayload('group',fields,row));
});
for(const family of ['manga','chapter','feed','aggregate'])for(const code of ['ta','tam','und','EN','en-US','--'])test('broader '+family+' language filter preserves '+code,()=>{
 const key=family==='manga'?'originalLanguage[]':'translatedLanguage[]';assert.equal(query.publicQuery(family,new URLSearchParams([[key,code]])).get(key),code);
});
for(const code of ['ta1','zh-hant','e','','en\n','en\r\n'])test('malformed public search language rejects '+code,()=>assert.throws(()=>query.publicQuery('manga',new URLSearchParams([['lang',code]]))));
test('cover filter follows strict locale schema while focusedLanguage remains bounded opaque',()=>{
 assert.equal(query.publicQuery('cover',new URLSearchParams([['locales[]','ta']])).get('locales[]'),'ta');
 for(const code of ['tam','EN','en-US'])assert.throws(()=>query.publicQuery('cover',new URLSearchParams([['locales[]',code]])));
 for(const label of ['zh-hant','EN','native:label'])assert.equal(query.publicQuery('group',new URLSearchParams({focusedLanguage:label})).get('focusedLanguage'),label);
 assert.equal(query.publicQuery('group',new URLSearchParams({focusedLanguage:'x'.repeat(4096)})).get('focusedLanguage').length,4096);
 assert.throws(()=>query.publicQuery('group',new URLSearchParams({focusedLanguage:'x'.repeat(4097)})));
 assert.throws(()=>query.publicQuery('group',new URLSearchParams({focusedLanguage:'界'.repeat(1366)})));
});
test('public parser, preferences and read filters accept broader syntax without changing native defaults',()=>{
 for(const code of ['ta','tam','EN','en-US']){
  const input=new URLSearchParams([['lang',code],['availableTranslatedLang',code],['tlang',code]]);
  assert.equal(publicSearch.validateMangaSearchQuery(input,false),undefined);
  const parsed=publicSearch.mangaSearchReadQuery(input,false);assert.equal(parsed.error,undefined);assert.equal(parsed.query.get('lang'),code);
  const effective=publicSearch.effectiveMangaSearchReadQuery(new URLSearchParams(),false,{applyMangaPreferences:q=>q.append('lang',code)},'20');assert.equal(effective.error,undefined);assert.equal(effective.query.get('lang'),code);
 }
 assert.ok(nativeSearch.validateMangaSearchQuery(new URLSearchParams({lang:'ta'}),false));
 assert.equal(nativeSearch.validateMangaSearchQuery(new URLSearchParams({lang:'tam'}),false),undefined);
});
function composed(relative,enabled){const file=path.join(frontend,relative);let code=fs.readFileSync(file,'utf8');for(const plugin of [createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend),createPublicApiOverlays(frontend,{enabled}),createPublicUploadOverlays(frontend,{enabled})])code=plugin.transform.handler(code,file)?.code??code;return code;}
test('selected public controls name languages while native disabled gate retains its original selector',()=>{
 const upload='src/lib/components/upload/UploadChapterMetadata.svelte';assert.match(composed(upload,true),/<select aria-label="Language"[^>]+bind:value=\{draft.language\}/);assert.match(composed(upload,true),/Previously saved language/);assert.match(composed(upload,true),/Object.entries\(Locale.Languages\)/);assert.match(composed(upload,false),/Object.entries\(Locale.Languages\)/);
 const filter='src/lib/components/MangaSearchFilter.svelte';assert.match(composed(filter,true),/\$publicApi\/manga-search-query/);assert.match(composed(filter,true),/PublicLanguageFilter bind:value=\{search.lang\}/);assert.match(composed(filter,false),/\$platform\/manga-search-query/);assert.match(composed(filter,false),/Object.entries\(Locale.Languages\)/);
});
test('all supplied originals, including native Locale, remain exact attachment bytes',()=>{
 const manifest=JSON.parse(fs.readFileSync(path.join(root,'test-evidence/frontend/source-parity.json'),'utf8'));assert.equal(manifest.files.length,676);
 for(const entry of manifest.files)assert.equal(createHash('sha256').update(fs.readFileSync(path.join(frontend,entry.path))).digest('hex'),entry.source_sha256,entry.path);
});

test('named public search selections preserve exact codes, including unknown URL languages',()=>{
 const source=fs.readFileSync(path.join(directory,'PublicLanguageFilter.svelte'),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1].replace(/import[^;]+;/g,'');
 const ui=vm.runInNewContext(stripTypeScriptTypes(source,{mode:'transform'})+'\n({toggle,get:()=>value})',{$state:value=>value,$derived:value=>value,$bindable:value=>value,$props:Object.assign(()=>({value:['EN'],label:'Language'}),{id:()=>'finite-language-filter'}),Locale:{Languages:{en:'English',ja:'Japanese'}},searchLanguageCodes:language.searchLanguageCodes});
 ui.toggle('ja',true); assert.deepEqual(plain(ui.get()),['EN','ja']); ui.toggle('EN',false); assert.deepEqual(plain(ui.get()),['ja']);
});

for(const mode of ['device','account'])test('public list parser keeps '+mode+' IDs distinct without changing the native parser',()=>{
 searchMode.setMode(mode);
 try {
  const device='a'.repeat(32),account='11111111-1111-4111-8111-111111111111',accepted=mode==='device'?device:account,rejected=mode==='device'?account:device;
  for(const value of [accepted,rejected,accepted+'\n']){
   const q=new URLSearchParams({list:value,lang:'ta',limit:'100'}),ok=value===accepted;
   assert.equal(publicSearch.validateMangaSearchQuery(q,true)===undefined,ok);
   const parsed=publicSearch.parseMangaSearchQuery({},q,true);assert.equal(parsed.issues.length===0,ok);
   const read=publicSearch.mangaSearchReadQuery(q,true);assert.equal(read.error===undefined,ok);if(ok)assert.equal(read.query.get('list'),accepted);
   const effective=publicSearch.effectiveMangaSearchReadQuery(q,true,{applyMangaPreferences(){assert.fail('Private library must not apply discovery preferences');}},'20');assert.equal(effective.error===undefined,ok);
  }
  assert.ok(publicSearch.validateMangaSearchQuery(new URLSearchParams({list:accepted}),false),'Discovery cannot accept private-list filters');
  assert.ok(publicSearch.validateMangaSearchQuery(new URLSearchParams([['list',accepted],['list',accepted]]),true),'A list selection is scalar');
  assert.ok(publicSearch.validateMangaSearchQuery(new URLSearchParams({list:accepted,limit:'101'}),true),'Page size remains bounded');
  assert.equal(nativeSearch.validateMangaSearchQuery(new URLSearchParams({list:device}),true),undefined);
  assert.ok(nativeSearch.validateMangaSearchQuery(new URLSearchParams({list:account}),true),'Native parser has not gained canonical-account UUID semantics');
 } finally {searchMode.setMode('device');}
});
