// Source comparison only. Actual fixtures and runtime modules keep current bytes.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reviewed-scheduled-source-associations.json',import.meta.url));
const digest=value=>createHash('sha256').update(value).digest('hex');
export const scheduledAssociationSha256='7d9d4d6d822c595f6f591be10a9ec2580b480df0dd87615267e554c8b9b8da45';
export function readScheduledAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||digest(bytes)!==scheduledAssociationSha256)throw Error('Unknown reviewed scheduling metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export function restoreReviewedScheduledSource(source,kind,bytes=metadata){
 const row=readScheduledAssociation(bytes).rows.find(row=>row.kind===kind);
 if(!row||typeof source!=='string'||Buffer.byteLength(source)!==row.current_bytes||digest(source)!==row.current_sha256)throw Error('Unknown reviewed language successor: '+kind);
 let result=source;
 for(const {current,previous} of row.literal_reversals){
  if(!current||result.split(current).length!==2)throw Error('Unknown reviewed scheduling literal');
  result=result.replace(current,()=>previous);
 }
 if(Buffer.byteLength(result)!==row.previous_bytes||digest(result)!==row.previous_sha256)throw Error('Unknown reviewed scheduling predecessor');
 return result;
}
