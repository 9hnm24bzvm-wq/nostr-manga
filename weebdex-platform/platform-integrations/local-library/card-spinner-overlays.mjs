// Exact inactive card-spinner presentation successor; all business script bytes remain unchanged.
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const cardSpinnerPatches=[
  {
    "label": "card loading status exists only while its existing mutation mutex is pending",
    "before": "<Spinner class={cn(\"!text-primary-foreground absolute scale-0 transition-all\", $mutex && \"scale-100 \")} />",
    "after": "{#if $mutex}<Spinner class={cn(\"!text-primary-foreground absolute scale-0 transition-all\", $mutex && \"scale-100 \")} />{/if}",
    "count": 1
  }
];
export const cardSpinnerManifest=[
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "predecessor_sha256": "8cb41720de1cd587ef1c8070a9ff071786247fb66aba64c10ae0883249ba92c5",
    "output_sha256": "a9b260b49f27b4fe909c76358c998bd12a5f0e123ba46af01395e321062d98fa"
  }
];
export function assertCardSpinnerScope(){if(cardSpinnerPatches.length!==1||hash(JSON.stringify(cardSpinnerPatches))!="ae36e9d46f7de54b7c9490942189d6a45c66364499ea80bfa38dcf6ed7744be0"||hash(JSON.stringify(cardSpinnerManifest))!="fefa626ee324e1b3903265b21affac17f49d19a6b79afaea0c460550d9384d15")throw Error('Unreviewed card-spinner source scope');}
export function applyCardSpinnerOutput(input,relative){assertCardSpinnerScope();const entry=cardSpinnerManifest.find(row=>row.path===relative);if(!entry)return input;if(hash(input)!==entry.predecessor_sha256)throw Error('Card-spinner predecessor source changed');let output=input;for(const row of cardSpinnerPatches){if(output.split(row.before).length-1!==row.count)throw Error('Card-spinner exact anchor changed');output=output.split(row.before).join(row.after);}if(hash(output)!==entry.output_sha256||restoreCardSpinnerOutput(output,relative)!==input)throw Error('Card-spinner output source changed');return output;}
export function restoreCardSpinnerOutput(output,relative){assertCardSpinnerScope();const entry=cardSpinnerManifest.find(row=>row.path===relative);if(!entry)return output;if(hash(output)!==entry.output_sha256)throw Error('Card-spinner output source changed');let previous=output;for(const row of [...cardSpinnerPatches].reverse()){if(previous.split(row.after).length-1!==row.count)throw Error('Card-spinner reverse anchor changed');previous=previous.split(row.after).join(row.before);}if(hash(previous)!==entry.predecessor_sha256)throw Error('Card-spinner predecessor source changed');return previous;}
