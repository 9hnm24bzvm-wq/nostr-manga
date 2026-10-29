// Exact presentation-only source successor. Historical associations/helpers stay immutable.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256,restoreReaderUxFixpassPredecessor as restoreFrozenFixpass,restoreReaderUxPredecessor as restoreFrozenReader} from './reviewed-reader-ux-fixpass-source-successor.mjs';
export {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256};
const metadata=fs.readFileSync(new URL('../../docs/fixtures/title-spinner-source-association-20261028.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const titleSpinnerSourceAssociationSha256='db72f81d232b9ab4da1a7f9b2f314bda81ac1ff95342e0d9bd36448ed8704979';
export function readTitleSpinnerSourceAssociation(bytes=metadata){if(!Buffer.isBuffer(bytes)||hash(bytes)!==titleSpinnerSourceAssociationSha256)throw Error('Unknown title-spinner source association');return JSON.parse(bytes.toString('utf8'));}
export function restoreTitleSpinnerSourcePredecessor(file,source,bytes=metadata){
 const rows=readTitleSpinnerSourceAssociation(bytes).rows.filter(row=>row.path===file);
 if(rows.length!==1||typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown title-spinner source');
 const row=rows[0],current=Buffer.from(source);if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current title-spinner source bytes: '+file);
 if(row.added)return null;let previous=current.toString('utf8');
 for(const [index,edit] of row.literal_reversals.entries()){
  if(edit.ordinal!==index+1||!edit.current||previous.split(edit.current).length!==2||hash(edit.current)!==edit.current_sha256||hash(edit.previous)!==edit.previous_sha256)throw Error('Unknown title-spinner literal/order: '+file);
  previous=previous.replace(edit.current,()=>edit.previous);
 }
 if(Buffer.byteLength(previous)!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown title-spinner predecessor bytes: '+file);
 return typeof source==='string'?previous:Buffer.from(previous);
}
function frozen(file,source){return readTitleSpinnerSourceAssociation().rows.some(row=>row.path===file)?restoreTitleSpinnerSourcePredecessor(file,source):source;}
export function restoreReaderUxFixpassPredecessor(file,source,bytes){return restoreFrozenFixpass(file,frozen(file,source),bytes);}
export function restoreReaderUxPredecessor(file,source){return restoreFrozenReader(file,frozen(file,source));}
