// Comparison-only reconstruction. Product execution always uses current bytes.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
export const historyAssociationSha256='c06a19d189112fb67edcd36e7bc766649b6084284e74a4b3711ea91b8547a33f';
const bytes=fs.readFileSync(new URL('../../docs/fixtures/device-history-source-association.json',import.meta.url));
const hash=x=>createHash('sha256').update(x).digest('hex');
function require(value){if(!value)throw Error('Unknown current source bytes: reviewed history association');}
export function readHistoryAssociation(input=bytes){require(hash(input)===historyAssociationSha256);return JSON.parse(input);}
export const historySuccessorPaths=Object.freeze(readHistoryAssociation().rows.map(row=>row.path));
export function restoreHistorySource(path,source,metadata=bytes){
 const row=readHistoryAssociation(metadata).rows.find(row=>row.path===path);require(row&& (typeof source==='string'||Buffer.isBuffer(source)));
 require(Buffer.byteLength(source)===row.current_bytes&&hash(source)===row.current_sha256);let output=source.toString();
 for(const edit of row.literal_reversals){require(edit.current&&output.split(edit.current).length===2);output=output.replace(edit.current,()=>edit.previous);}
 require(Buffer.byteLength(output)===row.previous_bytes&&hash(output)===row.previous_sha256);return output;
}
