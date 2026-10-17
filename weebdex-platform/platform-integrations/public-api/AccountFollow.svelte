<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {UserPlus} from '@lucide/svelte';
 import {Button} from '$lib/components/ui/button';
 import * as Dialog from '$lib/components/ui/dialog';
 import {signerIdentity} from '../local-library/publication';
 import {publicUuid} from './validation';
 import {loadAccountFollow,type AccountFollowKind,type AccountFollowView} from './account-follow-client';
 const {kind,targetId,requestedId,expectedAccountId,label,onChanged=async()=>{}}:{kind:AccountFollowKind;targetId:string;requestedId:string;expectedAccountId?:string;label?:string;onChanged?:(view:AccountFollowView)=>unknown|Promise<unknown>}=$props();
 let open=$state(false),pending=$state(false),error=$state(''),notice=$state(''),current=$state<AccountFollowView>();
 let client:Awaited<ReturnType<typeof loadAccountFollow>>|undefined,epoch=0,disposed=false;
 function guard(){const revision=epoch,target=targetId,request=requestedId,sourceAccount=expectedAccountId,type=kind,owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||!open||epoch!==revision||targetId!==target||requestedId!==request||expectedAccountId!==sourceAccount||kind!==type||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The selected resource or account changed. Reopen following.');};}
 function clear(){epoch++;client=undefined;current=undefined;pending=false;error='';notice='';}
 $effect(()=>{const target=targetId,request=requestedId,sourceAccount=expectedAccountId,type=kind,owner=$signerIdentity.owner,generation=$signerIdentity.generation;untrack(()=>{clear();open=false;});});
 onDestroy(()=>{disposed=true;epoch++;});
 async function load(){if(pending||!$signerIdentity.owner)return;const assert=guard();pending=true;error='';notice='';client=undefined;current=undefined;try{if(publicUuid(targetId)!==publicUuid(requestedId))throw Error('Open this resource’s canonical page before following it.');const sourceAccount=expectedAccountId===undefined?undefined:publicUuid(expectedAccountId),selected=await loadAccountFollow(kind,targetId,assert);assert();if(sourceAccount!==undefined&&selected.view().accountId!==sourceAccount)throw Error('The following account changed. Refresh the following view.');const result=await selected.reload();assert();client=selected;current=result;}catch(cause){try{assert();error=cause instanceof Error?cause.message:'Account following could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 function setOpen(value:boolean){if(!value){open=false;clear();return;}open=true;if($signerIdentity.owner)void load();}
 async function save(followed:boolean){if(pending||!client||!current||current.needsRecheck)return;const assert=guard(),selected=client;pending=true;error='';notice='';try{const result=followed?await selected.follow():await selected.unfollow();assert();current=result;notice=followed?'Follow saved to account.':'Account follow removed.';try{await onChanged(result);assert();}catch{assert();notice+=' Refresh the following view to see its latest state.';}}catch(cause){try{assert();current=selected.view();error=cause instanceof Error?cause.message:'The account-follow change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
{#if $signerIdentity.owner}<Button class="border-none" type="button" variant="outline" onclick={()=>setOpen(true)}><UserPlus/>{label??(kind==='manga'?'Follow on account':'Follow')}</Button>{/if}
<Dialog.Root {open} onOpenChange={setOpen}>
 <Dialog.Content class="sm:max-w-sm">
  <Dialog.Header><Dialog.Title>Account following</Dialog.Title><Dialog.Description>{kind==='manga'?'Save an account follow for this title. Your device library stays as it is.':'Follow this '+(kind==='group'?'group':'user')+' with your account on this instance.'}</Dialog.Description></Dialog.Header>
  {#if current}
   <p class="text-sm text-muted-foreground">Following as {current.username} on this instance.</p>
   <div class="flex flex-wrap gap-2"><Button disabled={pending||current.needsRecheck} onclick={()=>save(!current!.followed)}>{current.followed?'Unfollow':'Follow'}</Button>{#if !current.followed&&current.version===undefined}<Button variant="outline" disabled={pending||current.needsRecheck} onclick={()=>save(false)}>Remove saved follow</Button>{/if}{#if current.needsRecheck}<Button variant="outline" disabled={pending} onclick={load}>Reload account follow</Button>{/if}</div>
  {:else if !pending}<Button variant="outline" onclick={load}>Load account follow</Button>{/if}
  {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking account following…</p>{/if}
  {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
  {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
 </Dialog.Content>
</Dialog.Root>
