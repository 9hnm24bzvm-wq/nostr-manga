<script lang="ts">
 import AccountActionGate from './AccountActionGate.svelte';
 import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
 import {onDestroy,untrack} from 'svelte';
 import {PUBLIC_MANGADEX_CATALOGUE,PUBLIC_SITE_NAME} from '$env/static/public';
 import {Button} from '$lib/components/ui/button';
 import MangaSearchEntryCard from '$lib/components/MangaSearchEntryCard.svelte';
 import {Session} from '$lib/stores';
 import {loadSessionFollowing as loadAccountFollowing,type AccountFollowingRow} from './session-following-reader';
 import type{AccountFollowKind,AccountFollowView} from './account-follow-client';
 import AccountFollow from './SessionFollow.svelte';
 import FollowingNavigation from './FollowingNavigation.svelte';
 const {kind}:{kind:AccountFollowKind}=$props();
 let access=$state<NativeAccountIssue>();
 let reader:Awaited<ReturnType<typeof loadAccountFollowing>>|undefined,epoch=0,disposed=false;
 let rows=$state<AccountFollowingRow[]>([]),pending=$state(false),error=$state(''),username=$state(''),accountId=$state(''),offset=$state(0),total=$state(0);
 const title=$derived(kind==='manga'?'Followed titles':kind==='group'?'Followed groups':'Followed users');
 function clear(){epoch++;reader?.dispose();reader=undefined;access=undefined;rows=[];pending=false;error='';username='';accountId='';offset=0;total=0;}
 function guard(){const revision=epoch,type=kind,session=$Session;return()=>{if(disposed||epoch!==revision||kind!==type||session!==$Session)throw Error('The selected following view or account changed');};}
 async function load(start=0){if(pending||!$Session?.identity?.id)return;const assert=guard();pending=true;error='';access=undefined;rows=[];username='';accountId='';total=0;try{if(PUBLIC_MANGADEX_CATALOGUE!=='1')throw Error('Account following is unavailable in this catalogue.');const selected=reader??await loadAccountFollowing(kind,$Session!.identity.id,assert);try{assert();}catch(cause){selected.dispose();throw cause;}reader=selected;const result=await selected.page(start);assert();reader=selected;username=selected.username;accountId=selected.accountId;rows=result.data;offset=result.offset;total=result.total;}catch(cause){try{assert();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Account following could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 function refresh(){if(pending)return;reader?.dispose();reader=undefined;void load();}
 async function changed(view:AccountFollowView){if(view.accountId!==accountId)return;await load(offset);}
 function target(id:string){return'/'+(kind==='manga'?'title':kind)+'/'+id;}
 $effect(()=>{const type=kind,session=$Session;untrack(()=>{clear();if(session?.identity?.id)void load();});});
 onDestroy(()=>{disposed=true;epoch++;reader?.dispose();});
</script>
<svelte:head><title>{title} - {PUBLIC_SITE_NAME}</title></svelte:head>
<section class="space-y-5">
 <header class="space-y-3"><h1 class="text-xl font-semibold">{title}</h1><nav class="flex flex-wrap items-center gap-3"><a class="text-sm underline" href="/library">My library</a><Button variant="outline" disabled={pending||!$Session?.identity?.id} onclick={refresh}>Refresh</Button></nav></header>
 <FollowingNavigation active={kind}/>
 <AccountActionGate issue={access} onready={() => load()} />
 {#if !$Session?.identity?.id}<p class="text-muted-foreground">Sign in to see the titles, groups and readers you follow.</p>{/if}
 {#if username}<p class="text-sm text-muted-foreground">Following as {username}. <a class="underline" href="/settings/account">Account settings</a></p>{/if}
 {#if pending}<p role="status" class="text-muted-foreground">Loading account following…</p>{/if}
 {#if error}<p role="alert" class="text-destructive">{error}</p>{:else if username&&!pending&&!rows.length}<p class="text-muted-foreground">There are no available followed {kind==='manga'?'titles':kind==='group'?'groups':'users'} on this page.</p>{/if}
 <div class="grid gap-3 md:grid-cols-2">{#each rows as row (row.id)}<div class="space-y-2">
  {#if row.manga}<MangaSearchEntryCard manga={row.manga as unknown as Manga}/>{:else}<a class="block rounded-md border bg-card p-4 hover:border-accent" href={target(row.id)}><h2 class="font-medium">{row.name}</h2></a>{/if}
  <AccountFollow {kind} targetId={row.id} requestedId={row.id} expectedAccountId={accountId} onChanged={changed} label="Manage follow"/>
 </div>{/each}</div>
 {#if offset>0||total>20}<nav class="flex gap-2" aria-label="Account following pages"><Button variant="outline" disabled={pending||!!error||offset===0} onclick={()=>load(Math.max(0,offset-20))}>Previous</Button><Button variant="outline" disabled={pending||!!error||offset+20>=total||offset+20>10000} onclick={()=>load(offset+20)}>Next</Button></nav>{/if}
</section>
