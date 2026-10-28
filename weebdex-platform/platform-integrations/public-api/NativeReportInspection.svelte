<script lang="ts">
 import { onDestroy, untrack, tick } from 'svelte';
 import { page } from '$app/state';
 import { goto } from '$app/navigation';
 import { signerIdentity } from '$localLibrary/publication';
 import { createNativeReportReader, nativeReportInspectionQuery, nativeReportInspectionError, nativeReportDetailHref, nativeReportStatuses, nativeReportCategories, nativeReportMaxPage, type NativeReportSnapshot } from './native-report-inspection';
 let reader: Awaited<ReturnType<typeof createNativeReportReader>> | undefined, lifetime: AbortController | undefined;
 let generation = 0, disposed = false, busy = $state(false), message = $state(''), feed = $state<NativeReportSnapshot>();
 let accountName = $state(''), accountId = $state(''), status = $state(''), category = $state(''), objectId = $state(''), reasonId = $state('');
 const isDetail = $derived(page.url.searchParams.has('reportId'));
 function reset() { generation++; lifetime?.abort(); lifetime = undefined; reader?.dispose(); reader = undefined; feed = undefined; accountName = ''; accountId = ''; message = ''; busy = false; }
 $effect(() => {
  const identity = $signerIdentity, route = page.url.href;
  untrack(() => { reset(); status = page.url.searchParams.get('status') ?? ''; category = page.url.searchParams.get('category') ?? ''; objectId = page.url.searchParams.get('objectId') ?? ''; reasonId = page.url.searchParams.get('reasonId') ?? ''; });
 });
 onDestroy(() => { disposed = true; reset(); });
 function capture() { const token = generation, href = window.location.href; return () => { if (disposed || token !== generation || window.location.href !== href) throw Error('Inspection lifetime changed'); }; }
 async function load() {
  if (busy || !$signerIdentity.owner) return;
  reset(); const check = capture(), token = generation, query = new URLSearchParams(page.url.search); busy = true; lifetime = new AbortController();
  try {
   nativeReportInspectionQuery(query);
   const next = await createNativeReportReader(check, {}, lifetime.signal);
   try { check(); } catch (error) { next.dispose(); throw error; }
   reader = next; const result = await next.read(query); check(); accountName = next.account.name; accountId = next.account.id; feed = result;
  } catch (error) { if (!disposed && token === generation) { reader?.dispose(); reader = undefined; message = nativeReportInspectionError(error); } }
  finally { if (!disposed && token === generation) busy = false; }
 }
 async function navigate(query: URLSearchParams) {
  const href = '/reports/signed/native' + (query.size ? '?' + query.toString().replace(/^\?/, '') : '');
  reset(); await goto(href); await tick(); if (!disposed && window.location.pathname + window.location.search === href) await load();
 }
 async function apply(event: SubmitEvent) {
  event.preventDefault(); const query = new URLSearchParams();
  for (const [name, value] of [['status', status], ['category', category], ['objectId', objectId], ['reasonId', reasonId]]) if (value) query.set(name, value);
  await navigate(query);
 }
 async function turnPage(delta: number) {
  if (busy || !feed || feed.detail) return;
  const next = feed.page + delta; if (next < 1 || next > nativeReportMaxPage) return;
  const query = new URLSearchParams(page.url.search); query.set('page', String(next)); await navigate(query);
 }
 function cancel() { reset(); message = 'Inspection cancelled. Nothing was resubmitted.'; }
</script>
<svelte:head><title>My discussion reports</title></svelte:head>
<section class="flex flex-auto flex-col space-y-5">
 <header class="space-y-3">
  <h2 class="text-xl font-medium">My discussion reports</h2>
  <p>Private discussion reports from your selected identity. Your browser-login account’s reports are separate.</p>
  <nav aria-label="Report identity modes" class="flex flex-wrap gap-3"><a class="underline" href="/reports/signed">Catalogue reports</a><a class="underline" href="/my/reports">Browser-account reports</a></nav>
  <p role="note">Read-only inspection cannot confirm an uncertain submission from a similar report, a changed status, or an empty page. It never clears a pending safeguard or repeats a report. Ask this instance’s operator to reconcile an unconfirmed submission.</p>
 </header>
 {#if !$signerIdentity.owner}<p>Choose your identity in <a class="underline" href="/settings/identity">identity settings</a> to view its reports.</p>{/if}
 {#if isDetail}<p>Report details. <a class="underline" href="/reports/signed/native">Return to my discussion reports</a></p>
 {:else}
 <form class="flex flex-wrap items-end gap-3" onsubmit={apply}>
  <label>Status <select aria-label="Native report status" bind:value={status} disabled={busy}><option value="">All statuses</option>{#each nativeReportStatuses as value}<option {value}>{value}</option>{/each}</select></label>
  <label>Category <select aria-label="Native report category" bind:value={category} disabled={busy}><option value="">Thread and reply</option>{#each nativeReportCategories as value}<option {value}>{value}</option>{/each}</select></label>
  <label>Reported item reference <input aria-label="Native report subject ID" bind:value={objectId} maxlength="128" disabled={busy}/></label>
  <label>Reason reference <input aria-label="Native report reason ID" bind:value={reasonId} maxlength="128" disabled={busy}/></label>
  <button type="submit" disabled={busy || !$signerIdentity.owner}>Apply native report filters</button>
 </form>
 {/if}
 <div class="flex gap-3"><button type="button" disabled={busy || !$signerIdentity.owner} onclick={load}>{feed ? 'Reload native reports' : 'Load native reports'}</button>{#if busy}<button type="button" onclick={cancel}>Cancel native inspection</button>{/if}</div>
 {#if message}<p role="alert">{message}</p>{/if}
 {#if busy}<p role="status">Inspecting signed native reports…</p>{/if}
 {#if feed}
  <p>Discussion reports from {accountName}. Only this account’s submissions are shown.</p>
  {#if !feed.detail}<p>{feed.total} matching reports. Page {feed.page}, up to {feed.limit} reports per page.</p>{/if}
  {#if !feed.data.length}<p>No reports were returned for this account, filter and page. This does not prove that an uncertain submission failed.</p>{/if}
  <ul class="space-y-5">{#each feed.data as report (report.id)}<li class="space-y-2 border-t pt-4">
   <p><strong>{report.status}</strong> · {report.category} · <time datetime={report.createdAt}>{report.createdAt}</time></p>
   <p>Report {report.id.slice(0, 8)}{#if !feed.detail} · <a class="underline" href={nativeReportDetailHref(report.id)}>View report</a>{/if}</p>
   <details><summary>Reference details for support</summary><p>Report: {report.id}. Reported item: {report.objectId}. The reported content is not shown here and may no longer be available.</p></details>
   <p>Current reason: {report.reason.reason}. The label may have changed since submission.</p>
   <p class="whitespace-pre-wrap">{report.details || 'No additional details were submitted.'}</p>
  </li>{/each}</ul>
  {#if !feed.detail}
   <nav aria-label="Signed native report pages" class="flex gap-3"><button type="button" disabled={busy || feed.page <= 1} onclick={() => turnPage(-1)}>Previous native report page</button><button type="button" disabled={busy || feed.page >= nativeReportMaxPage || feed.offset + feed.limit >= feed.total} onclick={() => turnPage(1)}>Next native report page</button></nav>
   {#if feed.page >= nativeReportMaxPage && feed.offset + feed.limit < feed.total}<p>More reports are available, but cannot all be shown at once. Narrow the filters to see them.</p>{/if}
  {/if}
 {/if}
</section>
