<script lang="ts">
 import {Button, buttonVariants} from '$lib/components/ui/button';
 import ImageLoader from '$lib/components/ui/ImageLoader.svelte';
 import {untrack} from 'svelte';
 import {Crown, ChessRook} from '@lucide/svelte';
 import {readMembershipPage, type PublicMembership} from './user-memberships';
 let {userId}:{userId:string}=$props();
 let groups=$state<PublicMembership[]>([]),next=$state<string|null>(null),busy=$state(false),loaded=$state(false),message=$state('');
 let generation=0,request:AbortController|undefined,active=false,queuedReset=false,available=false;
 let pageNumber=$state(0);
 async function load(reset=false){
  if(active){if(reset){queuedReset=true;request?.abort();}return;}
  active=true;const own=new AbortController();request=own;const ticket=++generation,target=userId,cursor=reset?null:next;
  if(reset){groups=[];next=null;loaded=false;pageNumber=0;}
  busy=true;message='';
  try{
   const page=await readMembershipPage(target,cursor,own.signal);
   if(ticket!==generation||own.signal.aborted||target!==userId)return;
   // A single bounded page, never an unbounded accumulated membership list.
   groups=page.data;next=page.nextCursor;loaded=true;pageNumber=reset?1:pageNumber+1;
  }catch(error){
   if(ticket!==generation||own.signal.aborted||target!==userId)return;
   groups=[];next=null;loaded=false;message=error instanceof Error?error.message:'Public memberships could not be loaded. Refresh to try again.';
  }finally{
   active=false;busy=false;request=undefined;
   if(queuedReset&&available){queuedReset=false;void load(true);}
  }
 }
 $effect(()=>{const target=userId;void target;available=true;groups=[];next=null;loaded=false;pageNumber=0;message='';untrack(()=>{void load(true);});return()=>{available=false;queuedReset=false;++generation;request?.abort();};});

</script>
<section class="space-y-2" aria-label="Public group memberships" aria-busy={busy}>
 <header class="flex items-center justify-between gap-2"><h2 class="text-base font-semibold">Groups</h2><Button variant="outline" size="sm" onclick={()=>load(true)}>Refresh groups</Button></header>
 <p class="text-muted-foreground text-sm">Public memberships on this instance. Pages are loaded live, not as a single snapshot. Next replaces this page; Refresh returns to the first page.</p>
 {#if message}<p role="alert">{message}</p>{/if}
 {#if loaded}<p class="text-muted-foreground text-sm">Page {pageNumber} · {groups.length} public groups shown</p>{/if}
 {#if groups.length}
  <div class="flex flex-wrap items-center gap-1">
   {#each groups as group (group.id)}
    <a class={buttonVariants({class:'w-0 min-w-[40%] flex-auto justify-start border-none px-3 py-4',variant:'outline'})} href={'/group/'+group.id} title={group.name}>
     <figure class="bg-card size-5 shrink-0 overflow-hidden rounded-full"><ImageLoader class="size-full object-cover" src="/images/avatar.webp" /></figure>
     <span class="w-0 flex-auto truncate">{group.name}</span>
     {#if group.isLeader}<span class="shrink-0" aria-label="Group leader" title="Group leader"><Crown /></span>{:else if group.isOfficer}<span class="shrink-0" aria-label="Group officer" title="Group officer"><ChessRook /></span>{/if}
    </a>
   {/each}
  </div>
 {/if}
 {#if busy}<p role="status">Loading public groups…</p>{:else if loaded&&!groups.length}<p>No public group memberships.</p>{/if}
 {#if next}<Button variant="outline" disabled={busy} onclick={()=>load()}>Next groups</Button>{/if}
</section>
