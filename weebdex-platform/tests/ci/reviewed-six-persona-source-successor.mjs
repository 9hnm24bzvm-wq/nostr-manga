// One exact six-persona successor. Historical controls stay historical; no pass is inherited.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import * as baseline from './reviewed-card-spinner-source-successor.mjs';
export {readReaderUxAssociation,readReaderUxFixpassAssociation,readerUxFixpassAssociationSha256,readTitleSpinnerSourceAssociation,titleSpinnerSourceAssociationSha256,readReaderFollowupSourceAssociation,readerFollowupSourceAssociationSha256,readCardSpinnerSourceAssociation,cardSpinnerSourceAssociationSha256} from './reviewed-card-spinner-source-successor.mjs';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/six-persona-source-association-20261028.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const sixPersonaSourceAssociationSha256='4e30007ee41c0c1ad2392c1c1821324e85c72dd6cf31a2498375388b14e95444';
export function readSixPersonaSourceAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||hash(bytes)!==sixPersonaSourceAssociationSha256)throw Error('Unknown six-persona source association');
 const value=JSON.parse(bytes);if(value.baseline_tree!=='a035b00056557f83348db13c6288a680fd35aca8'||value.predecessor_association_sha256!==baseline.cardSpinnerSourceAssociationSha256)throw Error('Unknown six-persona predecessor');return value;
}
export function restoreSixPersonaSourcePredecessor(file,source,bytes=metadata){
 const rows=readSixPersonaSourceAssociation(bytes).rows.filter(row=>row.path===file);
 if(rows.length!==1||typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown six-persona source');
 const row=rows[0],current=Buffer.from(source);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current six-persona source bytes: '+file);
 if(row.added)return null;
 let previous=current,limit=current.length;
 for(const edit of [...row.byte_reversals].reverse()){
  if(!Number.isInteger(edit.start)||!Number.isInteger(edit.end)||edit.start<0||edit.end<edit.start||edit.end>limit)throw Error('Unknown six-persona reversal offsets');
  const old=Buffer.from(edit.previous_base64,'base64');
  if(old.toString('base64')!==edit.previous_base64||hash(old)!==edit.previous_sha256||hash(current.subarray(edit.start,edit.end))!==edit.current_sha256)throw Error('Unknown six-persona reversal bytes');
  previous=Buffer.concat([previous.subarray(0,edit.start),old,previous.subarray(edit.end)]);limit=edit.start;
 }
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown six-persona predecessor bytes: '+file);
 return typeof source==='string'?previous.toString('utf8'):previous;
}
function frozen(file,source){return readSixPersonaSourceAssociation().rows.some(row=>row.path===file)?restoreSixPersonaSourcePredecessor(file,source):source;}
export function restoreCardSpinnerSourcePredecessor(file,source,bytes){return baseline.restoreCardSpinnerSourcePredecessor(file,frozen(file,source),bytes);}
export function restoreReaderFollowupSourcePredecessor(file,source,bytes){return baseline.restoreReaderFollowupSourcePredecessor(file,frozen(file,source),bytes);}
export function restoreTitleSpinnerSourcePredecessor(file,source,bytes){return baseline.restoreTitleSpinnerSourcePredecessor(file,frozen(file,source),bytes);}
export function restoreReaderUxFixpassPredecessor(file,source,bytes){return baseline.restoreReaderUxFixpassPredecessor(file,frozen(file,source),bytes);}
export function restoreReaderUxPredecessor(file,source){return baseline.restoreReaderUxPredecessor(file,frozen(file,source));}
