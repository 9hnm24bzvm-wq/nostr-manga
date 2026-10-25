// Same finite probes are overlaid on baseline and candidate exports. Existing
// publicQuery and coverMetadata APIs only: no import of a new language helper,
// browser, service, real signer, network, or build.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
const directory=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../platform-integrations/public-api');
const context=vm.createContext({URL,URLSearchParams,TextEncoder,TextDecoder,Uint8Array,ArrayBuffer,console}),modules=new Map();
async function load(file){if(!modules.has(file)){const m=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform',sourceUrl:file}),{context,identifier:file});modules.set(file,m);await m.link((specifier,importer)=>load(path.resolve(path.dirname(importer.identifier),specifier+'.ts')));}return modules.get(file);}
async function namespace(name){const m=await load(path.join(directory,name+'.ts'));await m.evaluate();return m.namespace;}
const query=await namespace('query'),cover=await namespace('cover-multipart');
const coverFields=locale=>({locale,volume:'',description:'',main:true});
test('counterfactual cover upload accepts ta outside native enum',()=>assert.equal(cover.coverMetadata(coverFields('ta')).locale,'ta'));
for(const code of ['tam','und','zh-hant','EN','en-US','en\n','en\r\n'])test('counterfactual cover upload rejects strict-write code '+JSON.stringify(code),()=>assert.throws(()=>cover.coverMetadata(coverFields(code))));
for(const family of ['manga','chapter','feed','aggregate']){
 const key=family==='manga'?'originalLanguage[]':'translatedLanguage[]';
 for(const code of ['ta','tam','EN','en-US','--'])test('counterfactual broader '+family+' filter accepts '+JSON.stringify(code),()=>assert.equal(query.publicQuery(family,new URLSearchParams([[key,code]])).get(key),code));
 for(const code of ['ta1','zh-hant','en\n','en\r\n'])test('counterfactual bounded '+family+' filter rejects '+JSON.stringify(code),()=>assert.throws(()=>query.publicQuery(family,new URLSearchParams([[key,code]]))));
}
test('counterfactual cover locale filter accepts strict ta',()=>assert.equal(query.publicQuery('cover',new URLSearchParams([['locales[]','ta']])).get('locales[]'),'ta'));
for(const code of ['tam','und','zh-hant','EN','en-US','en\n','en\r\n'])test('counterfactual cover locale filter rejects '+JSON.stringify(code),()=>assert.throws(()=>query.publicQuery('cover',new URLSearchParams([['locales[]',code]]))));
