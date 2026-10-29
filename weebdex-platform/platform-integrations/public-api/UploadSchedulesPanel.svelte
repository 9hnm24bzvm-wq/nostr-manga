<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import type { PublicUploadOwner } from './upload-client';
  import type { ScheduleEntry, ScheduleLimits, RememberedSchedule } from './upload-schedules';
  let { owner, revision = 0 }: { owner: PublicUploadOwner; revision?: number } = $props();
  let expanded = $state(false);
  const localTime = (value: string | undefined) => value ? new Date(value + 'Z').toLocaleString(undefined, { year: 'numeric', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', timeZoneName: 'short' }) : 'Time unavailable';
  const retryTime = (seconds: number) => seconds % 3600 === 0 ? `${seconds / 3600} hours` : seconds % 60 === 0 ? `${seconds / 60} minutes` : `${seconds} seconds`;
  const scheduleState = (state: string) => ({ scheduled: 'Scheduled', starting: 'Publishing', blocked: 'Needs attention', complete: 'Finished', cancelled: 'Cancelled' }[state] ?? 'Check status');
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
{#if (owner as PublicUploadOwner & {schedulingAvailable?:boolean}).schedulingAvailable===false}<p class="text-sm text-muted-foreground">Scheduled publishing is not available on this site. You can upload now instead.</p>{/if}
<details bind:open={expanded} class="rounded border px-4 py-2">
  <summary class="cursor-pointer font-medium">Scheduled uploads{#if entries.length + recent.length}{' '}({entries.length + recent.length}){/if}{#if message || entries.some(entry => entry.uncertain || entry.receipt.state === 'blocked') || recent.some(entry => entry.uncertain)}{' '}· Needs attention{/if}</summary>
  {#if expanded}
  <section aria-label="Private scheduled uploads" class="space-y-3 pt-3">
  {#if entries.length || recent.length}
  <p>Scheduled pages stay private until publishing begins, at or after the time you chose. A schedule does not guarantee that the chapter is published.</p>
  <p>You cannot change a scheduled upload. To make changes, cancel it before publishing starts and create a new upload. You can keep uploading other chapters meanwhile.</p>
  {/if}
  <button type="button" class="underline" disabled={busy} onclick={refresh}>{busy ? 'Checking…' : 'Refresh schedules'}</button>
  {#if message}<p role="status">{message}</p>{/if}
  {#if !entries.length && !recent.length && !busy && !message}<p>No scheduled uploads. Upload a chapter and choose a publishing time to add one.</p>{/if}
  <ul class="space-y-4">
    {#each entries as entry (entry.sessionId)}
      <li class="space-y-1 border-t pt-3">
        <p class="font-medium">{typeof entry.receipt.resource.attributes.title === 'string' && entry.receipt.resource.attributes.title || 'Scheduled upload'} · {entry.sessionId.slice(0, 8)}</p>
        <p>Publishing time: {localTime(entry.receipt.publishAt)}</p>
        <p>{scheduleState(entry.receipt.state)}. {entry.receipt.readerAvailable ? 'Check that the chapter is available before opening it below.' : entry.receipt.sourceAccepted === true ? 'The publisher accepted the chapter, but reading availability is not yet confirmed.' : entry.receipt.sourceAccepted === null ? 'Publication is not confirmed. Keep your original files.' : 'Publication has not been confirmed.'}</p>
        {#if entry.receipt.blockedReason}<p>Reason: {entry.receipt.blockedReason}</p>{/if}
        {#if entry.uncertain}<p role="status">The last change has an unconfirmed result. Do not repeat it. Check saved status first; nothing will be repeated automatically.</p>{/if}
        {#if entry.receipt.canRetry}<p>Retrying keeps the same pages and details. Its retry window is {retryTime(entry.receipt.retryWindowSeconds)}.</p>{/if}
        <div class="flex flex-wrap gap-4">
          <button type="button" class="underline" disabled={busy} onclick={() => act(entry.sessionId, 'get')}>Check saved status</button>
          {#if entry.receipt.canCancel}<button type="button" class="underline" disabled={busy || !!entry.uncertain} onclick={() => act(entry.sessionId, 'cancel')}>Cancel scheduled upload</button>{/if}
          {#if entry.receipt.canRetry}<button type="button" class="underline" disabled={busy || !!entry.uncertain} onclick={() => act(entry.sessionId, 'retry')}>Retry this upload</button>{/if}
          {#if entry.receipt.readerAvailable}<button type="button" class="underline" disabled={busy} onclick={() => act(entry.sessionId, 'read')}>Check chapter availability</button>{/if}
          {#if readable[entry.sessionId]}<a class="underline" href={`/chapter/${readable[entry.sessionId]}`}>Read chapter</a>{/if}
        </div>
      </li>
    {/each}
  </ul>
  {#if recent.length}
    <h3 class="font-semibold">Previous uploads</h3>
    <p>These uploads are no longer in the active list. Check their status; being absent does not mean they were published or cancelled.</p>
    <ul class="space-y-2">
      {#each recent as item (item.sessionId)}<li>
        <span>Upload {item.sessionId.slice(0, 8)}{item.publishAt ? ` · ${localTime(item.publishAt)}` : ''}</span>
        {#if item.uncertain}<p>The last change has an unconfirmed result. Do not repeat it; check saved status first.</p>{/if}
        <button type="button" class="underline" disabled={busy} onclick={() => act(item.sessionId, 'get')}>Check saved status for {item.sessionId.slice(0, 8)}</button>
      </li>{/each}
    </ul>
  {/if}
</section>
  {/if}
</details>
