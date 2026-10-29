// Historical signed contribution controls only. Exact e3a8ca8 predecessor source
// is restored through the reviewed additive association; this is NOT current
// native-account UI qualification. Counterfactual override files remain explicit
// caller-selected inputs. No product module imports this fixture.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {readReaderUxFixpassAssociation,restoreReaderUxFixpassPredecessor} from './reviewed-card-spinner-source-successor.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const associated=new Set(readReaderUxFixpassAssociation().rows.map(row=>row.path));
export function historicalContributionSource(file,encoding='utf8'){
 const location=path.resolve(file instanceof URL?fileURLToPath(file):file),relative=path.relative(root,location).split(path.sep).join('/');
 const source=fs.readFileSync(location),key='weebdex-platform/'+relative;
 const restored=associated.has(key)?restoreReaderUxFixpassPredecessor(key,source):source;
 if(restored===null)throw Error('No historical contribution predecessor for '+relative);
 return encoding===null?Buffer.from(restored):Buffer.from(restored).toString(encoding);
}
let factories;
export async function historicalContributionOverlayFactories(){
 if(factories)return factories;
 factories=(async()=>{
  const context=vm.createContext({URL,console,process}),modules=new Map();
  const historicalFs={...fs,readFileSync:(file,encoding)=>historicalContributionSource(file,typeof encoding==='string'?encoding:encoding?.encoding??null)};
  async function load(file){
   if(modules.has(file))return modules.get(file);
   const module=new vm.SourceTextModule(historicalContributionSource(file),{context,identifier:file,initializeImportMeta:meta=>meta.url=pathToFileURL(file).href});modules.set(file,module);
   await module.link(async(specifier,parent)=>{
    if(specifier.startsWith('.'))return load(path.resolve(path.dirname(parent.identifier),specifier));
    const actual=specifier==='node:fs'?{...historicalFs,default:historicalFs}:await import(specifier);
    return new vm.SyntheticModule(Object.keys(actual),function(){for(const[key,value]of Object.entries(actual))this.setExport(key,value);},{context,identifier:specifier});
   });return module;
  }
  const result={};
  for(const relative of ['frontend-overlays.mjs','frontend-reader-overlays.mjs','local-library/overlays.mjs','public-api/overlays.mjs','public-api/upload-overlays.mjs']){
   const module=await load(path.join(root,'platform-integrations',relative));await module.evaluate();Object.assign(result,module.namespace);
  }
  return result;
 })();return factories;
}
