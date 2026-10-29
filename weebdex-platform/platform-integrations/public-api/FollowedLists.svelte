<script lang="ts">
 import AccountActionGate from './AccountActionGate.svelte';
 import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
 import {onDestroy,untrack} from 'svelte';
 import {PUBLIC_MANGADEX_CATALOGUE,PUBLIC_SITE_NAME} from '$env/static/public';
 import {Button} from '$lib/components/ui/button';
 import FollowingNavigation from './FollowingNavigation.svelte';
 import {Session} from '$lib/stores';
 import {loadSessionFollowedLists as loadFollowedLists} from './session-list-follow-client';
 import type{PublicListView}from './public-lists';
 let access=$state<NativeAccountIssue>();
 let reader:Awaited<ReturnType<typeof loadFollowedLists>>|undefined,epoch=0,disposed=false;
 let rows=$state<PublicListView[]>([]),pending=$state(false),error=$state(''),username=$state(''),offset=$state(0),total=$state(0),unavailable=$state(0);
 function clear(){epoch++;reader?.dispose();reader=undefined;access=undefined;rows=[];pending=false;error='';username='';offset=0;total=0;unavailable=0;}
 function guard(){const revision=epoch,session=$Session;return()=>{if(disposed||epoch!==revision||session!==$Session)throw Error('The following account changed');};}
 async function load(start=0){if(pending||!$Session?.identity?.id)return;const assert=guard();pending=true;error='';access=undefined;rows=[];username='';unavailable=0;total=0;try{if(PUBLIC_MANGADEX_CATALOGUE!=='1')throw Error('Public-list following is unavailable in this catalogue.');const selected=reader??await loadFollowedLists($Session!.identity.id,assert);try{assert();}catch(cause){selected.dispose();throw cause;}reader=selected;const result=await selected.page(start);assert();reader=selected;username=selected.username;rows=result.data;offset=result.offset;total=result.total;unavailable=result.unavailable;}catch(cause){try{assert();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Followed lists could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 $effect(()=>{const session=$Session;untrack(()=>{clear();if(session?.identity?.id)void load();});});
 onDestroy(()=>{disposed=true;epoch++;reader?.dispose();});
</script>
<svelte:head><title>Followed lists - {PUBLIC_SITE_NAME}</title></svelte:head>
<section class="space-y-5">
 <header class="space-y-3"><h1 class="text-xl font-semibold">Followed lists</h1><nav class="flex flex-wrap items-center gap-3"><a class="text-sm underline" href="/library">My library</a><Button variant="outline" disabled={pending||!$Session?.identity?.id} onclick={()=>{reader?.dispose();reader=undefined;void load();}}>Refresh</Button></nav></header>
 <FollowingNavigation active="list"/>
 <AccountActionGate issue={access} onready={() => load()} />
 {#if !$Session?.identity?.id}<p class="text-muted-foreground">Sign in to see the lists you follow.</p>{/if}
 {#if username}<p class="text-sm text-muted-foreground">Following as {username}.</p>{/if}
 {#if pending}<p role="status" class="text-muted-foreground">Loading followed lists…</p>{/if}
 {#if error}<p role="alert" class="text-destructive">{error}</p>{:else if username&&!pending&&!rows.length}<p class="text-muted-foreground">{offset>0?'There are no available followed lists on this page.':'You have no available followed lists.'}</p>{/if}
 {#if unavailable}<p class="text-sm text-muted-foreground">{unavailable.toLocaleString()} saved {unavailable===1?'list is':'lists are'} currently unavailable.</p>{/if}
 <div class="grid gap-3 md:grid-cols-2">{#each rows as list (list.id)}<a class="rounded-md border bg-card p-4 hover:border-accent" href="/list/{list.id}"><h2 class="font-medium">{list.name}</h2><p class="text-sm text-muted-foreground">{list.members.length.toLocaleString()} available titles</p></a>{/each}</div>
 {#if offset>0||total>20}<nav class="flex gap-2" aria-label="Followed list pages"><Button variant="outline" disabled={pending||!!error||offset===0} onclick={()=>load(Math.max(0,offset-20))}>Previous</Button><Button variant="outline" disabled={pending||!!error||offset+20>=total||offset+20>10000} onclick={()=>load(offset+20)}>Next</Button></nav>{/if}
</section>
