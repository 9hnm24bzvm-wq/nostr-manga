<script lang="ts">
 import { onDestroy, untrack } from 'svelte';
 import { page } from '$app/state';
 import { Session } from '$lib/stores';
 import AccountActionGate from './AccountActionGate.svelte';
 import { nativeAccountIssue, type NativeAccountIssue } from './native-account-scope';
 import { Button } from '$lib/components/ui/button';
 import Grid from '$lib/components/ui/Grid.svelte';
 import * as Empty from '$lib/components/ui/empty';
 import { Spinner } from '$lib/components/ui/spinner';
 import { createSessionDraftReader } from './session-draft-client';
 import PublicDraftEntry from './PublicDraftEntry.svelte';
 import type { MangaView, ViewFeed } from './types';
 let {feed=$bindable()}:{feed:ViewFeed<MangaView>|undefined}=$props();
 let reader:Awaited<ReturnType<typeof createSessionDraftReader>>|undefined;
 let issue=$state<NativeAccountIssue>();
 let generation=0,disposed=false,busy=$state(false),message=$state(''),account=$state<Awaited<ReturnType<typeof createSessionDraftReader>>['account']>();
 function reset(){generation++;reader?.dispose();reader=undefined;feed=undefined;busy=false;message='';account=undefined;issue=undefined;}
 $effect(()=>{const session=$Session;const route=page.url.href;untrack(()=>{reset();if(session?.identity?.id)void load();});});
 onDestroy(()=>{disposed=true;reset();});
 function capture(){const token=generation,href=window.location.href,session=$Session;return()=>{if(disposed||$Session!==session||token!==generation||window.location.href!==href)throw Error('The draft route or selected account changed.');};}
 async function load(){if(busy||!$Session?.identity?.id)return;const check=capture(),token=generation,query=new URLSearchParams(page.url.search);busy=true;message='';feed=undefined;account=undefined;reader?.dispose();reader=undefined;
  try{const next=await createSessionDraftReader($Session!.identity.id,check);try{check();}catch(error){next.dispose();throw error;}reader=next;const result=await next.read(query);check();account=next.account;feed=result;}
  catch(error){if(!disposed&&token===generation){issue=nativeAccountIssue(error);message=error instanceof Error?error.message:'Drafts could not be loaded.';}}
  finally{if(!disposed&&token===generation)busy=false;}
 }
 function changed(target:string,value:MangaView|undefined){reader?.check();if(!feed)return;if(value)feed={...feed,data:feed.data.map(row=>row.id===target?value:row)};else feed={...feed,total:Math.max(0,feed.total-1),data:feed.data.filter(row=>row.id!==target)};}
</script>
<div class="space-y-3">
 <AccountActionGate {issue} onready={load} />
 <Button type="button" variant="outline" disabled={busy||!$Session?.identity?.id} onclick={load}>{feed?'Reload drafts':'Load drafts'}</Button>
 {#if account}<p class="text-muted-foreground text-sm">Draft access for {account.name}. {account.roles.some(role=>['moderator','staff','sysop'].includes(role))?'Moderator access is limited by the current instance grants.':'Only this account’s drafts are shown.'}</p>{/if}
 {#if message}<p role="alert">{message}</p>{/if}
 {#if busy}<Empty.Root><Spinner class="size-10"/></Empty.Root>
 {:else if feed&&account}
  {#if feed.data.length}<Grid class="gap-1" breakpoints={{0:3,1400:2,920:1}}>{#each feed.data as manga (manga.id)}<PublicDraftEntry nativeAccount={$Session!.identity.id} {manga} {account} assertCurrent={()=>reader!.check()} onChanged={value=>changed(manga.id,value)}/>{/each}</Grid>
  {:else}<Empty.Root><Empty.Header><Empty.Title>No results found</Empty.Title><Empty.Description>No drafts were returned for this account and filter.</Empty.Description></Empty.Header></Empty.Root>{/if}
 {/if}
</div>
