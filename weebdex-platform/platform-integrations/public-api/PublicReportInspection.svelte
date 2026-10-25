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
 async function navigate(query:URLSearchParams){const href='/reports/signed'+(query.size?'?'+query:'');reset();await goto(href);await tick();if(!disposed&&window.location.pathname+window.location.search===href)await load();}
 async function apply(event:SubmitEvent){event.preventDefault();const query=new URLSearchParams(page.url.search);query.delete('page');for(const [name,value] of [['status',status],['category',category]]){if(value)query.set(name,value);else query.delete(name);}await navigate(query);}
 async function turnPage(delta:number){if(busy||!feed)return;const next=feed.page+delta;if(next<1||next>reportMaxPage)return;const query=new URLSearchParams(page.url.search);query.set('page',String(next));await navigate(query);}
 function cancel(){reset();message='Inspection cancelled. Nothing was resubmitted.';}
</script>
<svelte:head><title>My Signed Reports</title></svelte:head>
<section class="flex flex-auto flex-col space-y-5">
 <header class="space-y-3">
  <h2 class="text-xl font-medium">My Signed Reports</h2>
  <p>Private instance reports for the account bound to your selected signing identity. Native-session cookies do not grant access here.</p>
  <p><a class="underline" href="/reports/signed/native">Inspect my signed native thread and reply reports</a> using this signing identity. Native IDs remain separate from public UUID subjects.</p>
  <p><a class="underline" href="/my/reports">Open native-session reports</a> only for the separate browser-login account. This does not switch the signed account.</p>
  <p role="note">Inspection is read-only. Similar reports, an empty page, or a changed status cannot confirm whether an uncertain submission succeeded. Inspection never clears its pending safeguard or resubmits it. Do not retry an unconfirmed submission based on these results; ask this instance’s operator to reconcile it.</p>
 </header>
 {#if !$signerIdentity.owner}<p>Connect a signing identity in <a class="underline" href="/settings/identity">identity settings</a> to inspect its explicitly bound account’s reports.</p>{/if}
 <form class="flex flex-wrap items-end gap-3" onsubmit={apply}>
  <label>Status <select aria-label="Report status" bind:value={status} disabled={busy}><option value="">All statuses</option>{#each reportStatuses as value}<option {value}>{value}</option>{/each}</select></label>
  <label>Category <select aria-label="Report category" bind:value={category} disabled={busy}><option value="">All categories</option>{#each reportCategories as value}<option {value}>{value}</option>{/each}</select></label>
  <button type="submit" disabled={busy||!$signerIdentity.owner}>Apply report filters</button>
 </form>
 {#if page.url.searchParams.has('objectId')}<p>Subject UUID: {page.url.searchParams.get('objectId')} <a class="underline" href="/reports/signed">Clear subject filter</a></p>{/if}
 <div class="flex gap-3"><button type="button" disabled={busy||!$signerIdentity.owner} onclick={load}>{feed?'Reload personal reports':'Load personal reports'}</button>{#if busy}<button type="button" onclick={cancel}>Cancel inspection</button>{/if}</div>
 {#if message}<p role="alert">{message}</p>{/if}
 {#if busy}<p role="status">Inspecting signed personal reports…</p>{/if}
 {#if feed}
  <p>Report snapshot for {accountName}. Only this account’s reports are shown, including when it has moderator permissions.</p>
  <p>{feed.total} matching reports. Page {feed.page}, up to {feed.limit} reports per page.</p>
  {#if !feed.data.length}<p>No reports were returned for this account, filter and page. This does not prove that any uncertain submission failed.</p>{/if}
  <ul class="space-y-5">{#each feed.data as report (report.id)}<li class="space-y-1">
   <p><strong>{report.status}</strong> · {report.category} · <time datetime={report.createdAt}>{report.createdAt}</time></p>
   <p>Report UUID: {report.id}</p><p>Subject UUID: {report.objectId}</p>
   <p>Reason: {report.reason} (current reason revision {report.reasonVersion})</p>
   <p class="whitespace-pre-wrap">{report.details}</p>
  </li>{/each}</ul>
  <nav aria-label="Personal report pages" class="flex gap-3"><button type="button" disabled={busy||feed.page<=1} onclick={()=>turnPage(-1)}>Previous report page</button><button type="button" disabled={busy||feed.page>=reportMaxPage||feed.offset+feed.limit>=feed.total} onclick={()=>turnPage(1)}>Next report page</button></nav>
  {#if feed.page>=reportMaxPage&&feed.offset+feed.limit<feed.total}<p>The bounded pagination limit has been reached. Narrow the status or category filter; no additional pages were requested.</p>{/if}
 {/if}
</section>
