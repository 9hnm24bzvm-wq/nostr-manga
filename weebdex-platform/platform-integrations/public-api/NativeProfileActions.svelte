<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {page} from '$app/state';
 import {Button} from '$lib/components/ui/button';
 import {signerIdentity} from '../local-library/publication';
 import {loadNativeProfileActions,type NativeProfileActionsView} from './native-profile-actions';
 const {targetId,requestedId,nativeEligible}:{targetId:string;requestedId:string;nativeEligible:boolean}=$props();
 let client:Awaited<ReturnType<typeof loadNativeProfileActions>>|undefined,epoch=0,disposed=false;
 let open=$state(false),pending=$state(false),current=$state<NativeProfileActionsView>(),error=$state(''),notice=$state('');
 const eligible=$derived(nativeEligible&&!!$signerIdentity.owner);
 function guard(){const revision=epoch,target=targetId,request=requestedId,location=page.url.href,owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||!open||!eligible||epoch!==revision||targetId!==target||requestedId!==request||page.url.href!==location||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The selected profile or signing account changed. Reopen account actions.');};}
 function close(){epoch++;client?.dispose();client=undefined;open=false;pending=false;current=undefined;error='';notice='';}
 $effect(()=>{targetId;requestedId;nativeEligible;page.url.href;$signerIdentity.owner;$signerIdentity.generation;untrack(close);});
 onDestroy(()=>{disposed=true;close();});
 async function load(){if(pending||!eligible)return;open=true;const assert=guard();pending=true;error='';notice='';current=undefined;try{const selected=client??await loadNativeProfileActions(targetId,requestedId,assert);try{assert();}catch(cause){selected.dispose();throw cause;}client=selected;const value=await selected.reload();assert();current=value;}catch(cause){try{assert();current=client?.view();error=cause instanceof Error?cause.message:'The bound account could not be verified.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function block(){if(pending||!client||!current||current.needsRecheck||current.unconfirmed)return;const selected=client,blocked=!current.blocked;if(!confirm(`${blocked?'Block':'Unblock'} ${current.targetName} for ${current.username} on this instance? Device preferences and the browser login stay unchanged.`))return;const assert=guard();pending=true;error='';notice='';try{assert();current=await selected.setBlocked(blocked);assert();notice=blocked?'Uploader blocked for this account.':'Uploader unblocked for this account.';}catch(cause){try{assert();current=selected.view();error=cause instanceof Error?cause.message:'The uploader change was not confirmed.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
{#if eligible}
 {#if !open}<Button type="button" variant="outline" onclick={load}>Use signing account</Button>
 {:else}<section class="basis-full space-y-2 rounded border p-3" aria-label="Signed native profile actions">
  {#if current}<p class="text-sm">Signing account: {current.username}<span class="block break-all text-xs">{current.accountId}</span></p>
   {#if current.settingsHref&&!current.needsRecheck}<a class="underline" href={current.settingsHref}>Settings</a>{/if}
   {#if !current.self&&current.canBlock}
    {#if current.followed&&!current.blocked}<p>Unfollow this user explicitly before blocking them. No follow is changed here.</p>{/if}
    <Button type="button" variant="outline" disabled={pending||current.needsRecheck||current.unconfirmed||current.followed&&!current.blocked} onclick={block}>{current.blocked?'Unblock':'Block'}</Button>
   {/if}
   {#if current.unconfirmed}<p role="alert">A previous uploader change is unconfirmed. It will not be replayed. Reload can show current state but cannot clear that pending outcome.</p>{/if}
  {/if}
  <Button type="button" variant="outline" disabled={pending} onclick={load}>Reload account actions</Button><Button type="button" variant="outline" onclick={close}>Close account actions</Button>
  {#if pending}<p role="status">Checking the selected signing account…</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}{#if notice}<p role="status">{notice}</p>{/if}
 </section>{/if}
{/if}
