// Exact lifecycle successor: clear old private data before child search effects.
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const reportLifecyclePatches=[
  {
    "before": "\t$effect(() => { $privateFeedIdentity; privateSearchGeneration++; feed = undefined; });\r\n",
    "after": ""
  },
  {
    "before": "\tlet feed = $state<ReportFeedResponse>();",
    "after": "\tlet feed = $state<ReportFeedResponse>();\r\n\t// Invalidate before the child starts its next account-scoped search.\r\n\t$effect.pre(() => { $privateFeedIdentity; privateSearchGeneration++; feed = undefined; });"
  }
];
export const reportLifecycleManifest=[
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "original_sha256": "8a5ddb5387ecf78a376d2cffe6d5b0214ea8359895e7c438730156b811363f5a",
    "predecessor_sha256": "ae1e06e9552d8e28c1311297baeb3d4be9c5d94ca9442eae7bab6890a8977e64",
    "output_sha256": "cb293a6ee787f0f05e5235603cd4b27e25e63574f107fe571ed6d048e1c1b7b4"
  }
];
export function assertReportLifecycleScope(){
 if(hash(JSON.stringify(reportLifecyclePatches))!=="6521960e3a4c49413eaf961c884bce8c03e7f8266a0bc61b29281b128b98eac7"||hash(JSON.stringify(reportLifecycleManifest))!=="c38749eee0b3059d28005ea6781c377ef0af3c7daf0d1bc96aee8da80b99c057")throw Error('Unreviewed report lifecycle scope');
}
export function applyReportLifecycleOutput(input,relative){
 assertReportLifecycleScope();const row=reportLifecycleManifest.find(row=>row.path===relative);if(!row)return input;
 if(hash(input)!==row.predecessor_sha256)throw Error('Report lifecycle predecessor changed');
 let output=input;for(const patch of reportLifecyclePatches){if(output.split(patch.before).length!==2)throw Error('Report lifecycle anchor changed');output=output.replace(patch.before,patch.after);}
 if(hash(output)!==row.output_sha256)throw Error('Report lifecycle output changed');return output;
}
export function restoreReportLifecycleOutput(output,relative){
 assertReportLifecycleScope();const row=reportLifecycleManifest.find(row=>row.path===relative);if(!row)return output;
 if(hash(output)!==row.output_sha256)throw Error('Report lifecycle output changed');
 let input=output.replace(reportLifecyclePatches[1].after,reportLifecyclePatches[1].before);
 const anchor='\tlet privateSearchGeneration = 0;\r\n';
 if(input.split(anchor).length!==2)throw Error('Report lifecycle reverse anchor changed');
 input=input.replace(anchor,anchor+reportLifecyclePatches[0].before);
 if(hash(input)!==row.predecessor_sha256)throw Error('Report lifecycle predecessor changed');return input;
}
