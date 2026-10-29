// One exact reviewed successor for the entire six-persona fixpass.
// Historical stages and qualification pins are immutable predecessors.
import fs from 'node:fs';import path from 'node:path';import {createHash} from 'node:crypto';
import {readPublicApiGate} from '../public-api/overlays.mjs';
import {uploadCompletionPatches} from '../six-persona-upload-overlays.mjs';
import {contributorRoutePatches} from '../public-api/contributor-route-patches.mjs';
import {staffWorkspacePatches} from '../staff-workspace/patches.mjs';
import {integrationPatches} from './integration-patches.mjs';
export {sixPersonaRouteAdditions,sixPersonaComponents} from './integration-patches.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');
const REVIEWED_MANIFEST_SHA='1a7c5bdef584e45db14862ccf92a20843bfb468c5195935b4de80c8ad742604c';
const manifestBytes=fs.readFileSync(new URL('./manifest.json',import.meta.url));
export const sixPersonaManifest=JSON.parse(manifestBytes);
export const sixPersonaPatches=[...uploadCompletionPatches,...contributorRoutePatches,...staffWorkspacePatches,...integrationPatches];
export const sixPersonaOverlayManifest=sixPersonaManifest.rows;
export function assertSixPersonaScope(){
 if(hash(manifestBytes)!==REVIEWED_MANIFEST_SHA||sixPersonaPatches.length!==sixPersonaManifest.patch_count||hash(JSON.stringify(sixPersonaPatches))!==sixPersonaManifest.patch_registry_sha256)throw Error('Unreviewed six-persona fixpass source scope');
 if(new Set(sixPersonaOverlayManifest.map(x=>x.path)).size!==sixPersonaOverlayManifest.length)throw Error('Duplicate six-persona target');
 for(const p of sixPersonaPatches)if(!p.before||!p.after||!Number.isInteger(p.count)||p.count<1||!sixPersonaOverlayManifest.some(x=>x.path===p.path))throw Error('Invalid exact six-persona patch');
}
export function restoreSixPersonaOutput(output,relative){assertSixPersonaScope();const row=sixPersonaOverlayManifest.find(x=>x.path===relative);if(!row)return output;if(hash(output)!==row.output_sha256)throw Error('Six-persona output changed: '+relative);let previous=output;for(const p of sixPersonaPatches.filter(x=>x.path===relative).reverse()){if(previous.split(p.after).length-1!==p.count)throw Error('Six-persona reverse anchor changed: '+p.label);previous=previous.split(p.after).join(p.before);}if(hash(previous)!==row.input_sha256)throw Error('Six-persona predecessor changed: '+relative);return previous;}
export function applySixPersonaOutput(input,relative){assertSixPersonaScope();const row=sixPersonaOverlayManifest.find(x=>x.path===relative);if(!row)return input;if(hash(input)!==row.input_sha256)throw Error('Six-persona predecessor changed: '+relative);let output=input;for(const p of sixPersonaPatches.filter(x=>x.path===relative)){if(output.split(p.before).length-1!==p.count)throw Error('Six-persona anchor changed: '+p.label);output=output.split(p.before).join(p.after);}if(hash(output)!==row.output_sha256||restoreSixPersonaOutput(output,relative)!==input)throw Error('Six-persona output changed: '+relative);return output;}
export function createSixPersonaOverlays(frontend,{enabled=readPublicApiGate()}={}){assertSixPersonaScope();const root=path.resolve(frontend);return{name:'six-persona-reviewed-final-stage',enforce:'pre',transform:{order:'pre',handler(input,id){if(!enabled||id.includes('?'))return null;const relative=path.relative(root,id).split(path.sep).join('/'),row=sixPersonaOverlayManifest.find(x=>x.path===relative);if(!row)return null;if(hash(fs.readFileSync(path.join(root,relative)))!==row.original_sha256)throw Error('Six-persona canonical source changed: '+relative);return{code:applySixPersonaOutput(input,relative),map:null};}}};}
