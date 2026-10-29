// Explicit final-stage native-account library integration. No immutable source edits.
import fs from 'node:fs';import path from 'node:path';import {createHash} from 'node:crypto';
import {applyTitleSpinnerOutput,restoreTitleSpinnerOutput} from './title-spinner-overlays.mjs';
import {applyReadCompletionOutput,restoreReadCompletionOutput} from './reader-completion-overlays.mjs';
import {applyCardSpinnerOutput,restoreCardSpinnerOutput} from './card-spinner-overlays.mjs';
import {reportLifecycleManifest,applyReportLifecycleOutput,assertReportLifecycleScope} from './report-lifecycle-overlays.mjs';
import {profilePresentationManifest,applyProfilePresentationOutput,assertProfilePresentationScope} from './profile-presentation-overlays.mjs';
import {preferencesPresentationManifest,applyPreferencesPresentationOutput,assertPreferencesPresentationScope} from './preferences-presentation-overlays.mjs';
import {readPublicApiGate} from '../public-api/overlays.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');
export const accountLibraryPatches=JSON.parse(fs.readFileSync(new URL('./account-anchors.json',import.meta.url),'utf8'));
export const accountLibraryOverlayManifest=[
  {
    "path": "src/lib/components/MangaSearchEntryCard.svelte",
    "original_sha256": "378d0dee427e11f61166d9433f67eaf71155d127a6491a2156371b228338230c",
    "input_sha256": "e38ea2c265501df4fc7f5839946e619e3e07ec0c9cb120ee9392c34bc9b84920",
    "output_sha256": "8cb41720de1cd587ef1c8070a9ff071786247fb66aba64c10ae0883249ba92c5"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "original_sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "input_sha256": "11296404e965837cc128ef93aad0d85c04d41959bed840ed5b94d0529e9fb66e",
    "output_sha256": "05b0537f94ed4cb73f61fff4c4100198a0549ffec0131194e5f1dca64e86b41f"
  },
  {
    "path": "src/lib/components/ChapterFeedMangaEntry.svelte",
    "original_sha256": "fe05c6a639a507a1cfb082d3b3a322dfa998eeb1fc04064ba7690f9b6c0ed00b",
    "input_sha256": "6ba078a3ee2c131d1d5d70421f971ae03228d393ce342327c6b0d9a458c26b96",
    "output_sha256": "33783578347ef2445235d1493390d3f60f05cbb5c952a4009641d966ba47c95f"
  },
  {
    "path": "src/lib/components/reader/reader-history.svelte",
    "original_sha256": "ba14aa6c0d83334004b5d680d8cff52d842b7061bad6f31fbc7cc858d2f67052",
    "input_sha256": "3ed041fbc9d34e8979df7330b8bfa05c5cb17ca1eab9a3666986ea5ffe76ae5f",
    "output_sha256": "17ac9fb2146a1b1e7b47d1e2ddd352b2e802ae4abd78d0457a912dcd2485d662"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "original_sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "input_sha256": "78e48871b3f8c5f9d8065a53c271979bc36606685092af5f4ceb87e786dd897c",
    "output_sha256": "c14f088043b55b081e7c97515efb7c5936c2b865a6b2f3e619d59223c55b2d07"
  },
  {
    "path": "src/routes/(app)/library/+page.svelte",
    "original_sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
    "input_sha256": "70ed75821e7e05d8655f99fa641754ce2ccf6feb3b517618588c92d912381dc0",
    "output_sha256": "a5e4771e83fdef8f1a3accd8758d35b173698f8b627dac896fb51a1a22a44d57"
  },
  {
    "path": "src/lib/components/ChapterFeed.svelte",
    "original_sha256": "966b57b14dee30e3f1c4c2d93fe6b7d414f0ea0d7959fe86e162035a2fd3f13b",
    "input_sha256": "16677225d326b7e33d6586486390c148e5db53b2bb723dd54459b237edeb5b9c",
    "output_sha256": "b260d7176119a856ae40ad519d318b8b5a64f5b821e288995598844a512674ca"
  },
  {
    "path": "src/lib/components/ChapterFeedChapterEntry.svelte",
    "original_sha256": "30214055c6cb02bb5ffcded9c16234ed50fc69dbda8766a6cd1e9fe52f7e45ad",
    "input_sha256": "f636f6e06936f4155de7aad31f34f1b836c927cce8af072bd114841cc9bd5b26",
    "output_sha256": "f4c7b15bfee19342803962f80eb619043c9cf615c7377ff737f1f60036e946c8"
  },
  {
    "path": "src/lib/components/ChapterListEntry.svelte",
    "original_sha256": "cb8701ad0a233eb4ddea9940c9ca2c11cec221d847ee547f229fe6474ff30c4c",
    "input_sha256": "25ea506f97100b100cb9fcb409840a2017cab5238341cab4f3db888afc9b76f2",
    "output_sha256": "8422a003b2ae604b9ccd6c02b38978d99c8cf5522e8e86012fd3936d19b28c7c"
  }
];
export function assertAccountLibraryExceptionScope(){
 if(accountLibraryPatches.length!==51||hash(JSON.stringify(accountLibraryPatches))!=='322376beb2b3cca5c74ca040a067c7c94b7fd9e05d0bfcf7582b1fa8a77bd623')throw Error('Unreviewed account-library overlay scope');
 if(hash(JSON.stringify(accountLibraryOverlayManifest))!=='346f878ef1ac6fa338b8c5b4fff739b0aa9d45c708739f92c47adb37d7c7785f')throw Error('Unreviewed account-library stage hashes');
}
export function assertAccountLibraryOutput(input,output,relative){
 output=restoreCardSpinnerOutput(output,relative);
 output=restoreReadCompletionOutput(output,relative);
 output=restoreTitleSpinnerOutput(output,relative);
 const entry=accountLibraryOverlayManifest.find(row=>row.path===relative);if(!entry)throw Error('Unknown account-library stage target');
 if(hash(input)!==entry.input_sha256||hash(output)!==entry.output_sha256)throw Error('Account-library stage source changed: '+relative);
 let restored=output;for(const patch of accountLibraryPatches.filter(row=>row.path===relative).reverse()){
  if(!patch.after||restored.split(patch.after).length-1!==patch.count)throw Error('Account-library reverse anchor changed: '+patch.label);
  restored=restored.split(patch.after).join(patch.before);
 }
 if(restored!==input)throw Error('Account-library changed unapproved source: '+relative);
}
export function createAccountLibraryOverlays(frontend,{enabled=readPublicApiGate()}={}){
 assertAccountLibraryExceptionScope();assertReportLifecycleScope();assertProfilePresentationScope();assertPreferencesPresentationScope();const root=path.resolve(frontend);
 return{name:'native-account-library-final-stage',enforce:'pre',transform:{order:'pre',handler(input,id){
  if(!enabled||id.includes('?'))return null;const relative=path.relative(root,id).split(path.sep).join('/'),entry=accountLibraryOverlayManifest.find(row=>row.path===relative);
  if(!entry){
   const successors=[[reportLifecycleManifest,applyReportLifecycleOutput],[profilePresentationManifest,applyProfilePresentationOutput],[preferencesPresentationManifest,applyPreferencesPresentationOutput]];
   const matching=successors.filter(([manifest])=>manifest.some(row=>row.path===relative));if(!matching.length)return null;if(matching.length!==1)throw Error('Overlapping final successor scope: '+relative);
   const [manifest,apply]=matching[0],successor=manifest.find(row=>row.path===relative);
   if(hash(fs.readFileSync(path.join(root,relative)))!==successor.original_sha256)throw Error('Final successor immutable source changed: '+relative);
   return{code:apply(input,relative),map:null};
  }
  if(hash(fs.readFileSync(path.join(root,relative)))!==entry.original_sha256)throw Error('Account-library immutable source changed: '+relative);
  if(hash(input)!==entry.input_sha256)throw Error('Account-library composed input changed: '+relative);
  let code=input;for(const patch of accountLibraryPatches.filter(row=>row.path===relative)){
   if(code.split(patch.before).length-1!==patch.count)throw Error('Account-library exact anchor changed: '+patch.label);
   code=code.split(patch.before).join(patch.after);
  }code=applyTitleSpinnerOutput(code,relative);code=applyReadCompletionOutput(code,relative);code=applyCardSpinnerOutput(code,relative);assertAccountLibraryOutput(input,code,relative);return{code,map:null};
 }}};
}
