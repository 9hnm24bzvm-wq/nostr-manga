<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {Button} from '$lib/components/ui/button';
 import * as Dialog from '$lib/components/ui/dialog';
 import {signerIdentity} from '../local-library/publication';
 import {loadAccountBlocklists,type AccountBlockKind,type AccountBlockReference} from './blocklist-client';
 let {disabled=false,onOpen=()=>{}}:{disabled?:boolean;onOpen?:(open:boolean)=>void}=$props();
 let open=$state(false),busy=$state(false),error=$state(''),notice=$state(''),query=$state(''),selectedKind=$state<AccountBlockKind>('groups'),results=$state<AccountBlockReference[]>([]);
 let current=$state<ReturnType<Awaited<ReturnType<typeof loadAccountBlocklists>>['view']>>();
 let client:Awaited<ReturnType<typeof loadAccountBlocklists>>|undefined,epoch=0,searchEpoch=0,disposed=false;
 const path=window.location.pathname;
 function guard(){const captured=epoch,owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||!open||captured!==epoch||path!==window.location.pathname||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The account blocklist editor changed. Reopen it.');};}
 function clear(){epoch++;searchEpoch++;client=undefined;current=undefined;results=[];query='';busy=false;error='';notice='';}
 $effect(()=>{const owner=$signerIdentity.owner,generation=$signerIdentity.generation;untrack(()=>{clear();open=false;onOpen(false);});});
 onDestroy(()=>{disposed=true;epoch++;onOpen(false);});
 function setOpen(value:boolean){if(!value){open=false;clear();onOpen(false);return;}if(disabled||!$signerIdentity.owner)return;clear();open=true;onOpen(true);void reload();}
 async function reload(){if(busy||!open)return;if(current?.dirty&&!confirm('Discard these unsaved blocklist selections and read the current account?'))return;const assert=guard();busy=true;error='';notice='';results=[];searchEpoch++;try{const selected=client??await loadAccountBlocklists(assert);assert();const value=await selected.reload();assert();client=selected;current=value;}catch(cause){try{assert();current=client?.view();error=cause instanceof Error?cause.message:'Account blocklists could not be read.';}catch{}}finally{try{assert();busy=false;}catch{}}}
 function searchChanged(){searchEpoch++;results=[];}
 async function search(){if(busy||!client||!current||current.needsRecheck)return;const assert=guard(),request=++searchEpoch,text=query,key=selectedKind;busy=true;error='';results=[];try{const rows=await client.search(key,text);assert();if(request===searchEpoch&&query===text&&selectedKind===key)results=rows;}catch(cause){try{assert();current=client.view();error=cause instanceof Error?cause.message:'Reference search was not confirmed.';}catch{}}finally{try{assert();busy=false;}catch{}}}
 function edit(action:'add'|'remove',key:AccountBlockKind,id:string){if(busy||!client||current?.needsRecheck)return;try{guard()();current=client[action](key,id);notice='';error='';}catch(cause){error=cause instanceof Error?cause.message:'Reference selection failed.';}}
 async function save(){if(busy||!client||!current?.dirty||current.needsRecheck)return;if(!confirm(`Save these hidden group/uploader selections to ${current.username} on this instance?`))return;const assert=guard();busy=true;error='';notice='';try{const value=await client.save();assert();current=value;notice='Account blocklists saved.';}catch(cause){try{assert();current=client.view();error=cause instanceof Error?cause.message:'Account blocklist save was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();busy=false;}catch{}}}
</script>
{#if $signerIdentity.owner}<Button type="button" variant="outline" {disabled} onclick={()=>setOpen(true)}>Manage account blocklists</Button>{/if}
<Dialog.Root {open} onOpenChange={setOpen}>
 <Dialog.Content class="max-h-[85vh] overflow-y-auto sm:max-w-xl">
  <Dialog.Header><Dialog.Title>Account blocklists</Dialog.Title><Dialog.Description>Explicitly edit hidden groups and uploaders on your bound instance account. Device preferences are unchanged.</Dialog.Description></Dialog.Header>
  {#if current}
   <p>Account: {current.username}. Settings version: {current.version}.</p>
   {#if current.unavailable}<p class="text-sm">Unavailable account filter references are retained by the server and cannot be removed through this editor.</p>{/if}
   <fieldset disabled={busy||current.needsRecheck} class="space-y-3">
    {#each ['groups','uploaders'] as key}{@const kind=key as AccountBlockKind}<section class="space-y-2"><h4 class="font-medium">Hidden {kind}</h4><ul class="space-y-1">{#each current[kind] as row (row.id)}<li class="flex items-center justify-between gap-2"><span>{row.name}<span class="block break-all text-xs">{row.id}</span></span><Button type="button" variant="outline" onclick={()=>edit('remove',kind,row.id)}>Remove {row.name}</Button></li>{:else}<li class="text-sm">No visible {kind} selected.</li>{/each}</ul></section>{/each}
    <label class="block">Reference type<select class="block rounded border p-2" bind:value={selectedKind} onchange={searchChanged}><option value="groups">Groups</option><option value="uploaders">Uploaders</option></select></label>
    <label class="block">Search name<input class="block w-full rounded border p-2" bind:value={query} oninput={searchChanged} maxlength="100" /></label>
    <Button type="button" variant="outline" disabled={query.trim().length<2} onclick={search}>Search account references</Button>
    <ul>{#each results as row (row.id)}<li class="flex items-center justify-between gap-2"><span>{row.name}<span class="block break-all text-xs">{row.id}</span></span><Button type="button" variant="outline" disabled={current[selectedKind].some(value=>value.id===row.id)} onclick={()=>edit('add',selectedKind,row.id)}>Hide {row.name}</Button></li>{/each}</ul>
    <Button type="button" disabled={!current.dirty} onclick={save}>Review and save account blocklists</Button>
   </fieldset>
  {/if}
  <Button type="button" variant="outline" disabled={busy} onclick={reload}>Reload current account blocklists</Button>
  {#if busy}<p role="status">Checking account blocklists…</p>{/if}
  {#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
 </Dialog.Content>
</Dialog.Root>
