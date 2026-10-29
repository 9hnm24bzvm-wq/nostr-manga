// Current upload recipe ancestry through two real, separately recorded stages.
// Historical signed snapshots retain their original hashes; current native
// snapshots are never relabelled as historical execution evidence.
import {createHash} from 'node:crypto';
import {associateSupersededUploadRecipe} from './composed-upload-provenance.mjs';
import {nativeUploadOverlayManifest,restoreNativeUploadOutput} from '../platform-integrations/public-api/upload-overlays.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');
const nativeName='native-browser-account-upload-successor',historicalName='recovered-public-signed-upload';
export function associateCurrentUploadRecipe(options){
 const {entry,readSnapshot}=options;
 if(!entry)return associateSupersededUploadRecipe(options);
 const successors=entry.stages.flatMap((stage,index)=>stage.plugin===nativeName?[index]:[]);
 if(!successors.length)return associateSupersededUploadRecipe(options);
 if(successors.length!==1||successors[0]!==entry.stages.length-1)throw Error('Native upload source stage is ambiguous or nonfinal');
 const index=successors[0],native=entry.stages[index],previous=entry.stages[index-1],spec=nativeUploadOverlayManifest.find(row=>row.path===entry.canonical);
 if(!spec||previous?.plugin!==historicalName||previous.sha256!==spec.predecessor_sha256||native.sha256!==spec.output_sha256||entry.production_composed_sha256!==native.sha256)throw Error('Native upload source stage metadata changed');
 if(native.snapshot!==`production-stages/${nativeName}/${entry.canonical}`||previous.snapshot!==`production-stages/${historicalName}/${entry.canonical}`)throw Error('Native upload source snapshot escaped exact stage');
 const current=readSnapshot(native.snapshot),historical=readSnapshot(previous.snapshot);
 if(typeof current!=='string'||typeof historical!=='string'||hash(current)!==native.sha256||hash(historical)!==previous.sha256||restoreNativeUploadOutput(current,entry.canonical)!==historical)throw Error('Native upload source restoration changed');
 const ancestor=associateSupersededUploadRecipe({...options,entry:{...entry,stages:entry.stages.slice(0,index),production_composed_sha256:previous.sha256}});
 if(!ancestor)return null;
 return{...ancestor,qualification:'Exact historical recipe ancestry followed by a separately verified native-account successor. Both snapshots are real production-stage bytes; source association does not qualify browser or HTTP behavior.',native_successor:{plugin:nativeName,snapshot:native.snapshot,input_sha256:previous.sha256,output_sha256:native.sha256,exact_reverse_to_historical_snapshot:true}};
}
