<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {UserPlus} from '@lucide/svelte';
 import {Button} from '$lib/components/ui/button';
 import * as Dialog from '$lib/components/ui/dialog';
 import {signerIdentity} from '../local-library/publication';
 import {publicUuid} from './validation';
 import {loadAccountFollow,type AccountFollowKind,type AccountFollowView} from './account-follow-client';
 import {loadAccountReadingStatus,accountReadingStatuses,type AccountReadingStatus,type AccountReadingStatusView} from './account-reading-status-client';
 const {kind,targetId,requestedId,expectedAccountId,label,onChanged=async()=>{}}:{kind:AccountFollowKind;targetId:string;requestedId:string;expectedAccountId?:string;label?:string;onChanged?:(view:AccountFollowView)=>unknown|Promise<unknown>}=$props();
 let open=$state(false),pending=$state(false),error=$state(''),notice=$state(''),current=$state<AccountFollowView>();
 let reading=$state<AccountReadingStatusView>(),readingChoice=$state(''),needsReload=$state(false);
 let client:Awaited<ReturnType<typeof loadAccountFollow>>|undefined,readingClient:Awaited<ReturnType<typeof loadAccountReadingStatus>>|undefined,epoch=0,disposed=false,dialogAccountId:string|undefined;
 const readingLabels:Record<AccountReadingStatus,string>={reading:'Reading',plan_to_read:'Plan to read',completed:'Completed',on_hold:'On hold',re_reading:'Re-reading',dropped:'Dropped'};
 function guard(){const revision=epoch,target=targetId,request=requestedId,sourceAccount=expectedAccountId,type=kind,owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||!open||epoch!==revision||targetId!==target||requestedId!==request||expectedAccountId!==sourceAccount||kind!==type||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The selected resource or account changed. Reopen following.');};}
 function clear(){epoch++;dialogAccountId=undefined;client=undefined;readingClient=undefined;current=undefined;reading=undefined;readingChoice='';needsReload=false;pending=false;error='';notice='';}
 $effect(()=>{const target=targetId,request=requestedId,sourceAccount=expectedAccountId,type=kind,owner=$signerIdentity.owner,generation=$signerIdentity.generation;untrack(()=>{clear();open=false;});});
 onDestroy(()=>{disposed=true;epoch++;});
 async function readReading(follow:AccountFollowView,assert:()=>void){if(kind!=='manga'||!follow.followed)return undefined;const selected=await loadAccountReadingStatus(targetId,follow.accountId,assert);assert();const result=await selected.reload();assert();if(!result.followed)throw Error('Account following changed. Reload before editing reading status.');return{selected,result};}
 function installReading(observation:Awaited<ReturnType<typeof readReading>>){readingClient=observation?.selected;reading=observation?.result;readingChoice=observation?.result.status??'';}
 async function load(){if(pending||!$signerIdentity.owner)return;const assert=guard();pending=true;error='';notice='';needsReload=false;client=undefined;current=undefined;installReading(undefined);try{if(publicUuid(targetId)!==publicUuid(requestedId))throw Error('Open this resource’s canonical page before following it.');const sourceAccount=expectedAccountId===undefined?dialogAccountId:publicUuid(expectedAccountId),selected=await loadAccountFollow(kind,targetId,assert);assert();if(sourceAccount!==undefined&&selected.view().accountId!==sourceAccount)throw Error('The following account changed. Refresh the following view or reopen following.');dialogAccountId=selected.view().accountId;const result=await selected.reload();assert();const observed=await readReading(result,assert);assert();client=selected;current=result;installReading(observed);}catch(cause){try{assert();error=cause instanceof Error?cause.message:'Account following could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 function setOpen(value:boolean){if(!value){open=false;clear();return;}open=true;if($signerIdentity.owner)void load();}
 async function notify(result:AccountFollowView,assert:()=>void){try{await onChanged(result);assert();}catch{assert();notice+=' Refresh the following view to see its latest state.';}}
 async function save(followed:boolean){if(pending||!client||!current||current.needsRecheck||needsReload)return;const assert=guard(),selected=client;pending=true;error='';notice='';try{const result=followed?await selected.follow():await selected.unfollow();assert();current=result;installReading(undefined);notice=followed?'Follow saved to account.':'Account follow removed.';try{const observed=await readReading(result,assert);assert();installReading(observed);}catch(cause){assert();needsReload=true;error=cause instanceof Error?cause.message:'Reload following before editing reading status.';}await notify(result,assert);}catch(cause){try{assert();current=selected.view();error=cause instanceof Error?cause.message:'The account-follow change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function saveReading(){if(pending||needsReload||!client||!current?.followed||current.needsRecheck||!readingClient||!reading?.followed||reading.needsRecheck||!readingChoice||readingChoice===reading.status)return;const assert=guard(),selected=readingClient,membership=client,requested=readingChoice as AccountReadingStatus;pending=true;error='';notice='';try{const result=await selected.save(requested);assert();reading=result;if(readingChoice===requested)readingChoice=result.status??'';notice='Reading status saved to account.';const follow=await membership.reload();assert();current=follow;if(!follow.followed)throw Error('Account following changed. Reload before another change.');await notify(follow,assert);}catch(cause){try{assert();reading=selected.view();needsReload=true;error=cause instanceof Error?cause.message:'The reading-status change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}

</script>
{#if $signerIdentity.owner}<Button class="border-none" type="button" variant="outline" onclick={()=>setOpen(true)}><UserPlus/>{label??(kind==='manga'?'Follow on account':'Follow')}</Button>{/if}
<Dialog.Root {open} onOpenChange={setOpen}>
 <Dialog.Content class="sm:max-w-sm">
  <Dialog.Header><Dialog.Title>Account following</Dialog.Title><Dialog.Description>{kind==='manga'?'Save an account follow for this title. Your device library stays as it is.':'Follow this '+(kind==='group'?'group':'user')+' with your account on this instance.'}</Dialog.Description></Dialog.Header>
  {#if current}
   <p class="text-sm text-muted-foreground">Following as {current.username} on this instance.</p>
   {#if kind==='manga'&&current.followed&&reading}
    <div class="space-y-2"><label class="text-sm font-medium" for={'account-reading-status-'+targetId}>Reading status</label><select id={'account-reading-status-'+targetId} class="w-full rounded-md border bg-background px-3 py-2 text-sm" bind:value={readingChoice} disabled={pending||needsReload||current.needsRecheck||reading.needsRecheck}>{#each accountReadingStatuses as value}<option {value}>{readingLabels[value]}</option>{/each}</select><Button variant="outline" disabled={pending||needsReload||current.needsRecheck||reading.needsRecheck||readingChoice===reading.status} onclick={saveReading}>Save reading status</Button></div>
   {/if}
   <div class="flex flex-wrap gap-2"><Button disabled={pending||current.needsRecheck||needsReload} onclick={()=>save(!current!.followed)}>{current.followed?'Unfollow':'Follow'}</Button>{#if !current.followed&&current.version===undefined}<Button variant="outline" disabled={pending||current.needsRecheck||needsReload} onclick={()=>save(false)}>Remove saved follow</Button>{/if}{#if current.needsRecheck||needsReload}<Button variant="outline" disabled={pending} onclick={load}>Reload account follow</Button>{/if}</div>
  {:else if !pending}<Button variant="outline" onclick={load}>Load account follow</Button>{/if}
  {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking account library…</p>{/if}
  {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
  {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
 </Dialog.Content>
</Dialog.Root>
