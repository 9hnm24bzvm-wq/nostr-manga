<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {beforeNavigate} from '$app/navigation';
 import {Session} from '$lib/stores';
 import {createStaffClient,can,object,type StaffClient} from './staff-client';
 let {targetId,onSaved=()=>{}}:{targetId:string;onSaved?:(value:any)=>void}=$props();
 const fields=['description','discord','twitter','website'] as const;
 let client:StaffClient|undefined,epoch=0,closed=false;
 let current=$state<any>(),draft=$state<Record<string,string>>({}),pending=$state(false),error=$state(''),notice=$state(''),needsReload=$state(false),review=$state(false);
 const changed=$derived(!!current&&fields.some(field=>(current[field]??'')!==(draft[field]??'')));
 const path=$derived('/staff/users/'+encodeURIComponent(targetId)+'/profile');
 function guard(){const n=epoch,id=targetId,session=$Session;return()=>{if(closed||n!==epoch||id!==targetId||session!==$Session)throw Error('This profile or account changed. Open it again.');};}
 function reset(){draft=Object.fromEntries(fields.map(field=>[field,current?.[field]??'']));review=false;}
 function observe(value:any,write=false){if(value.scope!=='native-staff-profile'||value.userId!==targetId||value.changed!==write)throw Error('The profile receipt did not match this editor. Reload before continuing.');const attributes=object(value.attributes);if(!Number.isSafeInteger(attributes.version)||attributes.version<1||typeof attributes.username!=='string'||fields.some(field=>attributes[field]!==undefined&&typeof attributes[field]!=='string'))throw Error('The profile could not be checked.');return attributes;}
 $effect(()=>{const target=targetId,session=$Session;untrack(()=>{epoch++;client?.dispose();client=undefined;current=undefined;draft={};review=false;error='';notice='';pending=false;needsReload=false;if(target&&session?.identity?.id)void load();});});
 onDestroy(()=>{closed=true;epoch++;client?.dispose();});
 beforeNavigate(navigation=>{if(changed&&!pending&&!window.confirm('Discard your unsaved profile changes?'))navigation.cancel();});
 function unloading(event:BeforeUnloadEvent){if(changed){event.preventDefault();event.returnValue='';}}
 async function load(){if(pending||!$Session?.identity?.id)return;const assert=guard();pending=true;error='';review=false;try{const selected=client??await createStaffClient($Session.identity.id,assert);assert();client=selected;if(!can(selected.authority,'user.update',true))throw Error('Your current account cannot edit another person’s profile.');current=observe(await selected.read(path));assert();needsReload=false;reset();}catch(cause){try{assert();error=cause instanceof Error?cause.message:'The profile could not be loaded.';current=undefined;}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function save(){if(!client||!review||pending||!current||needsReload)return;const assert=guard(),selected=client,before=current,patch:Record<string,unknown>={version:before.version};for(const field of fields)if((draft[field]??'')!==(before[field]??''))patch[field]=draft[field]??'';pending=true;error='';notice='';try{const result=await selected.saveProfile(path,{expectedUserId:targetId,patch});assert();const next=observe(result,true);if(next.version!==before.version+1)throw Error('The profile receipt has an unexpected version. Reload it before continuing.');current=next;reset();needsReload=false;notice='Profile saved. Only the reviewed biography and contact fields changed.';onSaved(result);}catch(cause){try{assert();needsReload=true;review=false;error=cause instanceof Error?cause.message:'The result is not confirmed. Reload the profile before another edit.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
<svelte:window onbeforeunload={unloading}/>
<section class="space-y-4" aria-label="Staff profile editor">
 <h2 class="text-xl font-semibold">Edit public profile as staff</h2>
 <p>This changes the selected person’s biography and contact links on this instance only. It does not change their account roles, email, credentials or public signing identity.</p>
 {#if !$Session?.identity?.id}<p>Sign in to continue.</p><a href="/auth/login" class="underline">Sign in</a>{/if}
 {#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
 {#if notice}<p role="status">{notice}</p>{/if}
 {#if current}
  <p class="font-medium">Editing {current.username}</p>
  <form class="space-y-4" onsubmit={event=>{event.preventDefault();if(changed&&!needsReload&&!pending)review=true;}}>
   <label class="block">Biography<textarea class="block w-full rounded border p-2" rows="6" maxlength="20000" bind:value={draft.description} disabled={pending||needsReload||review}></textarea></label>
   {#each fields.filter(field=>field!=='description') as field}<label class="block">{field==='twitter'?'Twitter / X':field==='discord'?'Discord':'Website'}<input class="block w-full rounded border p-2" maxlength="2048" bind:value={draft[field]} disabled={pending||needsReload||review}/></label>{/each}
   {#if review}
    <section class="rounded border p-4 space-y-3" aria-label="Review public profile changes"><h3 class="font-semibold">Review changes for {current.username}</h3><ul>{#each fields.filter(field=>(current[field]??'')!==(draft[field]??'')) as field}<li><strong>{field}</strong>: {draft[field]?'Replace the current value':'Remove this value'}</li>{/each}</ul><p>Unchanged fields will be preserved.</p><button type="button" class="rounded border px-3 py-2" disabled={pending} onclick={()=>review=false}>Cancel</button> <button type="button" class="rounded border px-3 py-2" disabled={pending} onclick={save}>Confirm profile changes</button></section>
   {:else}<button type="submit" class="rounded border px-3 py-2" disabled={pending||needsReload||!changed}>Review changes</button> <button type="button" class="rounded border px-3 py-2" disabled={pending||!changed} onclick={reset}>Revert unsaved changes</button>{/if}
  </form>
 {/if}
 {#if pending}<p role="status">Checking profile…</p>{/if}
 {#if !current||needsReload}<button class="rounded border px-3 py-2" disabled={pending||!$Session?.identity?.id} onclick={load}>Reload profile</button>{/if}
</section>
