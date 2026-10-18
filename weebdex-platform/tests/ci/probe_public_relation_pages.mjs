#!/usr/bin/env node
// Synchronous-fixture bridge: exact production adapter/client over loopback HTTP.
// No account, cookie, signing key, installation or external service is used.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const script=fileURLToPath(import.meta.url),directory=path.resolve(path.dirname(script),'../../platform-integrations/public-api');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const receipt={schemaVersion:1,kind:'actual-public-relation-adapter-loopback-probe',startedAt:new Date().toISOString(),status:'failed',scriptSha256:sha(fs.readFileSync(script)),sources:{},requests:[],budget:{deadlineMs:60_000,pageNormalizedJsonBytes:1048576,totalNormalizedJsonBytes:16777216},boundary:'Actual production adapter/client over fixture loopback HTTP; no native-browser/storage or production acceptance claim.'};
const started=performance.now();
try{
 const input=process.env.PUBLIC_API_ORIGIN??'',origin=new URL(input);
 if(origin.protocol!=='http:'||!['127.0.0.1','localhost','[::1]'].includes(origin.hostname)||origin.username||origin.password||origin.pathname!=='/'||origin.search||origin.hash)throw Error('PUBLIC_API_ORIGIN must be an HTTP loopback origin without credentials, path, query or fragment');
 const id=process.env.MANGA_ID??'',expected=Number(process.env.EXPECTED_RELATIONS??4096);
 if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id)||!Number.isSafeInteger(expected)||expected<0||expected>4096)throw Error('MANGA_ID must be a canonical UUID and EXPECTED_RELATIONS an integer from0 through4096');
 receipt.origin=origin.origin;receipt.requestedMangaId=id;receipt.expectedRelations=expected;
 const context=vm.createContext({URL,URLSearchParams,AbortController,TextEncoder,setTimeout,clearTimeout,console,fetch:async(relative,options)=>{
  if(typeof relative!=='string'||!relative.startsWith('/api/v1/')||relative.startsWith('//')||options.method!=='GET')throw Error('Probe transport escaped its read-only public API prefix');
  const url=new URL(relative,origin),row={pathname:url.pathname,limit:url.searchParams.get('limit'),offset:url.searchParams.get('offset'),ownerVersion:url.searchParams.get('ownerVersion'),snapshot:url.searchParams.get('snapshot'),idCount:url.searchParams.getAll('ids[]').length,includes:url.searchParams.getAll('includes[]'),requestSha256:sha(relative)},at=performance.now();receipt.requests.push(row);
  try{const response=await fetch(url,options),body=await response.clone().arrayBuffer();row.status=response.status;row.rawBytes=body.byteLength;row.bodySha256=sha(new Uint8Array(body));return response;}finally{row.elapsedMs=Math.round((performance.now()-at)*1000)/1000;}
 }}),modules=new Map();
 async function load(name){const file=path.resolve(directory,name+'.ts');if(!file.startsWith(directory+path.sep))throw Error('Unexpected production import outside the public API closure');if(!modules.has(file)){const bytes=fs.readFileSync(file);receipt.sources[path.relative(directory,file)]=sha(bytes);const module=new vm.SourceTextModule(stripTypeScriptTypes(bytes.toString('utf8'),{mode:'transform',sourceUrl:file}),{context,identifier:file});modules.set(file,module);await module.link((specifier,importer)=>load(path.relative(directory,path.resolve(path.dirname(importer.identifier),specifier)).replace(/\.ts$/,'')));}return modules.get(file);}
 async function namespace(name){const module=await load(name);if(module.status!=='evaluated')await module.evaluate();return module.namespace;}
 const {PublicApiClient,fetchTransport}=await namespace('client'),{catalogueEdges}=await namespace('catalogue-edges');
 const result=await catalogueEdges(new PublicApiClient(fetchTransport('/api/v1')),'relation',id);
 receipt.httpStatus=result.status;
 if(result.status!==200||!result.body||result.body.total!==expected||result.body.data.length!==expected)throw Error('The actual adapter did not return the complete expected relation collection');
 const targets=result.body.data.map(row=>row.id),edges=result.body.data.map(row=>row.public_api.edge.resource.id);
 if(new Set(targets).size!==expected||new Set(edges).size!==expected)throw Error('The actual adapter returned a duplicate relation edge or target');
 receipt.status='passed';receipt.actualRelations=result.body.data.length;receipt.targetOrderSha256=sha(JSON.stringify(targets));receipt.edgeOrderSha256=sha(JSON.stringify(edges));receipt.canonicalOwner=result.body.data[0]?.public_api.edge.owner??null;
}catch(error){receipt.error={name:error?.name??'Error',message:String(error?.message??error),...(typeof error?.status==='number'?{status:error.status}:{})};process.exitCode=1;}
receipt.elapsedMs=Math.round((performance.now()-started)*1000)/1000;
receipt.relationPageRequests=receipt.requests.filter(row=>row.pathname.endsWith('/relation')).length;
receipt.hydrationRequests=receipt.requests.filter(row=>row.pathname==='/api/v1/manga').length;
receipt.rawResponseBytes=receipt.requests.reduce((sum,row)=>sum+(row.rawBytes??0),0);
const json=JSON.stringify(receipt,null,2)+'\n';if(process.env.RELATION_PROBE_RECEIPT)fs.writeFileSync(process.env.RELATION_PROBE_RECEIPT,json);process.stdout.write(json);
