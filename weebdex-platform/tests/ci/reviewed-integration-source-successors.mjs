import {updatesIntegrationPaths,restoreBundleSource} from './reviewed-bundle-source-successors.mjs';
import {nativeProfileActionsPaths,restoreNativeProfileActionsSource} from "./native-profile-actions-source-association.mjs";
// Test comparison only. Original metadata and physical current composition stay unchanged.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/integrated-schedule-catalogue-membership-source-association.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const integrationAssociationSha256='4f2aa3ce1919c48263336780c0c0fc352ef1332200d9ab348ada6a2835f02c8d';
export function readIntegrationAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||hash(bytes)!==integrationAssociationSha256)throw Error('Unknown integrated source association metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export const integrationSuccessorPaths=Object.freeze(readIntegrationAssociation().rows.map(row=>row.path));
export function restoreIntegratedSource(file,source,bytes=metadata){
 const row=readIntegrationAssociation(bytes).rows.find(row=>row.path===file);
 if(!row)throw Error('Unknown integrated source path');
 if(typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown current source bytes: '+file);
 let input=source;
 if(updatesIntegrationPaths.includes(file))input=restoreBundleSource('updates-integration',file,input);
 if(nativeProfileActionsPaths.includes(file)){try{input=restoreNativeProfileActionsSource(file,input);}catch(error){throw Error('Unknown current source bytes: '+file,{cause:error});}}
 const current=Buffer.from(input);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current source bytes: '+file);
 let restored=current.toString('utf8');
 for(const edit of row.literal_reversals){
  if(!edit.current||restored.split(edit.current).length!==2)throw Error('Unknown integrated source literal: '+file);
  restored=restored.replace(edit.current,()=>edit.previous);
 }
 const previous=Buffer.from(restored);
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown previous source bytes: '+file);
 return typeof source==='string'?restored:previous;
}
