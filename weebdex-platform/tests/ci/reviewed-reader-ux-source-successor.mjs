// Exact source-only association for the reviewed reader UX changes.
// This helper is never imported by product/build code. It does not grant the
// current source any historical browser, HTTP, or release qualification.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reader-ux-source-association-20261028.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const readerUxAssociationSha256='fe33ccbb30ff05efa1cba65a79a7ce9f8b561a9ba2447dde078c4642f94548fb';
export function readReaderUxAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||hash(bytes)!==readerUxAssociationSha256)throw Error('Unknown reader UX association metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export function restoreReaderUxPredecessor(file,source,bytes=metadata){
 const rows=readReaderUxAssociation(bytes).rows.filter(row=>row.path===file);
 if(rows.length!==1||typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown reader UX source');
 const row=rows[0],current=Buffer.from(source);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current reader UX bytes: '+file);
 if(row.added)return null;
 let restored=current.toString('utf8');
 for(const [index,edit] of row.literal_reversals.entries()){
  if(edit.ordinal!==index+1||!edit.current||restored.split(edit.current).length!==2||hash(edit.current)!==edit.current_sha256||hash(edit.previous)!==edit.previous_sha256)throw Error('Unknown reader UX literal/order: '+file);
  restored=restored.replace(edit.current,()=>edit.previous);
 }
 const previous=Buffer.from(restored);
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown reader UX predecessor bytes: '+file);
 return typeof source==='string'?restored:previous;
}
