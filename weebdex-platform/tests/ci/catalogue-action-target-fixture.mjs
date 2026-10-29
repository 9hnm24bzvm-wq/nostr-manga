// Real typed catalogue loader/action target helper; finite transport only.
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import {stripTypeScriptTypes} from 'node:module';import {fileURLToPath} from 'node:url';
export const root=fileURLToPath(new URL('../../',import.meta.url)),directory=path.join(root,'platform-integrations/public-api');
export const ids={manga:'11111111-1111-8111-8111-111111111111',group:'22222222-2222-8222-8222-222222222222',other:'33333333-3333-8333-8333-333333333333',alias:'44444444-4444-8444-8444-444444444444',namespace:'55555555-5555-8555-8555-555555555555',account:'66666666-6666-8666-8666-666666666666'};
export const legacy={manga:'LegacyManga1',group:'LegacyGroup1'};
const context=vm.createContext({URL,URLSearchParams,TextEncoder,TextDecoder,Uint8Array,AbortController,AbortSignal,Response,setTimeout,clearTimeout,console}),cache=new Map();
async function load(name){if(cache.has(name))return cache.get(name);const source=fs.readFileSync(name==='$lib/constants/Error'?path.join(root,'frontend/src/lib/constants/Error.ts'):path.join(directory,name+'.ts'),'utf8'),module=new vm.SourceTextModule(stripTypeScriptTypes(source,{mode:'transform'}),{context});cache.set(name,module);await module.link(specifier=>load(specifier.startsWith('.')?path.posix.normalize(path.posix.join(path.posix.dirname(name),specifier)):specifier));return module;}
for(const name of ['validation','query','client','normalize','catalogue-action-target','site-client','native-account-scope','session-follow-client']){const module=await load(name);await module.evaluate();}
export const current=Object.assign({},...['catalogue-action-target','site-client','client','session-follow-client'].map(name=>cache.get(name).namespace));
export const resource=(kind,id=ids[kind])=>({id,type:kind==='manga'?'manga':'scanlation_group',attributes:kind==='manga'?{title:{en:'Synthetic title'},originalLanguage:'en',status:'ongoing',contentRating:'safe',state:'published',version:1}:{name:'Synthetic group',version:1},relationships:[]});
export function fixture(kind,options={}){const calls=[];const transport=async request=>{calls.push(request);if(options.hold)await options.hold(request);const pathname=new URL(request.path,'https://catalog.example').pathname;let response;
 if(pathname==='/legacy/resolve'){const asked=request.body.resources[0];response={status:200,body:{result:'ok',data:options.unresolved?[]:[{kind:asked.kind,legacy_id:asked.id,id:ids[kind]}],unresolved:options.unresolved?[asked]:[]}};}
 else if(pathname==='/nostr/native-forum/contexts')response={status:200,body:{result:'ok',data:request.body.resources.map(row=>({...row,thread:null})),unavailable:[]}};
 else if(pathname.endsWith('/feed'))response={status:200,body:{result:'ok',response:'collection',data:[],limit:1,offset:0,total:0}};
 else if(pathname===`/${kind}/${ids[kind]}`||pathname===`/${kind}/${ids.alias}`)response={status:200,body:{result:'ok',response:'entity',data:resource(kind)}};
 else throw Error('Unexpected fixture route '+request.path);
 return options.transform?.(response,request)??response;};return{calls,client:new current.RetainedCatalogueClient(new current.PublicApiClient(transport),async()=>ids.namespace)};}
