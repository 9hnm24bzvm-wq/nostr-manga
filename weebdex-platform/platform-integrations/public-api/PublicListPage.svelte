<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {PUBLIC_MANGADEX_CATALOGUE,PUBLIC_SITE_NAME} from '$env/static/public';
 import {Button} from '$lib/components/ui/button';
 import MangaSearchEntryCard from '$lib/components/MangaSearchEntryCard.svelte';
 import {publicLists,type PublicListView} from './public-lists';
 import {publicUuid} from './validation';
 import PublicListFollow from './PublicListFollow.svelte';
 const {id}:{id:string}=$props();
 let list=$state<PublicListView>(),titles=$state<Manga[]>([]),pending=$state(false),error=$state(''),missing=$state(false),validId=$state(''),epoch=0,disposed=false;
 async function load(){const target=id,revision=++epoch,check=()=>{if(disposed||epoch!==revision||id!==target)throw Error('The selected list changed');};list=undefined;titles=[];pending=true;error='';missing=false;validId='';try{if(PUBLIC_MANGADEX_CATALOGUE!=='1')throw Error('Published lists are unavailable in this catalogue.');validId=publicUuid(target);const row=await publicLists.get(validId,check);check();const members=await publicLists.members(row,check);check();list=row;titles=members as unknown as Manga[];}catch(cause){try{check();missing=(cause as{status?:number}).status===404;error=cause instanceof Error?cause.message:'This list could not be loaded.';}catch{}}finally{try{check();pending=false;}catch{}}}
 $effect(()=>{const target=id;untrack(()=>void load());});
 onDestroy(()=>{disposed=true;epoch++;});
</script>
<svelte:head><title>{list?.name??'Published list'} - {PUBLIC_SITE_NAME}</title></svelte:head>
<section class="space-y-5">
 <header class="space-y-3"><h1 class="text-xl font-semibold">{list?.name??'Published list'}</h1>
  <div class="flex flex-wrap items-center gap-3">
   {#if list}<a class="text-sm underline" href="/user/{list.ownerId}/lists">More from this creator</a><PublicListFollow listId={list.id} head={list.head}/>{:else if missing&&validId}<PublicListFollow listId={validId} unavailable/>{/if}
   <a class="text-sm underline" href="/following/lists">Followed lists</a><Button variant="outline" disabled={pending} onclick={load}>Refresh</Button>
  </div>
 </header>
 {#if pending}<p role="status" class="text-muted-foreground">Loading list…</p>{/if}
 {#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
 {#if list}
  {#if titles.length}<div class="grid gap-3 md:grid-cols-2">{#each titles as manga (manga.id)}<MangaSearchEntryCard {manga}/>{/each}</div>{:else}<p class="text-muted-foreground">This list has no available titles.</p>{/if}
 {/if}
</section>
