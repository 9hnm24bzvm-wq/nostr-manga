// Presentation-only exact successor to the frozen native account-library output.
// Inactive scaled icons must not expose a false role=status Loading announcement.
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const titleSpinnerPatches=[
  {
    "label": "isUpdatingReadingStatus status exists only while pending",
    "before": "\t\t\t\t\t\t\t<Spinner\r\n\t\t\t\t\t\t\t\tclass={cn(\"absolute top-0 left-0 scale-0 transition-all\", isUpdatingReadingStatus && \"scale-100 \")}\r\n\t\t\t\t\t\t\t/>",
    "after": "\t\t\t\t\t\t\t{#if isUpdatingReadingStatus}\r\n\t\t\t\t\t\t\t<Spinner\r\n\t\t\t\t\t\t\t\tclass={cn(\"absolute top-0 left-0 scale-0 transition-all\", isUpdatingReadingStatus && \"scale-100 \")}\r\n\t\t\t\t\t\t\t/>\r\n\t\t\t\t\t\t\t{/if}",
    "count": 1
  },
  {
    "label": "isUpdatingNotificationStatus status exists only while pending",
    "before": "\t\t\t\t\t\t<Spinner class={cn(\"absolute scale-0 transition-all\", isUpdatingNotificationStatus && \"scale-100 \")} />",
    "after": "\t\t\t\t\t\t{#if isUpdatingNotificationStatus}\r\n\t\t\t\t\t\t<Spinner class={cn(\"absolute scale-0 transition-all\", isUpdatingNotificationStatus && \"scale-100 \")} />\r\n\t\t\t\t\t\t{/if}",
    "count": 1
  }
];
export const titleSpinnerManifest=[
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "predecessor_sha256": "05b0537f94ed4cb73f61fff4c4100198a0549ffec0131194e5f1dca64e86b41f",
    "output_sha256": "69e7bc1a273a57d2ba92d977d2a9fc4c883e11cc3cd3170bb7e331f2dff0cb4d",
    "business_script_unchanged": true
  }
];
export function assertTitleSpinnerScope(){
 if(titleSpinnerPatches.length!==2||hash(JSON.stringify(titleSpinnerPatches))!=="fbf6bb264e1354d054bfb07ac90885501a48640f7ba7bcfb7aa3bee56d5dcd19"||hash(JSON.stringify(titleSpinnerManifest))!=="5a5dae68d8e504d95ed0557949dbfed7dc87ec0b6c245fd545e932ff25e0b0de")throw Error('Unreviewed title-spinner presentation scope');
}
export function applyTitleSpinnerOutput(input,relative){
 assertTitleSpinnerScope();const entry=titleSpinnerManifest.find(row=>row.path===relative);if(!entry)return input;
 if(hash(input)!==entry.predecessor_sha256)throw Error('Title-spinner predecessor source changed');
 let output=input;for(const patch of titleSpinnerPatches){if(output.split(patch.before).length-1!==patch.count)throw Error('Title-spinner exact anchor changed');output=output.split(patch.before).join(patch.after);}
 if(hash(output)!==entry.output_sha256)throw Error('Title-spinner output source changed');
 if(restoreTitleSpinnerOutput(output,relative)!==input)throw Error('Title-spinner reversal changed');return output;
}
export function restoreTitleSpinnerOutput(output,relative){
 assertTitleSpinnerScope();const entry=titleSpinnerManifest.find(row=>row.path===relative);if(!entry)return output;
 if(hash(output)!==entry.output_sha256)throw Error('Title-spinner output source changed');
 let previous=output;for(const patch of [...titleSpinnerPatches].reverse()){if(previous.split(patch.after).length-1!==patch.count)throw Error('Title-spinner reverse anchor changed');previous=previous.split(patch.after).join(patch.before);}
 if(hash(previous)!==entry.predecessor_sha256)throw Error('Title-spinner predecessor source changed');return previous;
}
