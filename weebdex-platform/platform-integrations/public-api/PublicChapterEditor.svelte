<script lang="ts">
  import PublicChapterPagesEditor from './PublicChapterPagesEditor.svelte';
  import { chapterMetadataPatch } from './language';
  import { onDestroy, onMount, untrack } from 'svelte';
  import { Session } from '$lib/stores';
  import AccountActionGate from './AccountActionGate.svelte';
  import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
  import { loadUploadAccount, publicUploadOwner, type PublicUploadOwner } from './native-upload-client';
  import { normalizeChapter } from './normalize';
  import { publicCatalogueRead } from './site-client';
  import type {ChapterEditAccess} from './chapter-access';
  import { record, resource } from './validation';
  import type { MangaView } from './types';
  let { initial }: { initial: Chapter } = $props();
  const captured = untrack(() => initial);
  const id = captured.id;
  let chapter = $state(captured), title = $state(captured.title ?? ''), volume = $state(captured.volume ?? ''), number = $state(captured.chapter ?? ''), language = $state(captured.language);
  let groups = $state((captured.relationships.groups ?? []).map(group => group.id).join(', '));
  let owner = $state.raw<PublicUploadOwner>(), pagesLocked = $state(true);
  let ready = $state(false), busy = $state(false), pending = $state(false), removed = $state(false), removalChoice = $state<'request-review' | 'approve-hide'>(), message = $state('');
  let access=$state<ChapterEditAccess>(),sourcePending=$state(false);
  let disposed = false,generation=0,capturedSession=untrack(()=>$Session),issue=$state<NativeAccountIssue>();
  const current = () => { if (disposed || $Session!==capturedSession || initial.id !== id) throw Error('The chapter editor changed. Reload the current chapter.'); };
  onDestroy(() => { disposed = true; owner?.dispose(); });
  async function initialize(){
    if(disposed||!$Session?.identity?.id||busy)return;capturedSession=$Session;const session=capturedSession,token=generation;
    const check=()=>{current();if(token!==generation||$Session!==session)throw Error('The chapter account changed.');};
    busy=true;issue=undefined;
    try{const account=await loadUploadAccount(check);check();owner=publicUploadOwner(()=>initial.id,account,'chapter','caller');await refreshAccess();check();pending=owner.chapterPending();sourcePending=false;ready=true;}
    catch(error){if(!disposed&&token===generation){issue=nativeAccountIssue(error);message=error instanceof Error?error.message:'Your account could not be checked.';}}
    finally{if(!disposed&&token===generation)busy=false;}
  }
  $effect(()=>{const session=$Session;untrack(()=>{generation++;owner?.dispose();owner=undefined;capturedSession=session;ready=false;busy=false;pending=false;removed=false;access=undefined;message='';issue=undefined;restore(captured);if(session?.identity?.id)void initialize();});});
  const metadataChanged = $derived(title !== (chapter.title ?? '') || volume !== (chapter.volume ?? '') || number !== (chapter.chapter ?? '') || language !== chapter.language || groups !== (chapter.relationships.groups ?? []).map(group => group.id).join(', '));
  const canUpdate = $derived(access?.canUpdate===true);
  const canClaim = $derived(access?.canClaim===true);
  const approvesHide = $derived(access?.approvesHide===true);
  const canRemove = $derived(access?.canRemove===true);
  async function refreshAccess(){access=undefined;if(!owner)return;try{const next=await owner.chapterAccess(chapter.version);owner.assert();current();access=next;}catch(error){if(!disposed)message=error instanceof Error?error.message:'Chapter eligibility is unavailable. Reload the current chapter.';}}
  function restore(value: Chapter) { chapter = value; title = value.title ?? ''; volume = value.volume ?? ''; number = value.chapter ?? ''; language = value.language; groups = (value.relationships.groups ?? []).map(group => group.id).join(', '); }
  async function command(action: 'update' | 'claim' | 'remove', removalMode?: 'request-review' | 'approve-hide') {
    if (!owner || !ready || busy || pagesLocked || pending || removed || action === 'remove' && !removalMode) return;
    const operationOwner=owner,token=generation;busy = true; message = ''; removalChoice = undefined;
    try {
      await refreshAccess();owner.assert();current();
      if(action==='update'?!canUpdate:action==='claim'?!canClaim:removalMode==='approve-hide'?!approvesHide:!canRemove){message=message||'The current account is not eligible for this chapter action.';return;}
      const payload = action === 'update' ? chapterMetadataPatch(chapter, { title, volume, chapter: number, language, groups: groups.split(',').map(value => value.trim()).filter(Boolean) }) : action === 'remove' ? { removalMode } : undefined;
      const response = await owner.request(`/chapter/${id}${action === 'claim' ? '/claim' : ''}`, action === 'update' ? 'PUT' : action === 'claim' ? 'POST' : 'DELETE', payload);
      owner.assert(); current();
      if (response.status >= 400) { const errors = record(response.body).errors; throw Error(Array.isArray(errors) && typeof errors[0]?.detail === 'string' ? errors[0].detail : `Chapter operation refused (HTTP ${response.status})`); }
      if (action === 'remove') {
        if (response.status === 202 && removalMode === 'request-review') { const receipt = record(response.body); message = `Removal request ${receipt.requestId} is awaiting instance moderator review. The chapter remains visible until approval. No original source deletion was requested.`; }
        else if (response.status === 204 && removalMode === 'approve-hide') { removed = true; message = 'A moderator approved hiding this chapter on this instance. Original source deletion and peer copies have separate policy.'; }
        else throw Error('Chapter removal outcome is unconfirmed.');
      } else {
        if (response.status !== 200) throw Error('Chapter mutation outcome is unconfirmed.');
        const row = resource(record(response.body).data, 'chapter');
        restore({ ...chapter, ...normalizeChapter(row, chapter.relationships.manga as unknown as MangaView) } as unknown as Chapter);
        await refreshAccess();
        message = action === 'claim' ? 'Local catalogue ownership updated. Original signing authority is unchanged.' : 'Instance chapter metadata updated. Original signed metadata and image pages are unchanged.';
      }
    } catch (error) {
      if (!disposed&&token===generation&&owner===operationOwner) { message = error instanceof Error ? error.message : 'Chapter operation failed.'; }
    } finally { if (!disposed&&token===generation&&owner===operationOwner) {pending=owner.chapterPending();sourcePending=owner.chapterPendingPhase()==='local-review-accepted-source-unconfirmed';busy = false;} }
  }
  async function reload() {
    if (!owner || busy || pagesLocked || removed) return;
    const operationOwner=owner,token=generation;busy = true;
    try { const response = await publicCatalogueRead<Chapter>('chapter.get', { id }); owner.assert(); current(); if (response.status !== 200 || !response.body || response.body.id !== id) throw Error('Current chapter is unavailable.'); restore(response.body); pending=owner.chapterPending();sourcePending=owner.chapterPendingPhase()==='local-review-accepted-source-unconfirmed'; await refreshAccess(); if(access)message = pending?'Current chapter reloaded. Its unconfirmed command remains fenced; detail is not a command receipt. No mutation was replayed.':'Current chapter reloaded. No mutation was replayed.'; }
    catch (error) { if (!disposed&&token===generation&&owner===operationOwner) message = error instanceof Error ? error.message : 'Could not reload this chapter.'; }
    finally { if (!disposed&&token===generation&&owner===operationOwner) busy = false; }
  }
</script>
<section class="mx-auto max-w-270 space-y-4 p-5">
  <h2 class="text-xl font-medium">Edit chapter</h2>
  <p>Changes apply to this site. Original publications and their pages cannot be changed. Where page editing is available, prepare your changes privately below.</p>
  {#if message}<p role="status" class="rounded border p-3">{message}</p>{/if}
  <AccountActionGate {issue} onready={initialize} />
  {#if !ready&&$Session?.identity?.id}<button type="button" class="rounded border px-3 py-2" disabled={busy} onclick={initialize}>Check my access again</button>{/if}
  {#if sourcePending}<p>Your review request was accepted here, but its delivery to the original publisher is unconfirmed. Do not submit it again. Further changes are disabled; reloading cannot confirm delivery.</p>{:else if pending}<p>An earlier change could not be confirmed. Do not repeat it. Editing stays disabled even after reloading; contact support to check the result.</p>{/if}
  {#if ready&&!access}<p>Chapter eligibility could not be verified. Reload the current chapter before changing it.</p>{/if}
  {#if access&&!access.claimFactsAvailable}<p>We could not verify whether you can claim this chapter. Your access has not changed.</p>{/if}
  <form class="space-y-3" onsubmit={event => { event.preventDefault(); void command('update'); }}>
    <fieldset disabled={!ready || busy || pagesLocked || pending || removed || !canUpdate} class="space-y-3">
      <label class="block">Title<input class="block w-full rounded border p-2" bind:value={title} maxlength="255" /></label>
      <label class="block">Volume<input class="block w-full rounded border p-2" bind:value={volume} maxlength="32" /></label>
      <label class="block">Chapter<input class="block w-full rounded border p-2" bind:value={number} maxlength="32" /></label>
      <label class="block">Language<input class="block w-full rounded border p-2" bind:value={language} /></label>
      <p class="text-sm">Use a language code, such as en for English or pt-br for Brazilian Portuguese. You can leave an existing value unchanged.</p>
      <label class="block">Group IDs, separated by commas<input class="block w-full rounded border p-2" bind:value={groups} /></label>
      <button type="submit" class="rounded border px-4 py-2">Save changes</button>
    </fieldset>
  </form>
  <div class="flex flex-wrap gap-3">
    <button type="button" class="rounded border px-4 py-2" disabled={!ready || busy || pagesLocked || pending || removed || !canClaim} onclick={() => command('claim')}>Claim local chapter ownership</button>
    <button type="button" class="rounded border px-4 py-2" disabled={!ready || busy || pagesLocked || pending || removed || !canRemove} onclick={() => removalChoice = approvesHide ? 'approve-hide' : 'request-review'}>{approvesHide ? 'Approve instance hide' : 'Request chapter removal'}</button>
    <button type="button" class="rounded border px-4 py-2" disabled={!ready || busy || pagesLocked || removed} onclick={reload}>Reload current chapter</button>
  </div>
  {#if removalChoice}
    <div role="alertdialog" tabindex="-1" aria-label="Confirm chapter removal" class="space-y-3 rounded border p-4">
      <p>{removalChoice === 'approve-hide' ? 'Hide this chapter on this site now? This does not remove the original publication or copies on other sites.' : 'Ask a moderator to remove this chapter from this site? It stays visible until approval. Removing the original publication requires a separate review.'}</p>
      <button type="button" class="rounded border px-4 py-2" disabled={!ready||busy||pagesLocked||pending||removed||!canRemove} onclick={() => command('remove', removalChoice)}>{removalChoice === 'approve-hide' ? 'Approve and hide on this instance' : 'Submit removal request'}</button>
      <button type="button" class="rounded border px-4 py-2" onclick={() => removalChoice = undefined}>Cancel</button>
    </div>
  {/if}
  {#if owner && ready && !removed}
    {#if metadataChanged}<p>Save your chapter details or reload before editing pages. Saving pages does not save unsaved chapter details.</p>{/if}
    <PublicChapterPagesEditor {chapter} {owner} {access} disabled={busy || pending || metadataChanged} onlock={locked => pagesLocked = locked} oncommit={value => { restore(value); void refreshAccess(); }} />
  {/if}
  <a class="underline" href={`/title/${chapter.relationships.manga.id}`}>Return to the title</a>
</section>
