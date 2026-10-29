<script lang="ts">
 import {onMount,onDestroy,untrack,tick} from 'svelte';
 import {createSessionRelationController,relationKinds,relationLimit,type RelationController,type RelationAction,type RelationPage} from './session-relation-client';
 import type {MangaView,Resource} from './types';
 import {localized} from './validation';
 import PublicEntityPicker from './PublicEntityPicker.svelte';
 import type {EditorChoice} from './editor-fields';
 let {nativeAccount,title,accountId,disabled=false,assertCurrent,onState,onChanged}:{nativeAccount:string;title:MangaView;accountId:string;disabled?:boolean;assertCurrent:()=>void;onState:(state:{busy:boolean;pending:boolean})=>void;onChanged:()=>Promise<void>}=$props();
 const captured=untrack(()=>({id:title.id,accountId}));let disposed=false,owner=$state<RelationController>();
 let rows=$state<Resource[]>([]),ready=$state(false),busy=$state(false),pending=$state(false),notice=$state(''),targets=$state<EditorChoice[]>([]),kind=$state<string>('sequel'),edits=$state<Record<string,string>>({}),lastAttempt=$state(''),page=$state<Readonly<RelationPage>>(),previousOffsets=$state<number[]>([]);
 const staff=$derived(owner?.account.roles.some(r=>['moderator','staff','sysop'].includes(r)));
 const permitted=$derived((!title.locked||staff)&&(title.state==='published'?owner?.account.permissions.includes('manga.relation.edit'):owner?.account.permissions.includes('manga.create')||staff&&owner?.account.permissions.includes('manga.relation.edit')));
 const active=()=>{if(disposed||title.id!==captured.id||accountId!==captured.accountId)throw Error('The relation owner changed. Reopen this title.');assertCurrent();};
 function message(error:unknown){return error instanceof Error?error.message:'Relation outcome was not confirmed.';}
 function errorResponse(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Relation request refused (HTTP ${response.status})`;}
 function notify(){onState({busy,pending});}
 onDestroy(()=>{disposed=true;owner?.dispose();});
 onMount(()=>{void(async()=>{busy=true;notify();try{owner=await createSessionRelationController(nativeAccount,captured.id,captured.accountId,active);active();pending=owner.pending();}catch(error){if(!disposed)notice=message(error);}finally{if(!disposed){busy=false;notify();}}})();});
 $effect(()=>{const marker=title.version+':'+title.state;if(owner&&!busy&&lastAttempt!==marker)void load();});
 async function load(offset?:number){if(!owner||busy)return;busy=true;ready=false;lastAttempt=title.version+':'+title.state;notify();try{const response=await owner.read(title.version,title.state,offset);active();if(response.status!==200||!response.body)throw Error(errorResponse(response));rows=response.body;page=owner.page();if(offset===undefined)previousOffsets=[];edits=Object.fromEntries(rows.map(r=>[r.id,String(r.attributes.relation)]));ready=true;}catch(error){if(!disposed)notice=message(error);}finally{if(!disposed){busy=false;pending=owner.pending();notify();}}}
 async function nextPage(){if(page?.nextOffset===null||!page)return;const prior=page.offset;await load(page.nextOffset);if(ready)previousOffsets=[...previousOffsets,prior];}
 async function previousPage(){const offset=previousOffsets.at(-1);if(offset===undefined)return;await load(offset);if(ready)previousOffsets=previousOffsets.slice(0,-1);}
 async function command(action:RelationAction,row?:Resource){if(!owner||!ready||busy||pending||disabled||!permitted)return;if(action==='delete'&&!confirm('Remove this relation from this site? The original source is unchanged.'))return;
  if(action==='create'&&targets.length!==1)return;
  const input=action==='create'?{targetManga:targets[0].id,relation:kind,version:title.version}:action==='update'?{relationId:row?.id,relation:row&&edits[row.id],version:title.version}:{relationId:row?.id,version:title.version};busy=true;notice='';notify();
  try{const response=await owner.mutate(action,input);active();if(response.status>=400)throw Error(errorResponse(response));notice=action==='delete'?'Relation removed from this site.':'Relation saved on this site.';ready=false;await onChanged();active();await tick();}
  catch(error){if(!disposed)notice=message(error);}finally{if(!disposed){busy=false;pending=owner.pending();notify();}}
 }
 function targetId(row:Resource){return row.relationships.find(r=>r.type==='manga')!.id;}
 function targetName(row:Resource){const target=row.relationships.find(r=>r.type==='manga')!,name=target.attributes?.title;return name&&typeof name==='object'&&!Array.isArray(name)?localized(name):targets.find(choice=>choice.id===target.id)?.name??'View related title';}
 async function search(query:string,signal:AbortSignal){active();if(!owner)throw Error('The title editor is not ready.');return owner.search(query,signal);}
</script>
<section class="space-y-3 rounded border p-3">
 <h4 class="font-medium">Title relations</h4>
 <p>Related titles are shown on this site. You can add titles currently available in the catalogue.</p>
 {#if notice}<p role="status">{notice}</p>{/if}
 {#if pending}<p role="alert">A relation command has an unconfirmed outcome. It will not be replayed, and a read cannot clear that uncertainty.</p>{/if}
 <button class="rounded border px-3 py-1" type="button" disabled={!owner||busy} onclick={()=>load()}>Reload relations</button>
 {#if ready&&page}<p>Showing {page.total===0?0:page.offset+1}–{page.offset+rows.length} of {page.total} currently visible relations. Hidden relations still count toward the limit.</p><div class="flex gap-2"><button type="button" class="rounded border px-3 py-1" disabled={busy||previousOffsets.length===0} onclick={previousPage}>Previous page</button><button type="button" class="rounded border px-3 py-1" disabled={busy||page.nextOffset===null} onclick={nextPage}>Next page</button></div>{/if}
 {#if ready&&rows.length===0}<p>No currently visible relations.</p>{/if}
 {#if ready&&page&&page.persistedTotal>=relationLimit}<p>The 4096-relation limit is reached. Remove an existing relation before adding another.</p>{/if}
 {#each rows as row(row.id)}<div class="flex flex-wrap items-center gap-2"><a class="underline" href={`/title/${targetId(row)}`}>{targetName(row)}</a><select aria-label={`Relation to ${targetName(row)}`} bind:value={edits[row.id]} disabled={busy||pending||disabled||!permitted}>{#each relationKinds as option}<option value={option}>{option.replaceAll('_',' ')}</option>{/each}</select><button type="button" class="rounded border px-3 py-1" disabled={!ready||busy||pending||disabled||!permitted||edits[row.id]===row.attributes.relation} onclick={()=>command('update',row)}>Save relation</button><button type="button" class="rounded border px-3 py-1" disabled={!ready||busy||pending||disabled||!permitted} onclick={()=>command('delete',row)}>Remove relation</button></div>{/each}
 <form class="flex flex-wrap gap-2" onsubmit={event=>{event.preventDefault();void command('create');}}><fieldset class="contents" disabled={!ready||busy||pending||disabled||!permitted||!page||page.persistedTotal>=relationLimit}><PublicEntityPicker label="Related title" bind:selected={targets} {search} single disabled={!ready||busy||pending||disabled||!permitted||!page||page.persistedTotal>=relationLimit} /><label>Relation<select class="block rounded border p-2" bind:value={kind}>{#each relationKinds as option}<option value={option}>{option.replaceAll('_',' ')}</option>{/each}</select></label><button class="rounded border px-3 py-1" type="submit" disabled={targets.length!==1}>Add relation</button></fieldset></form>
</section>
