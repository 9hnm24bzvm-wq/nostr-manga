// Exact bounded-reader predecessor comparisons only. Never normalize physical runtime inputs.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/bounded-reader-source-associations.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const boundedReaderAssociationSha256='1a245ce7237911e3bc68532637c60861714dec7ec6b42289c496710afde8f716';
export function readBoundedReaderAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||bytes.length!==42359||hash(bytes)!==boundedReaderAssociationSha256)throw Error('Unknown bounded reader comparison metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export const boundedReaderSuccessorPaths=Object.freeze(readBoundedReaderAssociation().rows.map(row=>row.path));
export function restoreBoundedReaderSource(stage,file,source,bytes=metadata){
 const rows=readBoundedReaderAssociation(bytes).rows.filter(row=>row.stage===stage&&row.path===file);
 if(typeof stage!=='string'||typeof file!=='string'||rows.length!==1)throw Error('Unknown bounded reader comparison stage or path');
 const row=rows[0];
 if(typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown current source bytes: '+file);
 const current=Buffer.from(source);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current source bytes: '+file);
 let restored=current.toString('utf8'),cursor=0;
 for(const [index,edit] of row.literal_reversals.entries()){
  const position=restored.indexOf(edit.current);
  if(edit.ordinal!==index+1||!edit.current||restored.split(edit.current).length!==2||position<cursor||
   Buffer.byteLength(edit.current)!==edit.current_bytes||hash(edit.current)!==edit.current_literal_sha256||
   Buffer.byteLength(edit.previous)!==edit.previous_bytes||hash(edit.previous)!==edit.previous_literal_sha256)
   throw Error('Unknown bounded reader comparison literal or order: '+file);
  restored=restored.slice(0,position)+edit.previous+restored.slice(position+edit.current.length);
  cursor=position+edit.previous.length;
 }
 const previous=Buffer.from(restored);
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown previous source bytes: '+file);
 return typeof source==='string'?restored:previous;
}
