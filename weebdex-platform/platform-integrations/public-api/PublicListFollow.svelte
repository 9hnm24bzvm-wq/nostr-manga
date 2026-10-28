<script lang="ts">
 import IdentityGate from '../local-library/IdentityGate.svelte';
 import {onDestroy,untrack} from 'svelte';
 import {ListPlus} from '@lucide/svelte';
 import {Button} from '$lib/components/ui/button';
 import * as Dialog from '$lib/components/ui/dialog';
 import {signerIdentity} from '../local-library/publication';
 import {loadListFollow,type ListFollowState} from './list-follow-client';
 const {listId,head,unavailable=false}:{listId:string;head?:string;unavailable?:boolean}=$props();
 let lastListContext=JSON.stringify([listId,head]);
 let open=$state(false),pending=$state(false),error=$state(''),notice=$state(''),current=$state<ListFollowState>();
 let client:Awaited<ReturnType<typeof loadListFollow>>|undefined,epoch=0,disposed=false;
 function guard(){const revision=epoch,id=listId,snapshot=head,owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||!open||epoch!==revision||listId!==id||head!==snapshot||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The selected list or account changed. Reopen follow options.');};}
 function clear(){epoch++;client=undefined;current=undefined;pending=false;error='';notice='';}
 $effect(()=>{const id=listId,snapshot=head,owner=$signerIdentity.owner,generation=$signerIdentity.generation;untrack(()=>{clear();const next=JSON.stringify([id,snapshot]);if(next!==lastListContext)open=false;lastListContext=next;});});
 onDestroy(()=>{disposed=true;epoch++;});
 async function load(){if(pending||!$signerIdentity.owner)return;const assert=guard();pending=true;error='';notice='';client=undefined;current=undefined;try{const selected=await loadListFollow(listId,head,assert);assert();const state=await selected.reload();assert();client=selected;current=state;}catch(cause){try{assert();error=cause instanceof Error?cause.message:'Follow options could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 function setOpen(value:boolean){if(!value){open=false;clear();return;}open=true;if($signerIdentity.owner)void load();}
 async function save(followed:boolean){if(pending||!client||!current||current.needsRecheck)return;const assert=guard(),selected=client;pending=true;error='';notice='';try{const result=followed?await selected.follow():await selected.unfollow();assert();current=result;notice=followed?'List followed.':'Saved follow removed.';}catch(cause){try{assert();current=selected.view();error=cause instanceof Error?cause.message:'The follow change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
<Button variant="outline" onclick={()=>setOpen(true)}><ListPlus/>{unavailable?'Remove saved follow':'Follow list'}</Button>
<Dialog.Root {open} onOpenChange={setOpen}>
 <Dialog.Content class="sm:max-w-sm">
  <Dialog.Header><Dialog.Title>Follow this list</Dialog.Title><Dialog.Description>Keep public lists you enjoy in your following view.</Dialog.Description></Dialog.Header>
  <IdentityGate show={!$signerIdentity.owner} onfinish={() => { if ($signerIdentity.owner) void load(); }} />
 {#if !$signerIdentity.owner}
   <p class="text-sm text-muted-foreground">Create or connect your public identity to follow lists with an account on this instance.</p>
  {:else if current}
   <p class="text-sm text-muted-foreground">Following as {current.username} on this instance.</p>
   {#if unavailable}<p class="text-sm text-muted-foreground">This list is unavailable. You can still remove its saved follow.</p>{/if}
   <div class="flex flex-wrap gap-2">
    {#if !unavailable&&!current.followed}<Button disabled={pending||current.needsRecheck} onclick={()=>save(true)}>Follow list</Button>{/if}
    {#if unavailable||current.followed}<Button variant="outline" disabled={pending||current.needsRecheck} onclick={()=>save(false)}>Unfollow</Button>{/if}
    {#if current.needsRecheck}<Button variant="outline" disabled={pending} onclick={load}>Reload follow status</Button>{/if}
   </div>
  {:else if !pending}<Button variant="outline" onclick={load}>Load follow options</Button>{/if}
  {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking your followed lists…</p>{/if}
  {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
  {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
 </Dialog.Content>
</Dialog.Root>
