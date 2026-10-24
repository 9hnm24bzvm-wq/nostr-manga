<script lang="ts">
 import { goto } from '$app/navigation';
 import { page } from '$app/state';
 import { Button } from '$lib/components/ui/button';
 import * as Select from '$lib/components/ui/select';
 import { SearchIcon } from '@lucide/svelte';
 import type { MangaView, ViewFeed } from './types';
 let {feed=$bindable()}:{feed:ViewFeed<MangaView>|undefined}=$props();
 let selectedState=$state(page.url.searchParams.get('state')??''),sort=$state(page.url.searchParams.get('sort')??'created_at'),order=$state(page.url.searchParams.get('order')??'desc');
 $effect(()=>{selectedState=page.url.searchParams.get('state')??'';sort=page.url.searchParams.get('sort')??'created_at';order=page.url.searchParams.get('order')??'desc';});
 const states={draft:'Draft',submitted:'Submitted',rejected:'Rejected'},sorts={created_at:'Created at',updated_at:'Updated at',title:'Title',year:'Year'};
 function submit(event?:SubmitEvent){event?.preventDefault();const query=new URLSearchParams();if(selectedState)query.set('state',selectedState);if(sort!=='created_at')query.set('sort',sort);if(order!=='desc')query.set('order',order);const encoded=query.toString().replace(/^\?/,'');feed=undefined;void goto(page.url.pathname+(encoded?'?'+encoded:''));}
 function reset(event:Event){event.preventDefault();selectedState='';sort='created_at';order='desc';submit();}
</script>
<form class="space-y-1" onsubmit={submit} onreset={reset}>
 <div class="flex flex-wrap items-center gap-1">
  <Select.Root type="single" bind:value={selectedState}><Select.Trigger class="flex w-0 min-w-45 flex-auto border-none">State: {states[selectedState as keyof typeof states]??'Any'}</Select.Trigger><Select.Content><Select.Item value="">Any</Select.Item>{#each Object.entries(states) as [key,value]}<Select.Item value={key}>{value}</Select.Item>{/each}</Select.Content></Select.Root>
  <Select.Root type="single" bind:value={sort}><Select.Trigger class="text-muted-foreground flex w-0 min-w-45 flex-auto border-none">Sort: {sorts[sort as keyof typeof sorts]??'Created at'}</Select.Trigger><Select.Content>{#each Object.entries(sorts) as [key,value]}<Select.Item value={key}>{value}</Select.Item>{/each}</Select.Content></Select.Root>
  <Select.Root type="single" bind:value={order}><Select.Trigger class="text-muted-foreground flex w-0 min-w-60 flex-auto border-none">Order: {order==='asc'?'Ascending':'Descending'}</Select.Trigger><Select.Content><Select.Item value="asc">Ascending</Select.Item><Select.Item value="desc">Descending</Select.Item></Select.Content></Select.Root>
 </div>
 <div class="mt-5 flex items-center gap-2"><div class="w-0 flex-auto"><span class="text-muted-foreground">{#if feed}Found {feed.total} results{/if}</span></div><div class="flex shrink-0 items-center gap-1"><Button class="!text-destructive border-none" variant="outline" type="reset" disabled={!selectedState&&sort==='created_at'&&order==='desc'}>Reset</Button><Button type="submit"><SearchIcon/>Search</Button></div></div>
</form>
