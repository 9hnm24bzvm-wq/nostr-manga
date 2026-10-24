// Isolated in-memory source-comparison model, never a current build/runtime import.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {restoreBundleSource} from './reviewed-bundle-source-successors.mjs';
const oldPreparationHashes=Object.freeze([
 '38011a1d691495466ac05ba441cf9cdc0bbe5d4b743569bf72539d6687344aba',
 '56d8e9005ab1ea6af8e299adc25ac210da7d58f794b00d45d6b7996f4b4d7812',
]);
export async function preUpdatesOverlayComparison(preparation,context){
 if(typeof preparation!=='string'||!oldPreparationHashes.includes(createHash('sha256').update(preparation).digest('hex')))throw Error('Unknown predecessor preparation comparison bytes');
 const file=new URL('../../platform-integrations/local-library/overlays.mjs',import.meta.url);
 const source=restoreBundleSource('updates-integration','weebdex-platform/platform-integrations/local-library/overlays.mjs',fs.readFileSync(file,'utf8'));
 const module=new vm.SourceTextModule(source,{context,identifier:fileURLToPath(file)});
 const builtins={'node:crypto':{createHash},'node:fs':{readFileSync:fs.readFileSync},'node:path':{resolve:path.resolve}};
 await module.link(specifier=>{
  const values=builtins[specifier];
  if(!values)throw Error('Unknown predecessor overlay comparison dependency');
  return new vm.SyntheticModule(Object.keys(values),function(){for(const[key,value]of Object.entries(values))this.setExport(key,value);},{context});
 });
 return module;
}
