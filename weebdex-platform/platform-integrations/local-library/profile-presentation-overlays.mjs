// Presentation-only exact successor. IDs, routes and action authority stay unchanged.
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const profilePresentationPatches=[
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "profile displays a bounded human username instead of its canonical resource UUID",
    "count": 1,
    "before": "<strong class=\"text-muted-foreground truncate font-medium\">@{$user.id}</strong>",
    "after": "{#if typeof $user.name === \"string\" && /^[\\p{L}\\p{N}_-]{3,32}(?![\\s\\S])/u.test($user.name)}<strong class=\"text-muted-foreground truncate font-medium\">@{$user.name}</strong>{/if}"
  }
];
export const profilePresentationManifest=[
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "original_sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "predecessor_sha256": "5a4ffab43336d0245e512a3baabed38aaf0d831ab63c580a3b9b9f739837e9af",
    "output_sha256": "3e5fbb34960e51a7c13cefd13cee99221747b0040cf742dc39b15eee3f130119"
  }
];
export function assertProfilePresentationScope(){
 if(hash(JSON.stringify(profilePresentationPatches))!=="15951dab8356d36cc9c56544ee6b15ceacb9796ae5eaa0d8a1ee56cbfd9f83d6"||hash(JSON.stringify(profilePresentationManifest))!=="e2146720751bcbec815399e4c67e267be3ef02dc69476df5c56d3a36c1703ae9")throw Error('Unreviewed profile presentation scope');
}
export function applyProfilePresentationOutput(input,relative){
 assertProfilePresentationScope();const row=profilePresentationManifest.find(row=>row.path===relative);if(!row)return input;
 if(hash(input)!==row.predecessor_sha256)throw Error('Profile presentation predecessor changed');
 let output=input;for(const patch of profilePresentationPatches.filter(row=>row.path===relative)){
  if(output.split(patch.before).length-1!==patch.count)throw Error('Profile presentation anchor changed');
  output=output.replace(patch.before,patch.after);
 }
 if(hash(output)!==row.output_sha256)throw Error('Profile presentation output changed');return output;
}
export function restoreProfilePresentationOutput(output,relative){
 assertProfilePresentationScope();const row=profilePresentationManifest.find(row=>row.path===relative);if(!row)return output;
 if(hash(output)!==row.output_sha256)throw Error('Profile presentation output changed');
 let input=output;for(const patch of profilePresentationPatches.filter(row=>row.path===relative).reverse()){
  if(input.split(patch.after).length-1!==patch.count)throw Error('Profile presentation reverse anchor changed');
  input=input.replace(patch.after,patch.before);
 }
 if(hash(input)!==row.predecessor_sha256)throw Error('Profile presentation predecessor changed');return input;
}
