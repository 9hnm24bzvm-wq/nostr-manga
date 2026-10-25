<script lang="ts">
 import { untrack } from 'svelte';
 import { Button } from '$lib/components/ui/button';
 import * as Combobox from '$lib/components/ui/combobox';
 import { publicCatalogueRead } from './site-client';
 import type { TitleGroupOptions } from './title-group-options';
 let {mangaId,value=$bindable(),blacklisted=false}:{mangaId:string;value?:string[];blacklisted?:boolean}=$props();
 let options=$state<{id:string;name:string}[]>([]),names=$state<Record<string,string>>({});
 let total=$state<number|undefined>(),next=$state<string|null>(null),after=$state<string|undefined>();
 let busy=$state(false),error=$state(''),selectedError=$state('');
 let generation=0,pageEpoch=0,selectionEpoch=0,request:AbortController|undefined,selectionRequest:AbortController|undefined;
 async function load(cursor?:string){
  const owner=mangaId,current=generation,epoch=++pageEpoch;
  request?.abort();const controller=new AbortController();request=controller;busy=true;error='';
  const previousTotal=total;after=cursor;
  try{
   const query=new URLSearchParams({limit:'100'});if(cursor)query.set('after',cursor);
   const response=await publicCatalogueRead<TitleGroupOptions>('manga.availableGroups',{id:owner,query,signal:controller.signal});
   if(current!==generation||epoch!==pageEpoch||controller.signal.aborted)return;
   if(response.status!==200||!response.body)throw Error('Group choices unavailable (HTTP '+response.status+').');
   if(cursor&&previousTotal!==undefined&&previousTotal!==response.body.total)throw Error('Group choices changed. Start again from the first groups.');
   options=response.body.data;total=response.body.total;next=response.body.next;
   names=Object.fromEntries([...Object.entries(names).filter(([id])=>(value??[]).slice(0,100).includes(id)),...options.map(row=>[row.id,row.name])]);
  }catch(cause){if(current===generation&&epoch===pageEpoch&&!controller.signal.aborted){options=[];next=null;error=cause instanceof Error?cause.message:'Group choices unavailable.';}}
  finally{if(current===generation&&epoch===pageEpoch)busy=false;}
 }
 async function hydrate(selected:string[]){
  const owner=mangaId,current=generation,epoch=++selectionEpoch;
  selectionRequest?.abort();const controller=new AbortController();selectionRequest=controller;selectedError='';
  names=Object.fromEntries(Object.entries(names).filter(([id])=>selected.slice(0,100).includes(id)||options.some(row=>row.id===id)));
  if(selected.length>100){selectedError='At most 100 groups can be selected. Remove a group before applying.';return;}
  const missing=selected.filter(id=>!names[id]);if(!missing.length)return;
  try{
   const query=new URLSearchParams({limit:'100'});for(const id of missing)query.append('ids[]',id);
   const response=await publicCatalogueRead<TitleGroupOptions>('manga.availableGroups',{id:owner,query,signal:controller.signal});
   if(current!==generation||epoch!==selectionEpoch||controller.signal.aborted)return;
   if(response.status!==200||!response.body)throw Error('Selected group names are unavailable.');
   const actual=Object.fromEntries(response.body.data.filter(row=>(value??[]).includes(row.id)).map(row=>[row.id,row.name]));
   names={...names,...actual};if(missing.some(id=>!actual[id]))selectedError='Some selected groups are no longer visible for this title.';
  }catch(cause){if(current===generation&&epoch===selectionEpoch&&!controller.signal.aborted)selectedError=cause instanceof Error?cause.message:'Selected group names are unavailable.';}
 }
 $effect(()=>{const owner=mangaId;untrack(()=>{generation++;options=[];names={};total=undefined;next=null;after=undefined;selectedError='';void load();});return()=>{generation++;request?.abort();selectionRequest?.abort();};});
 $effect(()=>{const selected=[...(value??[])],owner=mangaId;untrack(()=>{void hydrate(selected);});});
</script>
<div class="min-w-38 flex-auto space-y-1">
 <Combobox.Root type="multiple" bind:value>
  <Combobox.Trigger class="w-full border-none">
   <span class="truncate">{#if value?.length}{value.map(id=>names[id]??('Selected group '+id)).join(', ')}{:else if blacklisted}[Forced] Applied group blacklist{:else}Any Group{/if}</span>
  </Combobox.Trigger>
  <Combobox.Content>
   {#each options as group (group.id)}<Combobox.Item value={group.id}>{group.name}</Combobox.Item>{/each}
  </Combobox.Content>
 </Combobox.Root>
 <div class="flex flex-wrap items-center gap-1 text-xs">
  {#if busy}<span role="status">Loading groups…</span>{:else if total!==undefined&&!error}<span>{options.length} choices on this page; {total} visible groups across all chapters and languages.</span>{/if}
  {#if after}<Button type="button" variant="outline" size="sm" disabled={busy} onclick={()=>load()}>First groups</Button>{/if}
  {#if next}<Button type="button" variant="outline" size="sm" disabled={busy} onclick={()=>load(next??undefined)}>Next groups</Button>{/if}
  {#if error}<span role="alert">{error} Title and chapters remain available.</span><Button type="button" variant="outline" size="sm" disabled={busy} onclick={()=>load(after)}>Retry groups</Button>{/if}
  {#if selectedError}<span role="status">{selectedError}</span><Button type="button" variant="outline" size="sm" onclick={()=>hydrate([...(value??[])])}>Retry selected names</Button>{/if}
 </div>
</div>
