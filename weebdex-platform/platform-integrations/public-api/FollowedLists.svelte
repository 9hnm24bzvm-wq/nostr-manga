<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {PUBLIC_MANGADEX_CATALOGUE,PUBLIC_SITE_NAME} from '$env/static/public';
 import {Button} from '$lib/components/ui/button';
 import FollowingNavigation from './FollowingNavigation.svelte';
 import {signerIdentity} from '../local-library/publication';
 import {loadFollowedLists} from './list-follow-client';
 import type{PublicListView}from './public-lists';
 let reader:Awaited<ReturnType<typeof loadFollowedLists>>|undefined,epoch=0,disposed=false;
 let rows=$state<PublicListView[]>([]),pending=$state(false),error=$state(''),username=$state(''),offset=$state(0),total=$state(0),unavailable=$state(0);
 function clear(){epoch++;reader=undefined;rows=[];pending=false;error='';username='';offset=0;total=0;unavailable=0;}
 function guard(){const revision=epoch,owner=$signerIdentity.owner,generation=$signerIdentity.generation;return()=>{if(disposed||epoch!==revision||owner!==$signerIdentity.owner||generation!==$signerIdentity.generation)throw Error('The following account changed');};}
 async function load(start=0){if(pending||!$signerIdentity.owner)return;const assert=guard();pending=true;error='';rows=[];username='';unavailable=0;total=0;try{if(PUBLIC_MANGADEX_CATALOGUE!=='1')throw Error('Public-list following is unavailable in this catalogue.');const selected=reader??await loadFollowedLists(assert);assert();const result=await selected.page(start);assert();reader=selected;username=selected.username;rows=result.data;offset=result.offset;total=result.total;unavailable=result.unavailable;}catch(cause){try{assert();error=cause instanceof Error?cause.message:'Followed lists could not be loaded.';reader=undefined;}catch{}}finally{try{assert();pending=false;}catch{}}}
 $effect(()=>{const owner=$signerIdentity.owner,generation=$signerIdentity.generation;untrack(()=>{clear();if(owner)void load();});});
 onDestroy(()=>{disposed=true;epoch++;});
</script>
<svelte:head><title>Followed lists - {PUBLIC_SITE_NAME}</title></svelte:head>
<section class="space-y-5">
 <header class="space-y-3"><h1 class="text-xl font-semibold">Followed lists</h1><nav class="flex flex-wrap items-center gap-3"><a class="text-sm underline" href="/library">My library</a><Button variant="outline" disabled={pending||!$signerIdentity.owner} onclick={()=>{reader=undefined;void load();}}>Refresh</Button></nav></header>
 <FollowingNavigation active="list"/>
 {#if !$signerIdentity.owner}<p class="text-muted-foreground">Connect your public identity to view account-followed lists on this instance.</p><a class="underline" href="/settings/identity">Manage public identity</a>{/if}
 {#if username}<p class="text-sm text-muted-foreground">Following as {username} on this instance.</p>{/if}
 {#if pending}<p role="status" class="text-muted-foreground">Loading followed lists…</p>{/if}
 {#if error}<p role="alert" class="text-destructive">{error}</p>{:else if username&&!pending&&!rows.length}<p class="text-muted-foreground">You have no available followed lists.</p>{/if}
 {#if unavailable}<p class="text-sm text-muted-foreground">{unavailable.toLocaleString()} saved {unavailable===1?'list is':'lists are'} currently unavailable.</p>{/if}
 <div class="grid gap-3 md:grid-cols-2">{#each rows as list (list.id)}<a class="rounded-md border bg-card p-4 hover:border-accent" href="/list/{list.id}"><h2 class="font-medium">{list.name}</h2><p class="text-sm text-muted-foreground">{list.members.length.toLocaleString()} available titles</p></a>{/each}</div>
 {#if total>20}<nav class="flex gap-2" aria-label="Followed list pages"><Button variant="outline" disabled={pending||offset===0} onclick={()=>load(Math.max(0,offset-20))}>Previous</Button><Button variant="outline" disabled={pending||offset+20>=total||offset+20>10000} onclick={()=>load(offset+20)}>Next</Button></nav>{/if}
</section>
