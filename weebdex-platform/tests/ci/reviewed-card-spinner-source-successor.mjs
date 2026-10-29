// Exact last card presentation successor. Earlier qualified source layers stay immutable.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256,readTitleSpinnerSourceAssociation,titleSpinnerSourceAssociationSha256,readReaderFollowupSourceAssociation,readerFollowupSourceAssociationSha256,restoreReaderFollowupSourcePredecessor as restoreFrozenFollowup,restoreTitleSpinnerSourcePredecessor as restoreFrozenSpinner,restoreReaderUxFixpassPredecessor as restoreFrozenFixpass,restoreReaderUxPredecessor as restoreFrozenReader} from './reviewed-reader-followup-source-successor.mjs';
export {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256,readTitleSpinnerSourceAssociation,titleSpinnerSourceAssociationSha256,readReaderFollowupSourceAssociation,readerFollowupSourceAssociationSha256};
const metadata=fs.readFileSync(new URL('../../docs/fixtures/card-spinner-source-association-20261028.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const cardSpinnerSourceAssociationSha256='d3bb271283000e44223eb2213ad8c8a8309875ed93dcb80749db54e997fdcd6e';
export function readCardSpinnerSourceAssociation(bytes=metadata){if(!Buffer.isBuffer(bytes)||hash(bytes)!==cardSpinnerSourceAssociationSha256)throw Error('Unknown card-spinner source association');return JSON.parse(bytes.toString('utf8'));}
export function restoreCardSpinnerSourcePredecessor(file,source,bytes=metadata){
 const rows=readCardSpinnerSourceAssociation(bytes).rows.filter(row=>row.path===file);
 if(rows.length!==1||typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown card-spinner source');
 const row=rows[0],current=Buffer.from(source);if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current card-spinner source bytes: '+file);
 if(row.added)return null;let previous=current.toString('utf8');
 for(const [index,edit] of row.literal_reversals.entries()){
  if(edit.ordinal!==index+1||!edit.current||previous.split(edit.current).length!==2||hash(edit.current)!==edit.current_sha256||hash(edit.previous)!==edit.previous_sha256)throw Error('Unknown card-spinner literal/order: '+file);
  previous=previous.replace(edit.current,()=>edit.previous);
 }
 if(Buffer.byteLength(previous)!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown card-spinner predecessor bytes: '+file);
 return typeof source==='string'?previous:Buffer.from(previous);
}
function frozen(file,source){return readCardSpinnerSourceAssociation().rows.some(row=>row.path===file)?restoreCardSpinnerSourcePredecessor(file,source):source;}
export function restoreReaderFollowupSourcePredecessor(file,source,bytes){return restoreFrozenFollowup(file,frozen(file,source),bytes);}
export function restoreTitleSpinnerSourcePredecessor(file,source,bytes){return restoreFrozenSpinner(file,frozen(file,source),bytes);}
export function restoreReaderUxFixpassPredecessor(file,source,bytes){return restoreFrozenFixpass(file,frozen(file,source),bytes);}
export function restoreReaderUxPredecessor(file,source){return restoreFrozenReader(file,frozen(file,source));}
