// Finite VM/observer seam. Import is the actual collector callback; the reader
// and signature verifier are explicit stand-ins. No browser or network starts.
import vm from 'node:vm';
import { pathToFileURL } from 'node:url';
import { captureUploadRequestRefusals } from '../../scripts/public-upload-transport-refusals.mjs';
import { harness } from './public-upload-observer-query-fixture.mjs';
export const previewPath = '/api/v1/nostr/uploads/11111111-1111-8111-8111-111111111111/files/22222222-2222-8222-8222-222222222222/preview';
export const snapshot = (records = [], extra = {}) => ({ schema:1, enabled:true, record_valid:true, truncated:false, records, ...extra });
export function pageWithReader(read) {
  let imports=0;
  return {get imports(){return imports;}, async evaluate(callback,args) {
    const context=vm.createContext({setTimeout,Promise,Object,Array});
    const reader=new vm.SyntheticModule(['readUploadRequestRefusals'],function(){this.setExport('readUploadRequestRefusals',read);},{context});
    await reader.link(()=>{throw Error('No reader dependencies');}); await reader.evaluate();
    const module=new vm.SourceTextModule('export default ('+callback.toString()+');',{context,importModuleDynamically:async name=>{
      if(name!=='/_app/immutable/qa/public-upload-refusals.js')throw Error('Unexpected reader path');imports++;return reader;
    }});
    await module.link(()=>{throw Error('No callback dependencies');});await module.evaluate();
    return module.namespace.default(args);
  }};
}
export function observedPreview({status=200,failed=true}={}) {
  const h=harness();
  try {
    const request=h.emit({pathname:previewPath});
    h.page.emit('response',{request:()=>request,status:()=>status});
    h.page.emit(failed?'requestfailed':'requestfinished',request);
    h.observer.close();return h.observer.snapshot();
  } finally {h.observer.close();}
}
export async function correlatedFixture() {
  const browser=observedPreview();
  const id=browser.rows[0].event_id,record={event_id:id,stage:'body-read',transport_status:200};
  const page=pageWithReader(()=>snapshot([record]));
  const captured=await captureUploadRequestRefusals(page,[id]);
  return {browser,captures:[captured],expected_stage:'body-read'};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  if(process.argv.slice(2).join(',')!=='--json')throw Error('Use --json');
  process.stdout.write(JSON.stringify(await correlatedFixture())+'\n');
}

