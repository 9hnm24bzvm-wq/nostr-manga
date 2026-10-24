import { retainedTitleHistorySource } from './retained_title_history_source.mjs';
// Focused semantic TypeScript check of the actual Search function against the
// retained follow/feed declarations. Not a substitute for composed Svelte strict.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import vm from 'node:vm';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),dependencies=process.env.WEEBDEX_FRONTEND_ROOT;
assert(dependencies,'Use existing exact-lock dependencies');
const require=createRequire(path.join(dependencies,'package.json')),ts=require('typescript');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
function declaration(source,name){const file=ts.createSourceFile(name+'.ts',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);let found;function visit(node){if(node.name?.text===name)found=node.getText(file);else ts.forEachChild(node,visit);}visit(file);assert(found,`Missing actual declaration ${name}`);return found;}
const actualResponse=read('frontend/src/@types/Response.d.ts');
const contract=`
interface Manga { id:string; title:string; }
namespace Constants { export namespace Manga { ${declaration(read('frontend/src/lib/constants/Manga.ts'),'ReadingStatus')} } }
${declaration(read('frontend/src/@types/MangaFollow.d.ts'),'MangaFollow')}
${declaration(actualResponse,'FeedResponse')}
${declaration(actualResponse,'MangaFollowFeedResponse')}
declare function assertCataloguePreviewLibraryReady():void;
declare function openLibrary():Promise<void>;
declare function params(input:URLSearchParams):URLSearchParams;
declare function captureLibraryContext():{mode:'account'|'device'};
declare function captureLibraryView():string;
declare function assertLibraryView(view:string):void;
declare function apiRead<T>(path:string,query:URLSearchParams,account:boolean):Promise<T>;
declare class HTTPError extends Error {constructor(status:number,message:string,title?:string);}
declare function openDeviceTitleFeed(query:URLSearchParams):Promise<{load(page:number):Promise<{ready:boolean;total:number|null;page:number;limit:number;data:Manga[]}>;dispose():Promise<void>}>;
`;
function diagnostics(source){const file=path.join(root,'__focused_title_response_contract__.ts'),text=contract+'\n'+declaration(source,'Search')+'\nconst result:Promise<{status:number;body:MangaFollowFeedResponse}>=Search(new URLSearchParams());',options={strict:true,noEmit:true,skipLibCheck:true,target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,types:[]},host=ts.createCompilerHost(options),original=host.getSourceFile;host.getSourceFile=(name,...args)=>name===file?ts.createSourceFile(file,text,ts.ScriptTarget.Latest,true):original(name,...args);return ts.getPreEmitDiagnostics(ts.createProgram([file],options,host)).map(row=>({code:row.code,message:ts.flattenDiagnosticMessageText(row.messageText,'\n')}));}
const product='platform-integrations/local-library/MangaFollow.ts';
test('actual Search has the retained MangaFollow feed contract after real required-field narrowing',()=>{assert.deepEqual(diagnostics(read(product)),[]);});
test('executed bf36 source fails the same semantic assignment because Manga rows erase required follow fields',()=>{const old=retainedTitleHistorySource('bf36e22-MangaFollow'),errors=diagnostics(old);assert.equal(errors.length,1);assert.equal(errors[0].code,2322);assert.match(errors[0].message,/reading_status, followed_at/);});

test('actual Search rejects absent or mistyped required follow fields and always awaits disposal',async()=>{
 for(const row of [{id:'one',title:'One'},{id:'one',title:'One',reading_status:'reading'},{id:'one',title:'One',reading_status:'invalid',followed_at:'date'},{id:'one',title:'One',reading_status:1,followed_at:'date'},{id:'one',title:'One',reading_status:'reading',followed_at:1}]){
  let disposed=false;const context=vm.createContext({URLSearchParams,assertCataloguePreviewLibraryReady(){},openLibrary:async()=>{},params:value=>new URLSearchParams(value),captureLibraryContext:()=>({mode:'device'}),captureLibraryView:()=>'',assertLibraryView(){},HTTPError:class extends Error{constructor(status,message){super(message);this.status=status;}},openDeviceTitleFeed:async()=>({load:async()=>({ready:true,total:1,limit:20,page:1,data:[row]}),dispose:async()=>{await Promise.resolve();disposed=true;}})});
  const module=new vm.SourceTextModule(ts.transpileModule(declaration(read(product),'Search'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText,{context});await module.link(()=>{throw Error('Unexpected Search import');});await module.evaluate();await assert.rejects(module.namespace.Search(new URLSearchParams()),error=>error.status===502&&/metadata/.test(error.message));assert.equal(disposed,true);
 }
});
