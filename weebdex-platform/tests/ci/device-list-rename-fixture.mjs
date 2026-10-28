// Actual DeviceStore.open, production IDB requests/transactions and mutation /
// restore modules on pinned fake-indexeddb6.2.4. No atomic-model substitution.
// This is IndexedDB-adapter evidence, not native-browser disk/crash acceptance.
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import test from 'node:test';import assert from 'node:assert/strict';import {stripTypeScriptTypes} from 'node:module';import {fileURLToPath,pathToFileURL} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),dependencies=process.env.WEEBDEX_MIGRATION_TEST_NODE_MODULES,noble=process.env.WEEBDEX_MIGRATION_TEST_NOBLE;
assert(dependencies&&path.isAbsolute(dependencies));assert(noble&&path.isAbsolute(noble));assert.equal(JSON.parse(fs.readFileSync(path.join(dependencies,'fake-indexeddb/package.json'))).version,'6.2.4');
const fake=await import(pathToFileURL(path.join(dependencies,'fake-indexeddb/build/esm/index.js'))),{sha256}=await import(pathToFileURL(path.join(noble,'sha2.js'))),indexedDB=new fake.IDBFactory();
const context=vm.createContext({console,URL,TextEncoder,TextDecoder,Uint8Array,ArrayBuffer,structuredClone,Date,crypto,Proxy,indexedDB,IDBKeyRange:fake.IDBKeyRange,navigator:{}}),cache=new Map();
function load(file){if(cache.has(file))return cache.get(file);const source=fs.readFileSync(file,'utf8'),module=new vm.SourceTextModule(file.endsWith('.ts')?stripTypeScriptTypes(source,{mode:'transform'}):source,{context,identifier:file});cache.set(file,module);return module;}
function resolve(specifier,importer){if(specifier==='@noble/hashes/sha2.js'){if(!cache.has(specifier))cache.set(specifier,new vm.SyntheticModule(['sha256'],function(){this.setExport('sha256',sha256);},{context}));return cache.get(specifier);}assert(specifier.startsWith('.'),'Unexpected adapter dependency');return load(path.resolve(path.dirname(importer.identifier),specifier+(path.extname(specifier)?'':'.ts')));}
async function namespace(relative){const module=load(path.join(root,'platform-integrations',relative));if(module.status==='unlinked')await module.link(resolve);if(module.status!=='evaluated')await module.evaluate();return module.namespace;}

export const {editDeviceListMember,orderedListMembers}=await namespace('local-library/device-list-members.ts');
const {previewDeviceListName,renameDeviceList}=await namespace('local-library/device-list-name.ts');
const {DeviceStore}=await namespace('local-library/device-store.ts'),migration=await namespace('public-api/device-id-migration.ts'),core=await namespace('public-api/device-write-v2.ts'),{request,transaction}=await namespace('local-library/protocol/indexeddb.js');
const manga='11111111-1111-8111-8111-111111111111',chapter='22222222-2222-8222-8222-222222222222',other='33333333-3333-8333-8333-333333333333',list='a'.repeat(32),namespaceId='55555555-5555-8555-8555-555555555555',service='b'.repeat(64),clone=value=>JSON.parse(JSON.stringify(value));
const series=(id=manga,updated_at=100)=>({family:'series',key:id,id,status:'reading',notification:true,followedAt:'2026-10-01T00:00:00Z',updated_at}),progress=(mangaId=manga)=>({family:'chapters',key:chapter,id:chapter,mangaId,completed:false,page:5,readAt:'2026-10-01T00:01:00Z',updated_at:101}),listRow={family:'lists',key:list,id:list,name:'Owned synthetic list',updated_at:99},member={family:'members',key:list+':'+manga,list,series:manga,position:0,updated_at:102};
let sequence=0;
async function fixture(){const scope={origin:'https://owned-v2-'+sequence+++'.example',serviceKey:null,catalogNamespace:namespaceId},opened=[];let closed=0;const open=async(selected=scope,options={idFormat:'public_uuid-v2'})=>{const store=await DeviceStore.open(selected,()=>{},()=>{closed++;},options);opened.push(store);return store;};const device=await open(),name=device.db.name;return{scope,device,name,open,closed:()=>closed,async dispose(){for(const store of opened){store.db.close();store.channel?.close();}await request(indexedDB.deleteDatabase(name));}};}
function stable(snapshot){const {exported_at,...body}=clone(snapshot);return body;}

export { previewDeviceListName, renameDeviceList, fixture, list, listRow, member, series, progress, service, namespaceId, other, stable, clone, request, transaction, indexedDB };
