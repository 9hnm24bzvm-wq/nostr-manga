// Exact source-only additive association, never imported by product/build code.
// Published first-pass metadata and qualification digests are not modified.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {readReaderUxAssociation,restoreReaderUxPredecessor as restorePublishedReaderUxPredecessor} from './reviewed-reader-ux-source-successor.mjs';
export {readReaderUxAssociation};
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reader-ux-fixpass-source-association-20261028.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const readerUxFixpassAssociationSha256='860ec694f57f712e1f3ce10753340ff03b3dd11c92cbf2dd6dc19fb85fae95e6';
export function readReaderUxFixpassAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||hash(bytes)!==readerUxFixpassAssociationSha256)throw Error('Unknown reader UX fixpass association metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export function restoreReaderUxFixpassPredecessor(file,source,bytes=metadata){
 const rows=readReaderUxFixpassAssociation(bytes).rows.filter(row=>row.path===file);
 if(rows.length!==1||typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown reader UX fixpass source');
 const row=rows[0],current=Buffer.from(source);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current reader UX fixpass bytes: '+file);
 if(row.added)return null;
 let restored=current.toString('utf8');
 for(const [index,edit] of row.literal_reversals.entries()){
  if(edit.ordinal!==index+1||!edit.current||restored.split(edit.current).length!==2||hash(edit.current)!==edit.current_sha256||hash(edit.previous)!==edit.previous_sha256)throw Error('Unknown reader UX fixpass literal/order: '+file);
  restored=restored.replace(edit.current,()=>edit.previous);
 }
 const previous=Buffer.from(restored);
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown reader UX fixpass predecessor bytes: '+file);
 return typeof source==='string'?restored:previous;
}
// Current comparison consumers compose both exact source-only layers. Unknown
// or edited second-pass bytes are refused before the unchanged published layer.
export function restoreReaderUxPredecessor(file,source){
 const rows=readReaderUxFixpassAssociation().rows.filter(row=>row.path===file);
 const published=rows.length?restoreReaderUxFixpassPredecessor(file,source):source;
 return restorePublishedReaderUxPredecessor(file,published);
}
