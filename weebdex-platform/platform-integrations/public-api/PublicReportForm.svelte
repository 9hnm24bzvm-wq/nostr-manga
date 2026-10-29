<script lang="ts">
 import { onDestroy, untrack } from 'svelte';
 import AccountActionGate from './AccountActionGate.svelte';
 import { nativeAccountIssue, type NativeAccountIssue } from './native-account-scope';
 import { Session } from '$lib/stores';
 import { Button, buttonVariants } from '$lib/components/ui/button';
 import * as Dialog from '$lib/components/ui/dialog';
 import * as Combobox from '$lib/components/ui/combobox';
 import { Textarea } from '$lib/components/ui/textarea';
 import { Separator } from '$lib/components/ui/separator';
 import { Spinner } from '$lib/components/ui/spinner';
 import ReportMangaCard from '$lib/components/ReportMangaCard.svelte';
 import ReportChapterCard from '$lib/components/ReportChapterCard.svelte';
 import ReportAuthorCard from '$lib/components/ReportAuthorCard.svelte';
 import ReportGroupCard from '$lib/components/ReportGroupCard.svelte';
 import ReportUserCard from '$lib/components/ReportUserCard.svelte';
 import { createSessionReportController, type SignedReportReason } from './session-report-client';
 let {entity,entityType,onClose}:{entity:User|Manga|Author|Chapter|ScanlationGroup|Thread|Reply;entityType:string;onClose:()=>void}=$props();
 let controller:Awaited<ReturnType<typeof createSessionReportController>>|undefined;
 let disposed=false,generation=0,busy=$state(false),pending=$state(false),reasons=$state<SignedReportReason[]>([]),selectedReasonId=$state(''),details=$state(''),message=$state(''),accountName=$state(''),sent=$state(false);
 let access=$state<NativeAccountIssue>();
 const reason=$derived(reasons.find(row=>row.id===selectedReasonId));
 const valid=$derived(!!reason&&(!reason.details_required||!!details.trim()));
 function reset(){generation++;controller?.dispose();controller=undefined;busy=false;pending=false;reasons=[];selectedReasonId='';details='';message='';accountName='';sent=false;access=undefined;}
 $effect(()=>{const identity=$Session;const target=entity.id;const category=entityType;untrack(()=>{reset();if($Session?.identity?.id)void load();});});
 onDestroy(()=>{disposed=true;reset();});
 function capture(){const token=generation,href=window.location.href,target=entity.id,category=entityType,session=$Session;return()=>{if(session!==$Session||disposed||token!==generation||window.location.href!==href||entity.id!==target||entityType!==category)throw Error('The report subject, account or dialog changed. No stale action was continued.');};}
 async function load(){if(busy||!$Session?.identity?.id||sent)return;const check=capture(),token=generation;busy=true;message='';access=undefined;reasons=[];selectedReasonId='';controller?.dispose();controller=undefined;try{const next=await createSessionReportController(entityType,entity.id,$Session!.identity.id,check);try{check();}catch(error){next.dispose();throw error;}controller=next;accountName=next.account.name;pending=next.pending();reasons=await next.read();check();}catch(error){if(!disposed&&token===generation){access=nativeAccountIssue(error);message=error instanceof Error?error.message:'Report reasons were not confirmed.';}}finally{if(!disposed&&token===generation)busy=false;}}
 async function submit(event?:SubmitEvent){event?.preventDefault();if(busy||pending||sent||!valid||!controller)return;const check=capture(),token=generation,owner=controller;busy=true;message='';try{await owner.submit(selectedReasonId,details);check();sent=true;message='Report sent to the site’s moderators.';}catch(error){if(!disposed&&token===generation){access=nativeAccountIssue(error);message=error instanceof Error?error.message:'Report submission was not confirmed.';reasons=[];selectedReasonId='';}}finally{if(!disposed&&token===generation){busy=false;pending=owner.pending();}}}
</script>
<form class="space-y-5" onsubmit={submit}>
 <div class="space-y-2">
  <div class="space-y-1">
   <strong class="block font-semibold">Subject</strong>
   {#if entityType==='manga'}<ReportMangaCard manga={entity as Manga}/>{:else if entityType==='chapter'}<ReportChapterCard chapter={entity as Chapter}/>{:else if entityType==='author'}<ReportAuthorCard author={entity as Author}/>{:else if entityType==='scanlation_group'}<ReportGroupCard group={entity as ScanlationGroup}/>{:else if entityType==='user'}<ReportUserCard user={entity as User}/>{/if}
  </div>
  <Separator/>
  <p class="text-muted-foreground text-sm">Reports are sent privately to the site’s moderators. Other readers cannot see them.</p>
  <AccountActionGate issue={access} onready={load} />
  {#if accountName}<p class="text-sm">Reporting as {accountName}</p>{/if}
  {#if message}<p role={sent?'status':'alert'}>{message}</p>{/if}
  {#if pending}<p role="alert">We could not confirm whether your previous report was received. It will not be sent again automatically, even if you close or reload this form. <a class="underline" href="/my/reports">Check reports about this item</a>. A report missing from that list does not prove it was never received, so sending again remains blocked.</p>{/if}
  <p><a class="underline" href="/my/reports">My reports</a></p>
  {#if !sent}<Button type="button" variant="outline" disabled={busy||!$Session?.identity?.id} onclick={load}>{reasons.length?'Reload report reasons':'Continue to report'}</Button>{/if}
  {#if !reasons.length && $Session?.identity?.id && !sent}<p class="text-xs text-muted-foreground">Choose a reason and explain the problem. Your report is only sent when you choose Send.</p>{/if}
  <Combobox.Root type="single" bind:value={selectedReasonId} disabled={busy||pending||sent||!reasons.length} required>
   <Combobox.Trigger class="w-full border-none">{reason?.reason??'Select a reason'}</Combobox.Trigger>
   <Combobox.Content>{#each reasons as item (item.id)}<Combobox.Item value={item.id}>{item.reason}</Combobox.Item>{/each}</Combobox.Content>
  </Combobox.Root>
  <Textarea class="min-h-35 border-none !ring-0" placeholder="Additional Explanation {reason?.details_required?'(required)':''}" bind:value={details} required={reason?.details_required} disabled={busy||pending||sent||!reasons.length}/>
 </div>
 <Dialog.Footer>
  <Button class={buttonVariants({class:'border-none',variant:'outline'})} type="button" onclick={()=>{reset();onClose();}}>{sent?'Close':'Cancel'}</Button>
  <Button class="w-30" type="submit" disabled={busy||pending||sent||!valid}>{#if busy}<Spinner/>{:else}Send{/if}</Button>
 </Dialog.Footer>
</form>
