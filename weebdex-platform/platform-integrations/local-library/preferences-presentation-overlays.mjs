// Exact presentation-only successor; never changes saved preferences or filters.
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const preferencesPresentationPatches=[
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "before": "{#each [28, 42, 56, 70, 84, 98] as value}",
    "after": "{#each [...new Set([Number(draft.items_per_page), 20, 28, 42, 56, 70, 84, 98])].filter(value => Number.isInteger(value) && value >= 1 && value <= 100).sort((a, b) => a - b) as value}"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "before": "+{draft.languages.length}",
    "after": "{draft.languages.length} selected"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "before": "+{draft.translated_languages.length}",
    "after": "{draft.translated_languages.length} selected"
  },
  {
    "path": "src/lib/components/ChapterListFilter.svelte",
    "before": "[Forced]\r\n",
    "after": "Your preference:\r\n"
  }
];
export const preferencesPresentationManifest=[
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "original_sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "predecessor_sha256": "83207291cbba63386a7bee7e724e8e38695dc5736e8c7e8e3bfe8f9ffd4513f8",
    "output_sha256": "f87f5ed651353f6662453b2586ea15528c9ad8df367c25b4a45ddc5d11ff0e32"
  },
  {
    "path": "src/lib/components/ChapterListFilter.svelte",
    "original_sha256": "2f38ac6741bb02cff665ec0885929ce721082eae1afbc05ce8a3f1bd6f3db6fa",
    "predecessor_sha256": "f2c3e4ce3d213d24aa5e6e096b69330770783b784a6d1297d1d928f66dbb88ac",
    "output_sha256": "bf1d462e30bf7912c4a2039d7a028ec00515de0a39322f4291a2240879a66f67"
  }
];
export function assertPreferencesPresentationScope(){if(hash(JSON.stringify(preferencesPresentationPatches))!=="01b566f6edf057b55d53f817a220f3bc55b470977ed0f47e04c397cf58de1377"||hash(JSON.stringify(preferencesPresentationManifest))!=="b55598c9760240386449082742c4b2806695c076295c0930763ed001f1a065d6")throw Error('Unreviewed preferences presentation scope');}
export function applyPreferencesPresentationOutput(input,relative){assertPreferencesPresentationScope();const row=preferencesPresentationManifest.find(row=>row.path===relative);if(!row)return input;if(hash(input)!==row.predecessor_sha256)throw Error('Preferences presentation predecessor changed');let output=input;for(const patch of preferencesPresentationPatches.filter(row=>row.path===relative)){if(output.split(patch.before).length!==2)throw Error('Preferences presentation anchor changed');output=output.replace(patch.before,patch.after);}if(hash(output)!==row.output_sha256)throw Error('Preferences presentation output changed');return output;}
export function restorePreferencesPresentationOutput(output,relative){assertPreferencesPresentationScope();const row=preferencesPresentationManifest.find(row=>row.path===relative);if(!row)return output;if(hash(output)!==row.output_sha256)throw Error('Preferences presentation output changed');let input=output;for(const patch of preferencesPresentationPatches.filter(row=>row.path===relative).reverse()){if(input.split(patch.after).length!==2)throw Error('Preferences presentation reverse anchor changed');input=input.replace(patch.after,patch.before);}if(hash(input)!==row.predecessor_sha256)throw Error('Preferences presentation predecessor changed');return input;}
