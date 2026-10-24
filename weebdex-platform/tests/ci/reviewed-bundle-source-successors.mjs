// Exact named comparison stages only. Runtime and build inputs stay physical current bytes.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reviewed-bundle-source-successors.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const bundleAssociationSha256='aff713f0a29179ee8701b1fd1b63e4cc6756ee2ef9cc84e57a7510ec614738c8';
export function readBundleAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||hash(bytes)!==bundleAssociationSha256)throw Error('Unknown bundle comparison metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export const updatesIntegrationPaths=Object.freeze(readBundleAssociation().rows.filter(row=>row.stage==='updates-integration').map(row=>row.path));
export const updatesCompositionPath='weebdex-platform/scripts/prepare-frontend-integration.mjs';
export function restoreBundleSource(stage,file,source,bytes=metadata){
 const rows=readBundleAssociation(bytes).rows.filter(row=>row.stage===stage&&row.path===file);
 if(rows.length!==1)throw Error('Unknown bundle comparison stage or path');
 const row=rows[0];
 if(typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown current source bytes: '+file);
 const current=Buffer.from(source);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current source bytes: '+file);
 let restored=current.toString('utf8');
 for(const edit of row.literal_reversals){
  if(!edit.current||restored.split(edit.current).length!==2)throw Error('Unknown bundle comparison literal: '+file);
  restored=restored.replace(edit.current,()=>edit.previous);
 }
 const previous=Buffer.from(restored);
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown previous source bytes: '+file);
 return typeof source==='string'?restored:previous;
}
