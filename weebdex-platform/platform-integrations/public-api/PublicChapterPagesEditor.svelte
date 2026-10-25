<script lang="ts">
  import { beforeNavigate } from '$app/navigation';
  import { onDestroy, onMount, untrack } from 'svelte';
  import { Mutex } from '$lib';
  import { setMutex, setUploadSession } from '$lib/Context';
  import UploadChapterPages from '$lib/components/upload/UploadChapterPages.svelte';
  import { setUploadOwner, prepareResume, acknowledge, signature, snapshot, isRemoteUploadFile, type PublicUploadOwner, type PublicSession } from './upload-client';
  import { createChapterPagesController } from './chapter-pages-controller';
  import type { ChapterEditAccess } from './chapter-access';
  let { chapter, owner, access, disabled = false, onlock, oncommit }: {
    chapter: Chapter; owner: PublicUploadOwner; access?: ChapterEditAccess; disabled?: boolean;
    onlock: (locked: boolean) => void; oncommit: (chapter: Chapter) => void;
  } = $props();
  const capturedOwner = untrack(() => owner), id = untrack(() => chapter.id), mutex = new Mutex();
  let session = $state<PublicSession>(), ready = $state(false), busy = $state(false), message = $state('');
  let recovery = $state(false), terms = $state(false), confirm = $state<'replace' | 'cancel'>(), savedSignature = $state('');
  let disposed = false;
  const current = () => { if (disposed || owner !== capturedOwner || chapter.id !== id) throw Error('The page editor changed. Staging was kept.'); capturedOwner.assert(); };
  const controller = createChapterPagesController(capturedOwner, () => chapter, current);
  setUploadOwner(capturedOwner); setMutex(mutex); setUploadSession(() => session);
  const foreign = $derived(!!session && (session.chapter_id !== id || session.manga_id !== chapter.relationships.manga.id || session.edit_version === undefined));
  const editable = $derived(ready && !disabled && !busy && !$mutex && !recovery && !foreign && access?.canEditPages === true);
  const filesReady = $derived(!!session?.relationships.files?.length && session.relationships.files.every(isRemoteUploadFile));
  const dirty = () => !!session && !foreign && signature(snapshot(session.draft ?? {}, session.relationships.files ?? [])) !== savedSignature;
  function updateRecovery() { recovery = capturedOwner.chapterBeginPending() || !!capturedOwner.commitState() || !!capturedOwner.pendingReplacement() || capturedOwner.chapterPending(); }
  function adopt(value?: PublicSession) {
    current(); session = value ? prepareResume(value) : undefined; terms = false;
    savedSignature = value ? signature(snapshot(value.draft ?? {}, value.relationships.files ?? [])) : ''; updateRecovery();
  }
  async function run(action: () => Promise<void>) {
    if (disposed || busy || mutex.isLocked() || disabled) return;
    await mutex.exec(async () => {
      busy = true; message = ''; confirm = undefined;
      try { current(); await action(); current(); }
      catch (error) { if (!disposed) message = error instanceof Error ? error.message : 'Page changes were not confirmed. Staging was kept.'; }
      finally { if (!disposed) { try { updateRecovery(); } catch { recovery = true; } busy = false; } }
    });
  }
  async function inspect() { adopt(await controller.inspect()); ready = true; }
  async function inspectReadOnly() { await inspect(); message = 'Current owned staging was inspected read-only. This does not prove the original admission or its mode; its uncertainty fence was kept.'; }
  async function begin(replace: boolean) { adopt(await controller.begin(replace)); ready = true; message = replace ? 'Empty private staging opened. Published pages are unchanged until Save Pages succeeds.' : 'Original pages restored into private staging. Published pages are unchanged until Save Pages succeeds.'; }
  async function save() {
    if (!session) return; const saved = await controller.save(session); current();
    acknowledge(session, saved); savedSignature = signature(snapshot(saved.draft ?? {}, saved.relationships.files ?? [])); message = 'Private page order saved. Published pages are unchanged.';
  }
  async function commit() {
    if (!session) return; const response = await controller.commit(session, terms); current();
    if (response.body) { oncommit(response.body); adopt(); message = 'Local chapter pages updated. Original signed protocol entries were not changed.'; }
    else { updateRecovery(); message = 'The page save was accepted but its exact readable receipt is not available. Reconcile the saved outcome; do not submit again.'; }
  }
  async function reconcile() {
    if (dirty() && !recovery) { message = 'Save the private page order before reloading staging. No local changes were discarded.'; return; }
    const next = await controller.reconcile(); current();
    if (next?.is_committed && next.receipt) { oncommit(next.receipt); adopt(); message = 'The exact saved chapter receipt was reconciled. No commit was replayed.'; }
    else { adopt(next); message = 'Current owned staging restored. No mutation was replayed.'; }
    ready = true;
  }
  async function cancel() { if (!session) return; await controller.cancel(session); adopt(); message = 'Private staging cancelled. Published chapter pages were kept.'; }
  $effect(() => { onlock(!ready || busy || !!session || recovery); });
  beforeNavigate(navigation => { if (!navigation.willUnload && (busy || dirty())) { navigation.cancel(); message = 'Wait for the current operation and save the private page order before leaving. Staging was kept.'; } });
  onMount(() => {
    const warn = (event: BeforeUnloadEvent) => { if (busy || dirty()) { event.preventDefault(); event.returnValue = ''; } };
    window.addEventListener('beforeunload', warn); void run(inspect);
    return () => window.removeEventListener('beforeunload', warn);
  });
  onDestroy(() => { disposed = true; });
</script>
<section class="space-y-3 rounded border p-4" aria-label="Chapter pages">
  <h3 class="text-lg font-medium">Chapter pages</h3>
  {#if access?.pageSource === 'protocol'}<p>These are original signed protocol pages. Edit Pages and Replace Pages are unavailable; the original is immutable.</p>
  {:else if access?.pageSource !== 'local'}<p>Local page-source eligibility is unavailable. Neither a missing source field nor a cookie session grants page-edit access.</p>
  {:else}<p>Edit the current local pages, or start empty private staging with Replace Pages. Neither action changes published pages before Save Pages.</p>{/if}
  {#if message}<p role="status" class="rounded border p-3">{message}</p>{/if}
  {#if recovery}<p>There is an unresolved signed operation. Writes and cancellation are disabled until its exact receipt is reconciled. No operation will be replayed automatically.</p>{/if}
  {#if foreign && session}
    <p>Another upload already owns staging. It has not been deleted or replaced.</p>
    <a class="underline" href={session.chapter_id ? `/chapter/${session.chapter_id}/edit` : `/title/${session.manga_id}/upload`}>Open the original upload editor</a>
  {:else if session}
    <p>Private session version {session.version}; captured chapter version {session.edit_version}.</p>
    <UploadChapterPages bind:pages={session.relationships.files!} disabled={!editable || session.can_mutate === false} />
    <label class="flex items-center gap-2"><input type="checkbox" bind:checked={terms} disabled={!editable || !session.can_mutate} />I accept the <a class="underline" href="/terms" target="_blank" rel="noopener noreferrer">Terms of Service</a> for this page upload.</label>
    <div class="flex flex-wrap gap-3">
      <button type="button" class="rounded border px-4 py-2" disabled={!editable || !filesReady || !session.can_mutate} onclick={() => run(save)}>Save staged order</button>
      <button type="button" class="rounded border px-4 py-2" disabled={!editable || !filesReady || !terms || !session.can_retry_commit} onclick={() => run(commit)}>Save Pages</button>
      <button type="button" class="rounded border px-4 py-2" disabled={!ready || disabled || busy || $mutex || recovery || (!session.can_cancel && !session.is_deleted)} onclick={() => confirm = 'cancel'}>{session.is_deleted ? 'Acknowledge cancelled staging' : 'Cancel page editing'}</button>
    </div>
  {:else}
    <div class="flex flex-wrap gap-3">
      <button type="button" class="rounded border px-4 py-2" disabled={!editable} onclick={() => run(() => begin(false))}>Edit Pages</button>
      <button type="button" class="rounded border px-4 py-2" disabled={!editable} onclick={() => confirm = 'replace'}>Replace Pages</button>
    </div>
  {/if}
  <button type="button" class="rounded border px-4 py-2" disabled={disabled || busy || $mutex} onclick={() => run(reconcile)}>{recovery ? 'Reconcile saved page outcome' : 'Reload owned staging'}</button>
  {#if recovery && capturedOwner.chapterBeginPending()}<button type="button" class="rounded border px-4 py-2" disabled={disabled || busy || $mutex} onclick={() => run(inspectReadOnly)}>Inspect owned staging (read-only)</button>{/if}
  {#if confirm}
    <div role="alertdialog" tabindex="-1" aria-label={confirm === 'replace' ? 'Confirm replacement staging' : 'Confirm staging cancellation'} class="space-y-3 rounded border p-3">
      <p>{confirm === 'replace' ? 'Start an empty private page-edit session for this exact chapter version? Current published pages remain unchanged until a later successful Save Pages.' : 'Discard this exact private page-edit session? Published chapter pages are kept. Unsaved staging cannot be resumed afterward.'}</p>
      <button type="button" class="rounded border px-4 py-2" disabled={disabled || busy || $mutex || recovery || (confirm === 'replace' && !editable)} onclick={() => run(confirm === 'replace' ? () => begin(true) : cancel)}>{confirm === 'replace' ? 'Start empty staging' : 'Discard this staging'}</button>
      <button type="button" class="rounded border px-4 py-2" onclick={() => confirm = undefined}>Keep current state</button>
    </div>
  {/if}
</section>
