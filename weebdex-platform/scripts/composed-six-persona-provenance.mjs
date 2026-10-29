// Preserve all historical upload recipe pins through the single reviewed final stage.
import {createHash} from 'node:crypto';
import {associateCurrentUploadRecipe as baselineAssociation} from './composed-native-upload-provenance.mjs';
import {sixPersonaOverlayManifest,restoreSixPersonaOutput} from '../platform-integrations/six-persona/overlays.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');
const plugin='six-persona-reviewed-final-stage';
export function associateCurrentUploadRecipe(options){
 const {entry,readSnapshot}=options;
 if(!entry)return baselineAssociation(options);
 const matches=entry.stages.flatMap((stage,index)=>stage.plugin===plugin?[index]:[]);
 if(!matches.length)return baselineAssociation(options);
 if(matches.length!==1||matches[0]!==entry.stages.length-1)throw Error('Six-persona source stage is ambiguous or nonfinal');
 const index=matches[0],current=entry.stages[index],previous=entry.stages[index-1],row=sixPersonaOverlayManifest.find(row=>row.path===entry.canonical);
 if(!row||!previous||previous.sha256!==row.input_sha256||current.sha256!==row.output_sha256||entry.production_composed_sha256!==current.sha256)throw Error('Six-persona source stage metadata changed');
 if(current.snapshot!==`production-stages/${plugin}/${entry.canonical}`||previous.snapshot!==`production-stages/${previous.plugin}/${entry.canonical}`)throw Error('Six-persona snapshot escaped exact stage');
 const output=readSnapshot(current.snapshot),input=readSnapshot(previous.snapshot);
 if(typeof output!=='string'||typeof input!=='string'||hash(output)!==current.sha256||hash(input)!==previous.sha256||restoreSixPersonaOutput(output,entry.canonical)!==input)throw Error('Six-persona source restoration changed');
 const ancestor=baselineAssociation({...options,entry:{...entry,stages:entry.stages.slice(0,index),production_composed_sha256:previous.sha256}});
 if(!ancestor)return null;
 return{...ancestor,qualification:'Exact historical recipe ancestry through separately pinned native-account and six-persona stages. These are actual production-stage bytes, not runtime acceptance evidence.',six_persona_successor:{plugin,snapshot:current.snapshot,input_sha256:previous.sha256,output_sha256:current.sha256,exact_reverse_to_qualified_snapshot:true}};
}
