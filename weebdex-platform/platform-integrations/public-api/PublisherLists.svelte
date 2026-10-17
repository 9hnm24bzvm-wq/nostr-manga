<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {PUBLIC_MANGADEX_CATALOGUE} from '$env/static/public';
 import {Button} from '$lib/components/ui/button';
 import {publicLists,type PublicListView} from './public-lists';
 const {ownerId}:{ownerId:string}=$props();
 let rows=$state<PublicListView[]>([]),pending=$state(false),error=$state(''),offset=$state(0),total=$state(0),head:string|undefined,epoch=0,disposed=false;
 async function load(start=0){const owner=ownerId,revision=++epoch,check=()=>{if(disposed||epoch!==revision||ownerId!==owner)throw Error('The selected creator changed');};pending=true;error='';rows=[];total=0;try{if(PUBLIC_MANGADEX_CATALOGUE!=='1')throw Error('Published lists are unavailable in this catalogue.');const result=await publicLists.owner(owner,start,start?head:undefined,check);check();rows=result.data;offset=result.offset;total=result.total;head=result.head??undefined;}catch(cause){try{check();error=cause instanceof Error?cause.message:'Published lists could not be loaded.';}catch{}}finally{try{check();pending=false;}catch{}}}
 $effect(()=>{const owner=ownerId;untrack(()=>{head=undefined;offset=0;total=0;void load();});});
 onDestroy(()=>{disposed=true;epoch++;});
</script>
<section class="space-y-4">
 <header class="flex flex-wrap items-center justify-between gap-3"><h2 class="text-lg font-medium">Published lists</h2><Button variant="outline" disabled={pending} onclick={()=>load()}>Refresh</Button></header>
 {#if pending}<p role="status" class="text-muted-foreground">Loading published lists…</p>{/if}
 {#if error}<p role="alert" class="text-destructive">{error}</p>{:else if !pending&&!rows.length}<p class="text-muted-foreground">This creator has no published lists.</p>{/if}
 <div class="grid gap-3 md:grid-cols-2">{#each rows as list (list.id)}<a class="rounded-md border bg-card p-4 hover:border-accent" href="/list/{list.id}"><h3 class="font-medium">{list.name}</h3><p class="text-sm text-muted-foreground">{list.members.length.toLocaleString()} available titles</p></a>{/each}</div>
 {#if total>20}<nav class="flex gap-2" aria-label="Published list pages"><Button variant="outline" disabled={pending||offset===0} onclick={()=>load(Math.max(0,offset-20))}>Previous</Button><Button variant="outline" disabled={pending||offset+20>=total||offset+20>10000} onclick={()=>load(offset+20)}>Next</Button></nav>{/if}
</section>
