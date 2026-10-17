<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { loadUploadAccount, publicUploadOwner, type PublicUploadOwner } from './upload-client';
  import { normalizeChapter } from './normalize';
  import { publicCatalogueRead } from './site-client';
  import { record, resource } from './validation';
  import type { MangaView } from './types';
  let { initial }: { initial: Chapter } = $props();
  const captured = untrack(() => initial);
  const id = captured.id;
  let chapter = $state(captured), title = $state(captured.title ?? ''), volume = $state(captured.volume ?? ''), number = $state(captured.chapter ?? ''), language = $state(captured.language);
  let groups = $state((captured.relationships.groups ?? []).map(group => group.id).join(', '));
  let owner: PublicUploadOwner | undefined, user = $state<User>(), ready = $state(false), busy = $state(false), pending = $state(false), removed = $state(false), confirmRemoval = $state(false), message = $state('');
  let disposed = false;
  const current = () => { if (disposed || initial.id !== id) throw Error('The chapter editor changed. Reload the current chapter.'); };
  onDestroy(() => { disposed = true; owner?.dispose(); });
  onMount(() => { void (async () => {
    try { const account = await loadUploadAccount(current); current(); user = account; owner = publicUploadOwner(() => initial.id, account, 'chapter'); ready = true; }
    catch (error) { if (!disposed) message = error instanceof Error ? error.message : 'Select a signing identity in Settings and reload this editor.'; }
  })(); });
  const staff = $derived(!!user?.roles?.some(role => ['moderator', 'staff', 'sysop'].includes(role)));
  const canUpdate = $derived(!!user?.permissions?.includes('self.chapter.update') || staff && !!user?.permissions?.includes('chapter.update'));
  const canClaim = $derived(!!user?.permissions?.includes('self.chapter.update'));
  const approvesHide = $derived(!!user?.roles?.some(role => ['moderator', 'staff', 'sysop'].includes(role)) && !!user?.permissions?.includes('chapter.remove'));
  const canRemove = $derived(!!user?.permissions?.includes('self.chapter.remove') || approvesHide);
  function restore(value: Chapter) { chapter = value; title = value.title ?? ''; volume = value.volume ?? ''; number = value.chapter ?? ''; language = value.language; groups = (value.relationships.groups ?? []).map(group => group.id).join(', '); }
  async function command(action: 'update' | 'claim' | 'remove') {
    if (!owner || busy || pending || removed) return;
    busy = true; message = ''; confirmRemoval = false;
    try {
      const payload = action === 'update' ? { version: chapter.version, title, volume, chapter: number, language, groups: groups.split(',').map(value => value.trim()).filter(Boolean) } : undefined;
      const response = await owner.request(`/chapter/${id}${action === 'claim' ? '/claim' : ''}`, action === 'update' ? 'PUT' : action === 'claim' ? 'POST' : 'DELETE', payload);
      owner.assert(); current();
      if (response.status >= 400) { const errors = record(response.body).errors; throw Error(Array.isArray(errors) && typeof errors[0]?.detail === 'string' ? errors[0].detail : `Chapter operation refused (HTTP ${response.status})`); }
      if (action === 'remove') {
        if (response.status === 202) { const receipt = record(response.body); message = `Removal request ${receipt.requestId} is awaiting instance moderator review. The chapter remains visible until approval. Original source request status: ${receipt.sourceDeletionStatus ?? 'unconfirmed'}.`; }
        else if (response.status === 204) { removed = true; message = 'A moderator approved hiding this chapter on this instance. Original source deletion and peer copies have separate policy.'; }
        else throw Error('Chapter removal outcome is unconfirmed.');
      } else {
        if (response.status !== 200) throw Error('Chapter mutation outcome is unconfirmed.');
        const row = resource(record(response.body).data, 'chapter');
        restore({ ...chapter, ...normalizeChapter(row, chapter.relationships.manga as unknown as MangaView) } as unknown as Chapter);
        message = action === 'claim' ? 'Local catalogue ownership updated. Original signing authority is unchanged.' : 'Instance chapter metadata updated. Original signed metadata and image pages are unchanged.';
      }
    } catch (error) {
      if (!disposed) { message = error instanceof Error ? error.message : 'Chapter operation failed.'; pending = true; }
    } finally { if (!disposed) busy = false; }
  }
  async function reload() {
    if (!owner || busy || removed) return;
    busy = true;
    try { const response = await publicCatalogueRead<Chapter>('chapter.get', { id }); owner.assert(); current(); if (response.status !== 200 || !response.body || response.body.id !== id) throw Error('Current chapter is unavailable.'); restore(response.body); pending = false; message = 'Current chapter reloaded. No mutation was replayed.'; }
    catch (error) { if (!disposed) message = error instanceof Error ? error.message : 'Could not reload this chapter.'; }
    finally { if (!disposed) busy = false; }
  }
</script>
<section class="mx-auto max-w-270 space-y-4 p-5">
  <h2 class="text-xl font-medium">Edit chapter</h2>
  <p>Changes are instance-local catalogue annotations. Signed source metadata and published image pages are immutable.</p>
  {#if message}<p role="status" class="rounded border p-3">{message}</p>{/if}
  {#if !ready}<p>Select a signing identity in Settings and open this page again if account authorization is required.</p>{/if}
  <form class="space-y-3" onsubmit={event => { event.preventDefault(); void command('update'); }}>
    <fieldset disabled={!ready || busy || pending || removed || !canUpdate} class="space-y-3">
      <label class="block">Title<input class="block w-full rounded border p-2" bind:value={title} maxlength="255" /></label>
      <label class="block">Volume<input class="block w-full rounded border p-2" bind:value={volume} maxlength="32" /></label>
      <label class="block">Chapter<input class="block w-full rounded border p-2" bind:value={number} maxlength="32" /></label>
      <label class="block">Language<input class="block w-full rounded border p-2" bind:value={language} maxlength="12" /></label>
      <label class="block">Group UUIDs, comma separated<input class="block w-full rounded border p-2" bind:value={groups} /></label>
      <button type="submit" class="rounded border px-4 py-2">Save instance metadata</button>
    </fieldset>
  </form>
  <div class="flex flex-wrap gap-3">
    <button type="button" class="rounded border px-4 py-2" disabled={!ready || busy || pending || removed || !canClaim} onclick={() => command('claim')}>Claim local chapter ownership</button>
    <button type="button" class="rounded border px-4 py-2" disabled={!ready || busy || pending || removed || !canRemove} onclick={() => confirmRemoval = true}>{approvesHide ? 'Approve instance hide' : 'Request chapter removal'}</button>
    <button type="button" class="rounded border px-4 py-2" disabled={!ready || busy || removed} onclick={reload}>Reload current chapter</button>
  </div>
  {#if confirmRemoval}
    <div role="alertdialog" tabindex="-1" aria-label="Confirm chapter removal" class="space-y-3 rounded border p-4">
      <p>{approvesHide ? 'Approve hiding this chapter on this instance immediately? This action uses your moderator chapter removal grant. Original protocol deletion and peer copies remain separate.' : 'Request instance moderator review of this chapter removal? The chapter remains visible until approval. Original protocol removal is reviewed separately.'}</p>
      <button type="button" class="rounded border px-4 py-2" onclick={() => command('remove')}>{approvesHide ? 'Approve and hide on this instance' : 'Submit removal request'}</button>
      <button type="button" class="rounded border px-4 py-2" onclick={() => confirmRemoval = false}>Cancel</button>
    </div>
  {/if}
  <a class="underline" href={`/title/${chapter.relationships.manga.id}`}>Return to the title</a>
</section>
