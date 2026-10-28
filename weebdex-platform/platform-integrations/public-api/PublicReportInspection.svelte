<script lang="ts">
 import { onDestroy, untrack, tick } from 'svelte';
 import { page } from '$app/state';
 import { goto } from '$app/navigation';
 import { signerIdentity } from '$localLibrary/publication';
 import { reportCategories } from './report-client';
 import { createSignedReportReader, reportInspectionQuery, reportStatuses, reportMaxPage, type InspectedReportPage } from './report-inspection';
 let reader:Awaited<ReturnType<typeof createSignedReportReader>>|undefined,lifetime:AbortController|undefined;
 let generation=0,disposed=false,busy=$state(false),message=$state(''),feed=$state<InspectedReportPage>(),accountName=$state(''),status=$state(''),category=$state('');
 function reset(){generation++;lifetime?.abort();lifetime=undefined;reader?.dispose();reader=undefined;feed=undefined;accountName='';message='';busy=false;}
 $effect(()=>{const identity=$signerIdentity,route=page.url.href;untrack(()=>{reset();status=page.url.searchParams.get('status')??'';category=page.url.searchParams.get('category')??'';});});
 onDestroy(()=>{disposed=true;reset();});
 function capture(){const token=generation,href=window.location.href;return()=>{if(disposed||token!==generation||window.location.href!==href)throw Error('The report inspection route or signing identity changed.');};}
 async function load(){if(busy||!$signerIdentity.owner)return;reset();const check=capture(),token=generation,query=new URLSearchParams(page.url.search);busy=true;lifetime=new AbortController();
  try{reportInspectionQuery(query);const next=await createSignedReportReader(check,{},lifetime.signal);try{check();}catch(error){next.dispose();throw error;}reader=next;const result=await next.read(query);check();accountName=next.account.name;feed=result;}
  catch(error){if(!disposed&&token===generation)message=error instanceof Error?error.message:'Personal reports could not be inspected.';}
  finally{if(!disposed&&token===generation)busy=false;}
 }
 async function navigate(query:URLSearchParams){const href='/reports/signed'+(query.size?'?'+query.toString().replace(/^\?/,''):'');reset();await goto(href);await tick();if(!disposed&&window.location.pathname+window.location.search===href)await load();}
 async function apply(event:SubmitEvent){event.preventDefault();const query=new URLSearchParams(page.url.search);query.delete('page');for(const [name,value] of [['status',status],['category',category]]){if(value)query.set(name,value);else query.delete(name);}await navigate(query);}
 async function turnPage(delta:number){if(busy||!feed)return;const next=feed.page+delta;if(next<1||next>reportMaxPage)return;const query=new URLSearchParams(page.url.search);query.set('page',String(next));await navigate(query);}
 function cancel(){reset();message='Inspection cancelled. Nothing was resubmitted.';}
</script>
<svelte:head><title>My catalogue reports</title></svelte:head>
<section class="flex flex-auto flex-col space-y-5">
 <header class="space-y-3">
  <h2 class="text-xl font-medium">My catalogue reports</h2>
  <p>Private reports from your selected identity. Your browser-login account’s reports are separate.</p>
  <p><a class="underline" href="/reports/signed/native">View my discussion reports</a> for this identity.</p>
  <p><a class="underline" href="/my/reports">View browser-account reports</a> for your separate browser-login account. This does not switch your selected identity.</p>
  <p role="note">Inspection is read-only. Similar reports, an empty page, or a changed status cannot confirm whether an uncertain submission succeeded. Inspection never clears its pending safeguard or resubmits it. Do not retry an unconfirmed submission based on these results; ask this instance’s operator to reconcile it.</p>
 </header>
 {#if !$signerIdentity.owner}<p>Choose your identity in <a class="underline" href="/settings/identity">identity settings</a> to view its reports.</p>{/if}
 <form class="flex flex-wrap items-end gap-3" onsubmit={apply}>
  <label>Status <select aria-label="Report status" bind:value={status} disabled={busy}><option value="">All statuses</option>{#each reportStatuses as value}<option {value}>{value}</option>{/each}</select></label>
  <label>Category <select aria-label="Report category" bind:value={category} disabled={busy}><option value="">All categories</option>{#each reportCategories as value}<option {value}>{value}</option>{/each}</select></label>
  <button type="submit" disabled={busy||!$signerIdentity.owner}>Apply report filters</button>
 </form>
 {#if page.url.searchParams.has('objectId')}<p>Showing reports for one selected item. <a class="underline" href="/reports/signed">Clear subject filter</a></p>{/if}
 <div class="flex gap-3"><button type="button" disabled={busy||!$signerIdentity.owner} onclick={load}>{feed?'Reload personal reports':'Load personal reports'}</button>{#if busy}<button type="button" onclick={cancel}>Cancel inspection</button>{/if}</div>
 {#if message}<p role="alert">{message}</p>{/if}
 {#if busy}<p role="status">Inspecting signed personal reports…</p>{/if}
 {#if feed}
  <p>Reports from {accountName}. Only this account’s submissions are shown.</p>
  <p>{feed.total} matching reports. Page {feed.page}, up to {feed.limit} reports per page.</p>
  {#if !feed.data.length}<p>No reports were returned for this account, filter and page. This does not prove that any uncertain submission failed.</p>{/if}
  <ul class="space-y-5">{#each feed.data as report (report.id)}<li class="space-y-1">
   <p><strong>{report.status}</strong> · {report.category} · <time datetime={report.createdAt}>{report.createdAt}</time></p>
   <details><summary>Reference details for support</summary><p>Report: {report.id}</p><p>Reported item: {report.objectId}</p></details>
   <p>Current reason: {report.reason}</p>
   <p class="whitespace-pre-wrap">{report.details}</p>
  </li>{/each}</ul>
  <nav aria-label="Personal report pages" class="flex gap-3"><button type="button" disabled={busy||feed.page<=1} onclick={()=>turnPage(-1)}>Previous report page</button><button type="button" disabled={busy||feed.page>=reportMaxPage||feed.offset+feed.limit>=feed.total} onclick={()=>turnPage(1)}>Next report page</button></nav>
  {#if feed.page>=reportMaxPage&&feed.offset+feed.limit<feed.total}<p>There are too many reports to show at once. Narrow the status or category filter to see more.</p>{/if}
 {/if}
</section>
