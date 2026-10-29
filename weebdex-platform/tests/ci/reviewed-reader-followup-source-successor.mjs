// Exact browser-found reader/account followup. Every earlier association stays immutable.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256,readTitleSpinnerSourceAssociation,titleSpinnerSourceAssociationSha256,restoreTitleSpinnerSourcePredecessor as restoreFrozenSpinner,restoreReaderUxFixpassPredecessor as restoreFrozenFixpass,restoreReaderUxPredecessor as restoreFrozenReader} from './reviewed-title-spinner-source-successor.mjs';
export {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256,readTitleSpinnerSourceAssociation,titleSpinnerSourceAssociationSha256};
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reader-followup-source-association-20261028.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const readerFollowupSourceAssociationSha256='a9b5812a039a91f31ad414be5f5e86ed6637c044eea6450589cd854ea9bcb6d1';
export function readReaderFollowupSourceAssociation(bytes=metadata){if(!Buffer.isBuffer(bytes)||hash(bytes)!==readerFollowupSourceAssociationSha256)throw Error('Unknown reader-followup source association');return JSON.parse(bytes.toString('utf8'));}
export function restoreReaderFollowupSourcePredecessor(file,source,bytes=metadata){
 const rows=readReaderFollowupSourceAssociation(bytes).rows.filter(row=>row.path===file);
 if(rows.length!==1||typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown reader-followup source');
 const row=rows[0],current=Buffer.from(source);if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current reader-followup source bytes: '+file);
 if(row.added)return null;let previous=current.toString('utf8');
 for(const [index,edit] of row.literal_reversals.entries()){
  if(edit.ordinal!==index+1||!edit.current||previous.split(edit.current).length!==2||hash(edit.current)!==edit.current_sha256||hash(edit.previous)!==edit.previous_sha256)throw Error('Unknown reader-followup literal/order: '+file);
  previous=previous.replace(edit.current,()=>edit.previous);
 }
 if(Buffer.byteLength(previous)!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown reader-followup predecessor bytes: '+file);
 return typeof source==='string'?previous:Buffer.from(previous);
}
function frozen(file,source){return readReaderFollowupSourceAssociation().rows.some(row=>row.path===file)?restoreReaderFollowupSourcePredecessor(file,source):source;}
export function restoreTitleSpinnerSourcePredecessor(file,source,bytes){return restoreFrozenSpinner(file,frozen(file,source),bytes);}
export function restoreReaderUxFixpassPredecessor(file,source,bytes){return restoreFrozenFixpass(file,frozen(file,source),bytes);}
export function restoreReaderUxPredecessor(file,source){return restoreFrozenReader(file,frozen(file,source));}
