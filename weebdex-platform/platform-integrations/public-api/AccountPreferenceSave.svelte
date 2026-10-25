<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {Button} from '$lib/components/ui/button';
 import * as Dialog from '$lib/components/ui/dialog';
 import {signerIdentity} from '../local-library/publication';
 import {loadAccountSettings,changedAccountPreferences,accountPreferenceLabels,type AccountSettingsView} from './settings-client';
 import AccountBlocklistEditor from './AccountBlocklistEditor.svelte';
 const {draft,baseline,disabled=false}:{draft:UserPreferences;baseline:UserPreferences;disabled?:boolean}=$props();
 let open=$state(false),pending=$state(false),error=$state(''),notice=$state(''),current=$state<AccountSettingsView>(),changes=$state<Record<string,unknown>>({});
 let client:Awaited<ReturnType<typeof loadAccountSettings>>|undefined,epoch=0,disposed=false;
 let blocklistsOpen=$state(false);
 function guard(){const revision=epoch,form=JSON.stringify([draft,baseline]),owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||!open||epoch!==revision||form!==JSON.stringify([draft,baseline])||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The preference form or account changed. Reopen account saving.');};}
 function clear(){epoch++;client=undefined;current=undefined;changes={};pending=false;error='';notice='';}
 $effect(()=>{const form=JSON.stringify([draft,baseline]),owner=$signerIdentity.owner,generation=$signerIdentity.generation;untrack(()=>{clear();open=false;});});
 onDestroy(()=>{disposed=true;epoch++;});
 async function load(){if(pending||!$signerIdentity.owner)return;const assert=guard();pending=true;error='';notice='';client=undefined;current=undefined;try{const patch=changedAccountPreferences(draft,baseline);if(!Object.keys(patch).length)throw Error('There are no supported preference changes to save.');const selected=await loadAccountSettings(assert);assert();const result=await selected.reload();assert();changes=patch;client=selected;current=result;}catch(cause){try{assert();error=cause instanceof Error?cause.message:'Account preferences could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 function setOpen(value:boolean){if(!value){open=false;clear();return;}open=true;if($signerIdentity.owner)void load();}
 async function save(){if(pending||!client||!current||current.needsRecheck)return;const assert=guard(),selected=client;pending=true;error='';notice='';try{const result=await selected.save(changes);assert();current=result;notice='Account preferences saved.';}catch(cause){try{assert();current=selected.view();error=cause instanceof Error?cause.message:'The account save was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
{#if $signerIdentity.owner}<Button type="button" variant="outline" disabled={disabled||blocklistsOpen} onclick={()=>setOpen(true)}>Save to account</Button>{/if}
<Dialog.Root {open} onOpenChange={setOpen}>
 <Dialog.Content class="sm:max-w-md">
  <Dialog.Header><Dialog.Title>Save preferences to account</Dialog.Title><Dialog.Description>Save the changes in this form to your account on this instance. The form’s Save button applies preferences here as usual.</Dialog.Description></Dialog.Header>
  {#if current}
   <p class="text-sm">Save these changes to {current.username}:</p>
   <ul class="list-inside list-disc text-sm">{#each Object.keys(changes) as key}<li>{accountPreferenceLabels[key]}</li>{/each}</ul>
   {#if current.unavailable}<p class="text-sm text-muted-foreground">Unavailable filter choices already on the account will be kept.</p>{/if}
   <div class="flex flex-wrap gap-2"><Button disabled={pending||current.needsRecheck||!!notice} onclick={save}>Save account changes</Button>{#if current.needsRecheck}<Button variant="outline" disabled={pending} onclick={load}>Reload account preferences</Button>{/if}</div>
  {:else if !pending}<Button variant="outline" onclick={load}>Load account preferences</Button>{/if}
  {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking account preferences…</p>{/if}
  {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
  {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
 </Dialog.Content>
</Dialog.Root>
<AccountBlocklistEditor disabled={pending||open} onOpen={value=>{blocklistsOpen=value;}} />
