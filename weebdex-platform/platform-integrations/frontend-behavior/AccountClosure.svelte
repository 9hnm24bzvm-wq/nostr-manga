<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {get} from 'svelte/store';
 import {page} from '$app/state';
 import Session from '$lib/stores/Session';
 import HTTP from '$lib/HTTP';
 import {PUBLIC_API_URL} from '$env/static/public';
 import {authenticatedMutation} from './authenticated-mutation';
 import {captureNativeAuthority,sameNativeAuthority} from './mutation-authority';
 import {boundAccountAccess} from '../public-api/bound-account-request';
 import {signerIdentity} from '../local-library/publication';
 import {createAccountClosure,browserExclusive,acknowledgement,nativeClosureBase,type ClosureView,type ClosureAccess} from './account-closure-client';
 let mode=$state<'native'|'signed'|''>(''),current=$state<ClosureView>(),prepared=$state(false),pending=$state(false),phrase=$state(''),error=$state(''),notice=$state('');
 let epoch=0,disposed=false,client:ReturnType<typeof createAccountClosure>|undefined;
 function reset(){epoch++;client=undefined;current=undefined;prepared=false;phrase='';pending=false;error='';notice='';}
 $effect(()=>{mode;page.url.href;$signerIdentity.owner;$signerIdentity.generation;$Session;untrack(reset);});
 onDestroy(()=>{disposed=true;epoch++;});
 function guard(){const revision=epoch,location=page.url.href;return()=>{if(disposed||epoch!==revision||page.url.href!==location)throw Error('The selected identity or page changed. A sent confirmation may have succeeded; never resend an uncertain operation.');};}
 async function load(){if(pending||!mode)return;const check=guard();pending=true;error='';try{let access:ClosureAccess;
  if(mode==='signed'){const bound=await boundAccountAccess(check);check();access={accountId:bound.accountId,origin:window.location.origin,check:bound.check,request:(action,body)=>bound.request(action==='read'?'/nostr/user/'+bound.accountId+'/closure?expectedAccountId='+bound.accountId:action==='prepare'?'/user/'+bound.accountId:'/user/delete/'+(body as {confirmationId:string}).confirmationId,action==='read'?'GET':action==='prepare'?'DELETE':'POST',body)};}
  else{const base=nativeClosureBase(PUBLIC_API_URL,window.location.href),authority=captureNativeAuthority(),account=get(Session)?.identity?.id;if(!account||authority.actor!==account)throw Error('Sign in with your normal native account first.');const assert=()=>{check();if(!sameNativeAuthority(authority))throw Error('The native account session changed.');};access={accountId:account,origin:window.location.origin,check:assert,request:async(action,body)=>{assert();const path='/account/closure/'+(action==='read'?encodeURIComponent(account):action);if(action==='read'){const h=new HTTP(base+path);h.setWithCredentials(true);const r=await h.GET();assert();if(!r)throw Error('No account preview response');return {status:r.status,body:r.body};}const r=await authenticatedMutation(path,'POST',body);assert();if(!r)throw Error('The confirmation outcome is unknown');return {status:r.status,body:r.body};}};}
  const selected=createAccountClosure(access,localStorage,browserExclusive),view=await selected.reload();check();client=selected;current=view;prepared=false;phrase='';
 }catch(cause){try{check();error=cause instanceof Error?cause.message:'Account preview unavailable';}catch{}}finally{try{check();pending=false;}catch{}}}
 async function prepare(){if(!client||pending)return;const check=guard();pending=true;error='';try{const result=await client.prepare();check();current=result.view;prepared=result.prepared;phrase='';}catch(cause){try{check();prepared=false;error=cause instanceof Error?cause.message:'Confirmation unavailable';}catch{}}finally{try{check();pending=false;}catch{}}}
 async function confirm(event:Event){event.preventDefault();if(!client||pending||!prepared||phrase!==acknowledgement)return;const check=guard();pending=true;error='';try{await client.confirm(phrase);check();prepared=false;phrase='';notice='This instance account is disabled. Contributions remain attributed. Contact the instance administrator for audited recovery. Old sessions, API clients and signer bindings will not be restored.';current=undefined;client=undefined;}catch(cause){try{check();prepared=false;phrase='';current=undefined;client=undefined;error=(cause instanceof Error?cause.message:'Confirmation not received')+' If the result is uncertain, contact the instance administrator. Do not retry or clear browser protection.';}catch{}}finally{try{check();pending=false;}catch{}}}
</script>
<section class="space-y-4 max-w-2xl p-4">
 <h1 class="text-2xl font-semibold">Disable this instance account</h1>
 <p>This is reversible disabling, not deletion. All instance sessions, API clients and bound signer access are revoked. Your public biography and contact links are hidden. Contributions and attribution remain. Nothing is automatically purged.</p>
 <p>Recovery is by the existing instance administrator only, not self-service. Your separate original signing identity, keys and encrypted Library are not deleted or revoked.</p>
 <label class="block" for="closure-authority">Choose the authority for this account</label>
 <select id="closure-authority" class="border rounded p-2" bind:value={mode} disabled={pending}><option value="">Select explicitly…</option><option value="native">Normal native browser account</option><option value="signed">Selected signing identity’s bound instance account</option></select>
 {#if mode==='native'}<p>Native account: {$Session?.identity?.username??'not signed in'}. Preparation and confirmation require a normal login within the last 10 minutes. <a class="underline" href="/auth/login?redirect=%2Faccount%2Fclosure">Sign in again</a></p>{/if}
 {#if mode==='signed'}<p>Selected public signing key: {$signerIdentity.owner??'not connected'}. Every step requires a fresh signature for this instance. <a class="underline" href="/settings/identity">Choose signing identity</a></p>{/if}
 <button class="border rounded px-3 py-2" disabled={!mode||pending} onclick={load}>Load selected account and blockers</button>
 {#if current}<div class="space-y-2"><strong>Selected account: {current.username}</strong><p>Account ID: {current.accountId}. Revision: {current.version}.</p>
  {#if current.blockers.length}<p>Resolve these items first. No work will be silently cancelled or replayed.</p><ul>{#each current.blockers as blocker}<li><a class="underline" href={blocker.href}>{blocker.id}</a>: {blocker.action}</li>{/each}</ul>
  {:else if !prepared}<button class="border rounded px-3 py-2" disabled={pending} onclick={prepare}>Prepare one-use disable confirmation</button>{/if}
 </div>{/if}
 {#if prepared&&current}<form class="space-y-2" onsubmit={confirm}><p>Disable {current.username} ({current.accountId}) at revision {current.version}. This confirmation expires in 10 minutes and cannot be used without this same authority.</p><label for="closure-phrase">Type {acknowledgement}</label><input id="closure-phrase" class="border rounded p-2 block" bind:value={phrase} autocomplete="off" disabled={pending}/><button class="border rounded px-3 py-2 text-destructive" disabled={pending||phrase!==acknowledgement}>Confirm reversible account disable</button></form>{/if}
 {#if pending}<p role="status">Checking the selected account…</p>{/if}{#if error}<p role="alert" class="text-destructive">{error}</p>{/if}{#if notice}<p role="status">{notice}</p>{/if}
</section>
