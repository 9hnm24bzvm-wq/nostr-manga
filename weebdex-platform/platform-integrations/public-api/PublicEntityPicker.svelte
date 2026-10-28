<script lang="ts">
 import {onDestroy} from 'svelte';
 import type {EditorChoice} from './editor-fields';
 let {label,selected=$bindable<EditorChoice[]>([]),search,disabled=false,single=false}:{label:string;selected?:EditorChoice[];search:(query:string,signal:AbortSignal)=>Promise<EditorChoice[]>;disabled?:boolean;single?:boolean}=$props();
 let query=$state(''),results=$state<EditorChoice[]>([]),busy=$state(false),notice=$state('');let epoch=0,disposed=false,abort:AbortController|undefined;
 function cancel(){epoch++;abort?.abort();abort=undefined;busy=false;results=[];notice='';}
 onDestroy(()=>{disposed=true;cancel();});
 $effect(()=>{if(disabled)cancel();});
 async function find(){if(disabled)return;cancel();const generation=epoch,controller=new AbortController();abort=controller;busy=true;try{const rows=await search(query,controller.signal);if(disposed||generation!==epoch||disabled)return;results=rows;notice=rows.length?'Choose a result below. Try a more specific name if you do not see the one you need.':'No matching results.';}catch(error){if(!disposed&&generation===epoch)notice=error instanceof Error?error.message:'Search failed.';}finally{if(!disposed&&generation===epoch)busy=false;}}
 function select(row:EditorChoice){if(disabled||selected.some(r=>r.id===row.id))return;selected=single?[row]:[...selected,row];results=results.filter(r=>r.id!==row.id);}
</script>
<fieldset class="space-y-2 rounded border p-3" {disabled}>
 <legend class="font-medium">{label}</legend>
 <div class="flex flex-wrap gap-2">{#each selected as row(row.id)}<button type="button" class="rounded border px-2 py-1" title={row.id} aria-label={`Remove ${row.name} from ${label}`} onclick={()=>{selected=selected.filter(r=>r.id!==row.id);}}>{row.name} ×</button>{/each}</div>
 <div class="flex gap-2"><input aria-label={`Search ${label}`} class="min-w-0 flex-1 rounded border p-2" bind:value={query} maxlength="200" oninput={cancel} onkeydown={event=>{if(event.key==='Enter'){event.preventDefault();void find();}}} /><button type="button" class="rounded border px-3 py-1" disabled={busy} onclick={find}>Search</button><button type="button" class="rounded border px-3 py-1" onclick={cancel}>Cancel search</button></div>
 {#if notice}<p role="status">{notice}</p>{/if}
 <div class="flex flex-wrap gap-2">{#each results.filter(r=>!selected.some(s=>s.id===r.id)) as row(row.id)}<button type="button" class="rounded border px-2 py-1" title={row.id} onclick={()=>select(row)}>Add {row.name}</button>{/each}</div>
</fieldset>
