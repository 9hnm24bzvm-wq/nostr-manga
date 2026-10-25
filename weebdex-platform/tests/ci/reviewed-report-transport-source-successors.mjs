// Test comparisons only; never import into a transport, runtime or build.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/native-report-transport-source-association.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
export const reportTransportAssociationSha256='b97dc0ca3aea0330e59c0c88b3281122f426f666773b163353d83b55fc24b1b7';
export function readReportTransportAssociation(bytes=metadata){
 if(!Buffer.isBuffer(bytes)||hash(bytes)!==reportTransportAssociationSha256)throw Error('Unknown report transport association metadata');
 return JSON.parse(bytes.toString('utf8'));
}
export function restoreReportTransportSource(file,source,mode,bytes=metadata){
 const data=readReportTransportAssociation(bytes);
 if(file!==data.path||!['raw','qa_stripped'].includes(mode))throw Error('Unknown report transport path or mode');
 const row=data[mode];
 if(typeof source!=='string'&&!Buffer.isBuffer(source))throw Error('Unknown current report transport bytes');
 const current=Buffer.from(source);
 if(current.length!==row.current_bytes||hash(current)!==row.current_sha256)throw Error('Unknown current report transport bytes');
 let restored=current.toString('utf8');
 for(const edit of row.literal_reversals){
  if(!edit.current||restored.split(edit.current).length!==2)throw Error('Unknown report transport literal');
  restored=restored.replace(edit.current,()=>edit.previous);
 }
 const previous=Buffer.from(restored);
 if(previous.length!==row.previous_bytes||hash(previous)!==row.previous_sha256)throw Error('Unknown previous report transport bytes');
 return typeof source==='string'?restored:previous;
}
