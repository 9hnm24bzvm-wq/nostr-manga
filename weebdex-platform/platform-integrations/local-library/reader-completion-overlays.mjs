// Exact correction of read-state source precedence. No mutation/admission bypass.
// Bounded badges already validate their own current account/page/catalogue key;
// whole-history raw-session observations apply only to the non-bounded fallback.
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const readCompletionPatches=[
  {
    "label": "bounded visible badges use their own current authority; legacy fallback keeps raw-session guard",
    "before": "const isRead = $derived($localLibraryState.mode === \"account\" && historyObservationSession !== $Session ? undefined : visibleBadges ? visibleBadges.value($badgeState, chapter.id) :",
    "after": "const isRead = $derived(visibleBadges ? visibleBadges.value($badgeState, chapter.id) : $localLibraryState.mode === \"account\" && historyObservationSession !== $Session ? undefined :",
    "count": 1
  }
];
export const readCompletionManifest=[
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "predecessor_sha256": "8422a003b2ae604b9ccd6c02b38978d99c8cf5522e8e86012fd3936d19b28c7c",
    "output_sha256": "fb226f891e26e79e1fa8b2e58ddbc4a731ae22bade30e46b09eae3ca95909c85"
  }
];
export function assertReadCompletionScope(){if(readCompletionPatches.length!==1||hash(JSON.stringify(readCompletionPatches))!="59f8396f11c2ebd1bf0b51c80887792229c16df13bc9f03359b1fa6185078557"||hash(JSON.stringify(readCompletionManifest))!="3559b1c7b4922afe27d9e4bc83ab8b466e08de04d5a747afbca5a72749524db2")throw Error('Unreviewed read-completion source scope');}
export function applyReadCompletionOutput(input,relative){assertReadCompletionScope();const entry=readCompletionManifest.find(row=>row.path===relative);if(!entry)return input;if(hash(input)!==entry.predecessor_sha256)throw Error('Read-completion predecessor source changed');let output=input;for(const row of readCompletionPatches){if(output.split(row.before).length-1!==row.count)throw Error('Read-completion exact anchor changed');output=output.split(row.before).join(row.after);}if(hash(output)!==entry.output_sha256||restoreReadCompletionOutput(output,relative)!==input)throw Error('Read-completion output source changed');return output;}
export function restoreReadCompletionOutput(output,relative){assertReadCompletionScope();const entry=readCompletionManifest.find(row=>row.path===relative);if(!entry)return output;if(hash(output)!==entry.output_sha256)throw Error('Read-completion output source changed');let previous=output;for(const row of [...readCompletionPatches].reverse()){if(previous.split(row.after).length-1!==row.count)throw Error('Read-completion reverse anchor changed');previous=previous.split(row.after).join(row.before);}if(hash(previous)!==entry.predecessor_sha256)throw Error('Read-completion predecessor source changed');return previous;}
