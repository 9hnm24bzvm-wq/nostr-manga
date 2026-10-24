<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import type { PublicUploadOwner } from './upload-client';
  import type { ScheduleEntry, ScheduleLimits, RememberedSchedule } from './upload-schedules';
  let { owner, revision = 0 }: { owner: PublicUploadOwner; revision?: number } = $props();
  let entries = $state<ScheduleEntry[]>([]), recent = $state<RememberedSchedule[]>([]), limits = $state<ScheduleLimits>(), busy = $state(false), message = $state('');
  let readable = $state<Record<string, string>>({}), mounted = false, disposed = false, generation = 0, lastOwner = untrack(() => owner);
  const current = (captured: PublicUploadOwner, run: number) => !disposed && captured === owner && run === generation && captured.alive();
  onDestroy(() => { disposed = true; generation++; });
  async function refresh() {
    if (busy) return;
    const captured = owner, run = ++generation; busy = true; message = ''; readable = {};
    try { const result = await captured.schedules.list(); if (current(captured, run)) { entries = result.entries; recent = result.recent; limits = result.limits; } }
    catch (error) { if (current(captured, run)) { entries = []; recent = []; limits = undefined; message = error instanceof Error ? error.message : 'Private schedule status is unavailable.'; } }
    finally { if (current(captured, run)) busy = false; }
  }
  onMount(() => { mounted = true; void refresh(); });
  $effect(() => { const changed = revision; if (mounted && changed > 0) untrack(() => { void refresh(); }); });
  $effect(() => { if (owner !== lastOwner) { lastOwner = owner; generation++; entries = []; recent = []; limits = undefined; readable = {}; busy = false; if (mounted) untrack(() => { void refresh(); }); } });
  async function act(id: string, action: 'get' | 'cancel' | 'retry' | 'read') {
    if (busy) return;
    const captured = owner, run = ++generation; busy = true; message = ''; delete readable[id];
    try {
      if (action === 'read') { const chapter = await captured.schedules.readableChapter(id); if (current(captured, run)) { if (chapter) readable[id] = chapter.id; else message = 'A readable chapter is not confirmed. No reader navigation was started.'; } }
      else { const result = await captured.schedules[action](id); if (current(captured, run)) { entries = entries.some(row => row.sessionId === id) ? entries.map(row => row.sessionId === id ? result : row) : [...entries, result]; recent = recent.filter(row => row.sessionId !== id); } }
    } catch (error) { if (current(captured, run)) { message = error instanceof Error ? error.message : 'This operation was not confirmed. Check its saved status before retrying.'; try { if (error && typeof error === 'object' && 'status' in error && [401, 403, 409].includes(Number(error.status))) { entries = []; recent = []; limits = undefined; } else { entries = entries.map(row => row.sessionId === id ? { ...row, uncertain: captured.schedules.pending(id) } : row); recent = recent.map(row => row.sessionId === id ? { ...row, uncertain: captured.schedules.pending(id) } : row); } } catch { message = 'Saved scheduling recovery could not be read. No further operation was sent.'; } } }
    finally { if (current(captured, run)) busy = false; }
  }
</script>
<section aria-label="Private scheduled uploads" class="space-y-3 rounded border p-4">
  <h2 class="font-semibold">Private scheduled uploads</h2>
  <p>Accepted schedules are not published chapters. Their original pages stay private until publication starts at or after the UTC deadline. A blocked or uncertain intent may retain storage until safely reconciled.</p>
  <p>Metadata and pages are immutable after scheduling. Cancel before publication starts, then create a new upload to change them. Scheduling does not occupy your open upload slot.</p>
  {#if limits}<p>Instance limits: {limits.maximumOutstanding} outstanding schedules, {Math.floor(limits.maximumRetainedBytes / 1048576)} MiB retained scheduling storage including derivatives, {Math.floor(limits.maximumHorizonSeconds / 86400)} days ahead. New intents use a {limits.retryWindowSeconds}-second retry window.</p>{/if}
  <button type="button" class="underline" disabled={busy} onclick={refresh}>{busy ? 'Checking…' : 'Refresh private schedules'}</button>
  {#if message}<p role="status">{message}</p>{/if}
  {#if !entries.length && !recent.length && !busy && !message}<p>No pending or recently remembered schedules.</p>{/if}
  <ul class="space-y-4">
    {#each entries as entry (entry.sessionId)}
      <li class="space-y-1 border-t pt-3">
        <p>Upload {entry.sessionId}</p>
        <p>Requested UTC: {entry.receipt.publishAt ?? 'not retained in this browser'} · Captured retry window: {entry.receipt.retryWindowSeconds} seconds</p>
        <p>Schedule: {entry.receipt.state}. {entry.receipt.readerAvailable ? 'A readable receipt is available; verify the current reader below.' : entry.receipt.sourceAccepted === true ? 'Original source acceptance verified; readable chapter not confirmed.' : entry.receipt.sourceAccepted === null ? 'Source acceptance is unverified. Retained originals must not be discarded.' : 'No chapter source acceptance is confirmed.'}</p>
        {#if entry.receipt.sourceState}<p>Source status: {entry.receipt.sourceState}</p>{/if}
        {#if entry.receipt.blockedReason}<p>Reason: {entry.receipt.blockedReason}</p>{/if}
        {#if entry.uncertain}<p role="status">The {entry.uncertain} response is unconfirmed. Only check saved status; no operation will be replayed.</p>{/if}
        <div class="flex flex-wrap gap-4">
          <button type="button" class="underline" disabled={busy} onclick={() => act(entry.sessionId, 'get')}>Check saved status</button>
          {#if entry.receipt.canCancel}<button type="button" class="underline" disabled={busy || !!entry.uncertain} onclick={() => act(entry.sessionId, 'cancel')}>Cancel scheduled upload</button>{/if}
          {#if entry.receipt.canRetry}<button type="button" class="underline" disabled={busy || !!entry.uncertain} onclick={() => act(entry.sessionId, 'retry')}>Retry checked immutable intent</button>{/if}
          {#if entry.receipt.readerAvailable}<button type="button" class="underline" disabled={busy} onclick={() => act(entry.sessionId, 'read')}>Verify readable chapter</button>{/if}
          {#if readable[entry.sessionId]}<a class="underline" href={`/chapter/${readable[entry.sessionId]}`}>Open verified reader</a>{/if}
        </div>
      </li>
    {/each}
  </ul>
  {#if recent.length}
    <h3 class="font-semibold">Recently remembered uploads</h3>
    <p>These are not in the active list. Check each private status explicitly; absence does not prove publication, cancellation or deleted storage.</p>
    <ul class="space-y-2">
      {#each recent as item (item.sessionId)}<li>
        <span>Upload {item.sessionId}{item.publishAt ? ` · Requested UTC: ${item.publishAt}` : ''}</span>
        {#if item.uncertain}<p>Unconfirmed {item.uncertain}; its saved recovery marker is retained.</p>{/if}
        <button type="button" class="underline" disabled={busy} onclick={() => act(item.sessionId, 'get')}>Check saved status for {item.sessionId}</button>
      </li>{/each}
    </ul>
  {/if}
</section>
