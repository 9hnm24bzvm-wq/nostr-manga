<script lang="ts">
 import {onDestroy,untrack,tick} from 'svelte';
 import {get} from 'svelte/store';
 import {publicLibraryCandidates} from '../public-api/library-catalogue';
 import {nativeAuthority} from '../frontend-behavior/mutation-authority';
 import {accountLibrary,refreshAccountLibraryTitles} from './native-account-library';
 import {currentDeviceSnapshot,assertCatalogueScope,assertCurrentCatalogueScopeValue} from './library';
 import {prepareGuestAccountAdoption,copyGuestAccountAdoption,GUEST_ADOPTION_BATCH,type GuestAdoptionReview,type GuestAdoptionReceipt} from './guest-account-adoption';
 const {assertViewCurrent=()=>{},onBusyChange=()=>{},onCopied=async()=>{}}:{assertViewCurrent?:()=>void;onBusyChange?:(busy:boolean)=>void;onCopied?:(assertCopyCurrent:()=>void)=>Promise<void>}=$props();
 let review=$state.raw<GuestAdoptionReview>(),receipt=$state<GuestAdoptionReceipt>(),pending=$state<'preview'|'copy'|''>(''),error=$state(''),notice=$state(''),page=$state(1),pages=$state(1),stopRequested=$state(false),uncertain=$state(false);
 let names=$state<Record<string,string>>({});
 let epoch=0,disposed=false;
 let previewButton=$state<HTMLButtonElement>(),reviewHeading=$state<HTMLHeadingElement>();
 function reset(){epoch++;review=undefined;names={};receipt=undefined;pending='';error='';notice='';page=1;pages=1;stopRequested=false;uncertain=false;}
 $effect(()=>{ $nativeAuthority.actor;$nativeAuthority.epoch;untrack(reset); });
 $effect(()=>{onBusyChange(!!pending);});
 onDestroy(()=>{disposed=true;epoch++;onBusyChange(false);});
 function guard(current:number){const owner=get(nativeAuthority);return()=>{assertViewCurrent();const now=get(nativeAuthority);if(disposed||current!==epoch||now.actor!==owner.actor||now.epoch!==owner.epoch)throw Error('This account view changed. Open it again before copying device saves.');};}
 async function source(check:()=>void){check();await assertCatalogueScope();check();const snapshot=await currentDeviceSnapshot();check();assertCurrentCatalogueScopeValue(snapshot.scope);return snapshot;}
 async function preview(){if(pending)return;const current=++epoch,check=guard(current);pending='preview';review=undefined;receipt=undefined;error='';notice='';uncertain=false;
  try{const access=await accountLibrary(check);access.check();const snapshot=await source(access.check);access.check();const value=await prepareGuestAccountAdoption(access.client,snapshot,page,access.check);access.check();const ids=[...new Set(value.items.flatMap(item=>item.mangaId?[item.mangaId]:[]))];names={};if(ids.length){const query=new URLSearchParams({page:'1',limit:'100',sort:'title'});for(const rating of ['safe','suggestive','erotica','pornographic'])query.append('contentRating',rating);try{const titles=await publicLibraryCandidates<{id:string;title:string}>('/manga','id',ids,query);access.check();names=Object.fromEntries(titles.map(row=>[row.id,row.title]));}catch{access.check();notice='Some title names could not be loaded. You can cancel and preview again before copying.';}}access.check();review=value;pages=value.pages;await tick();access.check();reviewHeading?.focus();}
  catch(cause){if(!disposed&&current===epoch)error=cause instanceof Error?cause.message:'Device saves could not be checked. Nothing was copied.';}
  finally{if(!disposed&&current===epoch)pending='';}
 }
 async function confirmCopy(){if(pending||!review)return;const selected=review,current=epoch,check=guard(current);review=undefined;pending='copy';stopRequested=false;error='';uncertain=false;receipt={completed:0,total:selected.copy,titles:0,chapters:0,lists:0};
  try{await copyGuestAccountAdoption(selected,{check,source:()=>source(check),cancelled:()=>stopRequested,onReceipt:value=>{check();receipt=value;refreshAccountLibraryTitles();}});check();notice='Your reviewed device saves were copied into your private account library. The device library is unchanged.';await onCopied(check);check();}
  catch(cause){if(!disposed&&current===epoch){uncertain=(cause as {outcome?:string})?.outcome==='uncertain';error=cause instanceof Error?cause.message:'The copy could not be completed. Check your account before trying again.';}}
  finally{if(!disposed&&current===epoch)pending='';}
 }
 async function cancelPreview(){epoch++;review=undefined;pending='';error='';notice='Preview cancelled. Nothing was copied.';await tick();if(!disposed)previewButton?.focus();}
 function changePage(next:number){page=next;review=undefined;receipt=undefined;error='';notice='';}
</script>
<section class="space-y-3 rounded border p-4" aria-label="Copy device saves to your account" aria-busy={!!pending}>
 <h3 class="font-medium">Bring your device saves into this account</h3>
 <p>Your account library works across devices when you sign in. This copies only the saves you review. It never deletes the library on this device or publishes your reading.</p>
 <p class="text-sm text-muted-foreground">We review up to {GUEST_ADOPTION_BATCH} saves at a time. Existing account saves, reading positions and lists with the same name are kept, not overwritten.</p>
 {#if pages>1}<nav class="flex gap-3" aria-label="Device save groups"><button type="button" disabled={!!pending||page===1} onclick={()=>changePage(page-1)}>Previous group</button><span>Group {page} of {pages}</span><button type="button" disabled={!!pending||page===pages} onclick={()=>changePage(page+1)}>Next group</button></nav>{/if}
 <button type="button" class="underline" bind:this={previewButton} disabled={!!pending} onclick={preview}>{receipt||error?'Check account and preview again':'Preview device saves'}</button>
 {#if pending==='preview'}<p role="status">Checking this group of device saves and your account…</p><button type="button" onclick={cancelPreview}>Cancel preview</button>{/if}
 {#if review}
  <section class="space-y-2" aria-label="Review device copy">
   <h4 bind:this={reviewHeading} tabindex="-1">Copy to <strong>{review.accountName}</strong>’s private account library</h4>
   <ul class="list-inside list-disc"><li>{review.titles} saved titles to add</li><li>{review.chapters} chapter progress entries to add</li><li>{review.lists} lists to add{review.listNames.length?': '+review.listNames.join(', '):''}</li></ul>
   <ul class="list-inside list-disc text-sm" aria-label="Saves included in this copy">{#each review.items as item}<li>{#if item.kind==='list'}List: {item.name}{:else}{names[item.mangaId!]||'Saved title'} — {item.kind==='title'?'save title':item.page?'reading position, page '+item.page:'chapter completed'}{item.kind==='chapter'&&item.page&&item.completed?' (completed)':''}{/if}</li>{/each}</ul>
   {#if review.keep}<p>{review.keep} existing account saves or same-name lists will be kept unchanged.</p>{/if}
   {#if review.unavailable}<p role="status">{review.unavailable} device saves cannot be copied in this group. They may be unavailable, use older links, have no reading position, or exceed a list limit. They stay safely on this device.</p>{/if}
   {#if !review.selected}<p>There are no saved titles, reading positions or lists on this device.</p>{/if}
   <p class="text-sm">Copying happens in small steps. If you stop or a request fails, confirmed saves stay in your account. Nothing is sent again automatically.</p>
   <div class="flex flex-wrap gap-3"><button type="button" disabled={!!pending||review.copy===0} onclick={confirmCopy}>Copy reviewed saves</button><button type="button" disabled={!!pending} onclick={cancelPreview}>Cancel</button></div>
  </section>
 {/if}
 {#if receipt}<p role="status">Confirmed {receipt.completed} of {receipt.total} saves: {receipt.titles} titles, {receipt.chapters} chapter progress entries and {receipt.lists} lists.</p>{/if}
 {#if pending==='copy'}<button type="button" disabled={stopRequested} onclick={()=>stopRequested=true}>{stopRequested?'Stopping after the current request…':'Stop copying'}</button><p class="text-sm">The current request may still finish. Stopping prevents the next save.</p>{/if}
 {#if notice}<p role="status">{notice}</p>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
 {#if uncertain}<p role="alert">The last save has an unknown result. Check your account before starting another preview. Confirmed counts above do not include that save.</p>{/if}
</section>
