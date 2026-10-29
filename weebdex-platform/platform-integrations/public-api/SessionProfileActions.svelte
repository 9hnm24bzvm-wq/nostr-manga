<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {page} from '$app/state';
 import {Session} from '$lib/stores';
 import {Button} from '$lib/components/ui/button';
 import AccountActionGate from './AccountActionGate.svelte';
 import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
 import {loadSessionProfileActions,type NativeProfileActionsView} from './session-profile-actions';
 const {targetId,requestedId,nativeEligible}:{targetId:string;requestedId:string;nativeEligible:boolean}=$props();
 let client:Awaited<ReturnType<typeof loadSessionProfileActions>>|undefined,epoch=0,disposed=false;
 let pending=$state(false),current=$state<NativeProfileActionsView>(),error=$state(''),notice=$state(''),access=$state<NativeAccountIssue>();
 function guard(){const revision=epoch,target=targetId,request=requestedId,location=page.url.href,session=$Session;return()=>{if(disposed||!nativeEligible||epoch!==revision||targetId!==target||requestedId!==request||page.url.href!==location||session!==$Session)throw Error('The selected profile or account changed. Reload this profile.');};}
 function clear(){epoch++;client?.dispose();client=undefined;current=undefined;pending=false;error='';notice='';access=undefined;}
 $effect(()=>{targetId;requestedId;nativeEligible;page.url.href;$Session;untrack(()=>{clear();if(nativeEligible&&$Session?.identity?.id)void load();});});
 onDestroy(()=>{disposed=true;clear();});
 async function load(){if(pending||!nativeEligible||!$Session?.identity?.id)return;const assert=guard();pending=true;error='';notice='';access=undefined;current=undefined;try{const selected=client??await loadSessionProfileActions(targetId,requestedId,$Session.identity.id,assert);try{assert();}catch(cause){selected.dispose();throw cause;}client=selected;const value=await selected.reload();assert();current=value;}catch(cause){try{assert();current=client?.view();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Account actions could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function block(){if(pending||!client||!current||current.needsRecheck||current.unconfirmed)return;const selected=client,blocked=!current.blocked;if(!confirm(`${blocked?'Hide':'Show'} uploads by ${current.targetName} for your account?`))return;const assert=guard();pending=true;error='';notice='';try{assert();current=await selected.setBlocked(blocked);assert();notice=blocked?'Uploader blocked.':'Uploader unblocked.';}catch(cause){try{assert();current=selected.view();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'The change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function checkStatus(){if(pending||!client||!current?.unconfirmed)return;const selected=client,assert=guard();pending=true;error='';notice='';try{assert();const result=await selected.reconcile();assert();current=result;notice='Current block status checked. You can continue.';}catch(cause){try{assert();current=selected.view();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'The earlier change is still unconfirmed.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function keepCurrent(){if(pending||!client||!current?.canResolve)return;if(!confirm(`Keep the current uploader setting for ${current.targetName} and finish checking the earlier request? This will not block or unblock anyone.`))return;const selected=client,assert=guard();pending=true;error='';notice='';try{assert();const result=await selected.resolve();assert();current=result;notice='The current setting is confirmed. You can continue.';}catch(cause){try{assert();current=selected.view();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'The earlier change is still unconfirmed.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
{#if nativeEligible&&$Session?.identity?.id}
 <section class="basis-full space-y-2" aria-label="Account profile actions">
  <AccountActionGate issue={access} onready={load}/>
  {#if current}
   {#if current.settingsHref&&!current.needsRecheck}<a class="inline-flex min-h-11 items-center rounded border px-3 underline" href={current.settingsHref}>Edit profile</a>{/if}
   {#if !current.self&&current.canBlock}
    {#if current.followed&&!current.blocked}<p class="text-sm">Unfollow this reader before blocking them. Your follows are never changed automatically.</p>{/if}
    <Button type="button" variant="outline" disabled={pending||current.needsRecheck||current.unconfirmed||current.followed&&!current.blocked} onclick={block}>{current.blocked?'Unblock uploader':'Block uploader'}</Button>
   {/if}
   {#if current.unconfirmed}<p role="alert" class="text-sm">Your previous change is still unconfirmed and will not be sent again. Current setting: {current.blocked?'blocked':'not blocked'}.</p><Button type="button" variant="outline" disabled={pending} onclick={checkStatus}>Check status</Button>{#if current.canResolve}<Button type="button" variant="outline" disabled={pending} onclick={keepCurrent}>Keep current setting</Button>{/if}{/if}
  {/if}
  {#if error||current?.needsRecheck}<Button type="button" variant="outline" disabled={pending} onclick={load}>Reload account actions</Button>{/if}
  {#if error}<p role="alert" class="text-sm">{error}</p>{/if}{#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
 </section>
{/if}
