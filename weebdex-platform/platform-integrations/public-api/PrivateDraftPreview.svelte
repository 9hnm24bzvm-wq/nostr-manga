<script lang="ts">
 import {untrack} from 'svelte';
 import {page} from '$app/state';
 import {Session} from '$lib/stores';
 import {Locale} from '$lib';
 import {PUBLIC_SITE_NAME} from '$env/static/public';
 import {createSessionDraftPreview} from './session-draft-preview';
 import AccountActionGate from './AccountActionGate.svelte';
 import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
 import {createSessionCoverController,type SignedCoverController} from './session-cover-client';
 import {createSessionRelationController,type RelationController} from './session-relation-client';
 import {reviewTimeline} from './draft-feedback';
 import {languageLabel} from './language';
 import {localized} from './validation';
 import type {MangaView,Resource} from './types';
 let {target}:{target:string}=$props();
 let title=$state<MangaView>(),busy=$state(false),failure=$state(''),coverUrl=$state(''),coverNotice=$state(''),relationNotice=$state(''),relations=$state<Resource[]>([]),relationTotal=$state(0),relationsReady=$state(false),retry=$state(0);
 let generation=0,disposeCurrent=()=>{};
 let issue=$state<NativeAccountIssue>();
 let admittedSession=$state.raw<typeof $Session>(),admittedRoute=$state('');
 const current=$derived(!!title&&title.id===target&&admittedSession===$Session&&admittedRoute===page.url.href&&page.url.searchParams.get('draft')==='true');
 const feedback=$derived(title?reviewTimeline(title.public_api.resource.attributes.reviewTimeline):{events:[],hasEarlier:false});
 function clear(){issue=undefined;disposeCurrent();disposeCurrent=()=>{};if(coverUrl)URL.revokeObjectURL(coverUrl);coverUrl='';title=undefined;relations=[];relationTotal=0;relationsReady=false;coverNotice='';relationNotice='';failure='';busy=false;admittedSession=undefined;admittedRoute='';}
 $effect(()=>{
  const session=$Session,href=page.url.href,id=target,preview=page.url.searchParams.get('draft')==='true';
  retry;
  untrack(()=>{
   generation++;clear();const token=generation;
   if(!preview)return;
   if(!session?.identity?.id){issue='signin';failure='Sign in to view this private draft.';return;}
   let reader:Awaited<ReturnType<typeof createSessionDraftPreview>>|undefined,covers:SignedCoverController|undefined,edges:RelationController|undefined,disposed=false;
   const check=()=>{if(disposed||token!==generation||$Session!==session||window.location.href!==href||target!==id)throw Error('The private preview account or route changed.');};
   disposeCurrent=()=>{disposed=true;reader?.dispose();covers?.dispose();edges?.dispose();};
   async function readCover(value:MangaView){
    try{
     const next=await createSessionCoverController(session!.identity.id,check,()=>value,reader!.account.id);try{check();}catch(error){next.dispose();throw error;}covers=next;
     const saved=await covers.read();check();
     const chosen=saved.data.find(row=>row.id===value.relationships.cover?.id)??saved.data.find(row=>row.main);
     if(!chosen?.original){coverNotice=saved.total?'Open the editor to inspect the other saved covers.':'No saved cover.';return;}
     const blob=await covers.preview(chosen);check();const url=URL.createObjectURL(blob);
     try{check();}catch(error){URL.revokeObjectURL(url);throw error;}coverUrl=url;
    }catch{if(!disposed&&token===generation)coverNotice='The saved cover could not be verified. Reload the preview or open the editor.';}
   }
   async function readRelations(value:MangaView){
    try{
     const next=await createSessionRelationController(session!.identity.id,value.id,reader!.account.id,check);try{check();}catch(error){next.dispose();throw error;}edges=next;
     const result=await edges.read(value.version,value.state);check();if(result.status!==200||!result.body)throw Error('Private relations unavailable');
     relations=result.body;relationsReady=true;relationTotal=edges.page()?.total??result.body.length;
    }catch{if(!disposed&&token===generation)relationNotice='Saved related titles could not be verified. Reload the preview or open the editor.';}
   }
   busy=true;
   void(async()=>{
    try{
     const next=await createSessionDraftPreview(session.identity.id,id,check);try{check();}catch(error){next.dispose();throw error;}reader=next;
     const value=await reader.read();check();title=value;admittedSession=session;admittedRoute=href;busy=false;
     await Promise.all([readCover(value),readRelations(value)]);
    }catch(error){if(!disposed&&token===generation){title=undefined;issue=nativeAccountIssue(error);failure='This private draft is unavailable for the current account. Return to your drafts or reload after checking access.';}}
    finally{if(!disposed&&token===generation)busy=false;}
   })();
  });
  return()=>{generation++;clear();};
 });
 function related(row:Resource){return row.relationships.find(r=>r.type==='manga')!;}
 function relatedName(row:Resource){const value=related(row).attributes?.title;return value&&typeof value==='object'&&!Array.isArray(value)?localized(value):'View related title';}
</script>
<svelte:head><title>{current&&title?`${title.title} — Private draft preview`:'Private draft preview'} - {PUBLIC_SITE_NAME}</title><meta name="robots" content="noindex, nofollow" /></svelte:head>
<section class="space-y-5" aria-label="Private draft preview">
 <AccountActionGate {issue} />
 <nav class="flex flex-wrap gap-4"><a class="underline" href="/drafts/title">My Title Drafts</a>{#if current&&title}<a class="underline" href={`/title/${title.id}/edit?draft=true`}>Edit draft</a>{/if}<button type="button" class="rounded border px-3 py-1" disabled={busy||!$Session?.identity?.id} onclick={()=>retry++}>Reload preview</button></nav>
 {#if busy}<p role="status">Loading private draft…</p>{/if}
 {#if failure}<p role="alert">{failure}</p>{/if}
 {#if current&&title}
  <header class="space-y-2"><p class="font-medium">Private draft preview</p><h1 class="text-2xl font-bold">{title.title}</h1><p>{title.state==='submitted'?'Awaiting review':title.state==='rejected'?'Changes requested':'Draft'} · {languageLabel(title.language,Locale.Languages)}</p><p class="text-sm text-muted-foreground">This preview shows saved title details. Chapters, library actions and reading progress are not available here.</p></header>
  <div class="flex flex-wrap gap-5">
   <div>{#if coverUrl}<img class="max-h-96 max-w-64 object-contain" src={coverUrl} alt={`Saved cover for ${title.title}`} />{:else if !coverNotice}<p role="status">Loading saved cover…</p>{/if}{#if coverNotice}<p role="status">{coverNotice}</p>{/if}</div>
   <div class="min-w-0 flex-1 space-y-3"><h2 class="font-semibold">Synopsis</h2><p class="whitespace-pre-wrap">{title.description||'No description saved.'}</p>
    {#if Object.values(title.alt_titles).flat().length}<p>Other titles: {Object.values(title.alt_titles).flat().join(' · ')}</p>{/if}
    <p>{title.status}{title.year?` · ${title.year}`:''} · {title.content_rating}</p>
    {#if title.relationships.authors.length}<p>Authors: {title.relationships.authors.map(row=>row.name).join(', ')}</p>{/if}
    {#if title.relationships.artists.length}<p>Artists: {title.relationships.artists.map(row=>row.name).join(', ')}</p>{/if}
    {#if title.relationships.tags.length}<p>Tags: {title.relationships.tags.map(row=>row.name).join(', ')}</p>{/if}
   </div>
  </div>
  <section class="space-y-2"><h2 class="font-semibold">Saved related titles</h2>{#if relationNotice}<p role="status">{relationNotice}</p>{:else if !relationsReady}<p role="status">Loading saved related titles…</p>{:else if relations.length}<ul>{#each relations as row(row.id)}<li><a class="underline" href={`/title/${related(row).id}`}>{relatedName(row)}</a> — {String(row.attributes.relation).replaceAll('_',' ')}</li>{/each}</ul>{:else}<p>No currently visible saved relations.</p>{/if}{#if relationTotal>relations.length}<p>Showing the first {relations.length} of {relationTotal} visible relations. Open the editor to inspect the remaining pages.</p>{/if}</section>
  {#if feedback.events.length}<section class="space-y-2"><h2 class="font-semibold">Review timeline</h2>{#each feedback.events as event}<p>{event.action==='rejected'?'Changes requested':event.action==='submitted'?'Submitted for review':'Published'}{event.reason?`: ${event.reason}`:''}</p>{/each}{#if feedback.hasEarlier}<p>Earlier review events are available in the editor.</p>{/if}</section>{/if}
 {/if}
</section>
