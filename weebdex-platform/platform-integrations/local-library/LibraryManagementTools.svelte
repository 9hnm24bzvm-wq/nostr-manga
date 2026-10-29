<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { createRouteActionOwner } from '$platform/route-action-owner';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { localLibraryState, libraryIdentity, openLibrary, useMode, verifyCatalogueScope, message, currentDeviceSnapshot, downloadBlob, createList, previewListName, renameList, type DeviceListNameReview, editListMember, saveSavedSeries, removeLocalRecord, captureLibraryContext, assertLibraryContext } from './library';
  import { loadDeviceListTitles } from './device-list-titles';
  import { orderedListMembers } from './device-list-members';
  import { sameScope, statuses } from './device-store';
  import { recordKey } from './protocol/codec.js';
  import type { Counts, LibraryRecord } from './protocol/types.d.ts';
  import { connectSigner, disconnectSigner, signerIdentity, capabilities, relayPreferences, rememberRelay, selectRelays, forgetRelay, admitAccount, previewPublication, publishReviewed, publicationJobs, retryPublication, cancelPublication, removePublication, previewServiceAdoption, adoptServiceIdentity, type ServiceAdoptionReview, previewRecovery, applyRecovery, exportCanonicalBackup, exportLegacyCanonicalBackup, canonicalRecords, removeCanonicalRecord, previewCanonicalImport, applyCanonicalImport, type CanonicalImportReview, type CanonicalSelection, type Relay, type PublicationReview, type RecoveryReview, type Capabilities } from './publication';
  const { view, onTask, assertViewCurrent = () => {} }: { view: 'lists' | 'sync' | 'destinations' | 'publish' | 'public-recovery' | 'attempts' | 'recovery'; onTask: (view: 'sync' | 'destinations' | 'publish' | 'public-recovery' | 'attempts') => void; assertViewCurrent?: () => void } = $props();
  let showScope = $state(false), showAdoption = $state(false), showCanonical = $state(false), showKnownKey = $state(false), inspectedRelay = $state(''), inspectedJob = $state('');
  let titleNames = $state<Record<string,string>>({}), titlePage = $state(0), titlesLoading = $state(false), titleError = $state('');
  let titleEpoch = 0;
  const savedTitle = (id: string | undefined) => id && titleNames[id] || 'Unavailable title';
  async function loadTitleNames(page = 0) {
    const epoch = ++titleEpoch, context = captureLibraryContext();
    titlePage = page; titlesLoading = true; titleError = '';
    const current = () => { assertActive(); assertLibraryContext(context); if(epoch !== titleEpoch || view !== 'lists') throw Error('Saved title selection changed'); };
    try {
      const ids = series.slice(page * 100, (page + 1) * 100).map(row=>row.id!);
      const names = ids.length ? await loadDeviceListTitles(ids, current) : {};
      current(); titleNames = Object.fromEntries(Object.entries({...titleNames,...names}).slice(-1000));
    } catch(cause) { if(!canonicalDisposed && epoch===titleEpoch) titleError = 'Title names could not be loaded. Your saved titles are unchanged.'; }
    finally { if(!canonicalDisposed && epoch===titleEpoch) titlesLoading = false; }
  }
  async function changeMember(list: string, id: string, direction: -1 | 0 | 1) {
    const context = captureLibraryContext();
    await run('member', async () => { await editListMember(list,id,direction,()=>{assertActive();assertLibraryContext(context);}); notice = direction ? 'List order updated.' : 'Title added to your list.'; });
  }
  const queueState = (state: string) => ({ prepared: 'Ready to send', pending: 'Waiting to send', sending: 'Sending', retryable: 'Ready to retry', refused: 'Not sent', accepted: 'Sent', superseded: 'Replaced by a newer snapshot', outcome_unknown: 'Delivery needs checking', projection_pending: 'Updating', cancelled: 'Cancelled' }[state] ?? 'Needs attention');
  let notice = $state('Changes stay local unless you review and publish a public snapshot.'), error = $state(''), pending = $state('');
  let relays = $state<Relay[]>([]), relayRevision = $state(0), jobs = $state<any[]>([]);
  let policy = $state<Capabilities>(), relayOrigin = $state(''), serviceKey = $state('');
  let publicConsent = $state(false), publication = $state<PublicationReview>(), recovery = $state<RecoveryReview>();
  let listName = $state(''), memberList = $state(''), memberSeries = $state(''), scopeText = $state('');
  let listNameReview = $state.raw<DeviceListNameReview>();
  let renamedListName = $state('');
  let listNameContext: ReturnType<typeof captureLibraryContext> | undefined;
  let adoption = $state<ServiceAdoptionReview>(), adoptionPage = $state(0);
  let canonicalFamily = $state('series'), canonicalRows = $state<LibraryRecord[]>([]), canonicalCounts = $state<Partial<Counts>>({}), canonicalAfter = $state<string | null>(null), canonicalImport = $state<CanonicalImportReview>();
  let canonicalSelection = $state<CanonicalSelection>();
  let refreshEpoch = 0;
  let canonicalGeneration = 0, canonicalLoadedFamily = '', canonicalDisposed = false;
  function assertActive() { assertViewCurrent(); if (canonicalDisposed) throw Error('Library settings were closed. Reopen them and review the action again.'); }
  function invalidateCanonical() { canonicalSelection = undefined; canonicalGeneration++; canonicalLoadedFamily = ''; canonicalRows = []; canonicalCounts = {}; canonicalAfter = null; }
  async function canonicalFamilyChanged() { invalidateCanonical(); await run('canonical-read', () => loadCanonical()); }
  async function loadCanonical(more = false) {
    const family = canonicalFamily, generation = ++canonicalGeneration;
    if (more && canonicalLoadedFamily !== family) throw Error('Canonical family changed; refresh before loading more');
    const previous = more ? [...canonicalRows] : [], after = more ? canonicalAfter : null;
    if (!more) { canonicalLoadedFamily = ''; canonicalRows = []; canonicalCounts = {}; canonicalAfter = null; }
    const result = await canonicalRecords(family, after);
    if (canonicalDisposed || generation !== canonicalGeneration || family !== canonicalFamily || result.owner !== $signerIdentity.owner || result.generation !== $signerIdentity.generation) return;
    canonicalSelection = { owner: result.owner, generation: result.generation }; canonicalLoadedFamily = family; canonicalRows = [...previous, ...result.records]; canonicalCounts = result.counts;
    const last = result.records.at(-1); canonicalAfter = last ? recordKey(family, last) : after;
  }
  async function removeCanonicalRow(row: LibraryRecord) {
    const family = canonicalLoadedFamily, key = recordKey(family, row), selected = canonicalSelection;
    if (!selected || !family || family !== canonicalFamily || !canonicalRows.includes(row)) throw Error('Canonical selection changed; refresh and review the current record');
    if (!confirm('Remove this local canonical record? Already published snapshots remain public.')) return;
    await run('canonical-remove', async () => {
      if (canonicalDisposed || family !== canonicalFamily || canonicalLoadedFamily !== family || !canonicalRows.includes(row)) throw Error('Canonical selection changed before removal; no record removed');
      // This captured family/key is never reinterpreted after an await.
      await removeCanonicalRecord(family, key, selected); await loadCanonical();
    });
  }
  function currentCanonicalReview(review: CanonicalSelection) { return !canonicalDisposed && review.owner === $signerIdentity.owner && review.generation === $signerIdentity.generation; }
  async function installCanonicalReview<T extends CanonicalSelection>(operation: Promise<T>, install: (review: T) => void) {
    const epoch = canonicalGeneration, review = await operation;
    if (epoch === canonicalGeneration && currentCanonicalReview(review)) install(review);
  }
  async function canonicalExport() {
    const selected = { owner: $signerIdentity.owner, generation: $signerIdentity.generation }, blob = await exportCanonicalBackup();
    if (!selected.owner || !currentCanonicalReview({ owner: selected.owner, generation: selected.generation })) throw Error('Canonical export identity changed');
    downloadBlob(blob, 'manga-canonical-library.jsonl');
  }
  async function canonicalFileChanged(event: Event) { const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = ''; if (file) await run('canonical-import-preview', () => installCanonicalReview(previewCanonicalImport(file), review => { canonicalImport = review; })); }
  const lists = $derived($localLibraryState.records.filter(row => row.family === 'lists'));
  const series = $derived($localLibraryState.records.filter(row => row.family === 'series'));
  async function refresh() {
    const epoch = ++refreshEpoch, selected = { owner: $signerIdentity.owner, generation: $signerIdentity.generation };
    const current = () => !canonicalDisposed && epoch === refreshEpoch && selected.owner === $signerIdentity.owner && selected.generation === $signerIdentity.generation;
    const preferences = await relayPreferences(); if (!current()) return; relays = preferences.relays; relayRevision = preferences.revision;
    const savedJobs = await publicationJobs(); if (!current()) return; jobs = savedJobs;
    const snapshot = await currentDeviceSnapshot(); if (!current()) return;
    scopeText = `${snapshot.scope.origin} · namespace ${snapshot.scope.catalogNamespace ?? 'origin-only'} · ${snapshot.scope.serviceKey ?? 'origin-only scope; service identity not adopted'}`;
  }
  async function run(name: string, operation: () => Promise<void>) {
    if (pending || canonicalDisposed) return; pending = name; error = '';
    try { assertActive(); await operation(); }
    catch (cause) { if (!canonicalDisposed) error = message(cause); }
    finally { if (!canonicalDisposed) { await refresh().catch(cause => { if (!canonicalDisposed) error ||= message(cause); }); if (!canonicalDisposed) pending = ''; } }
  }
  async function cancel(id: string) { try { await cancelPublication(id); await refresh(); notice = 'Further sending cancelled. Already sent copies cannot be recalled.'; } catch (cause) { error = message(cause); } }
  function resetReviews() { publication = undefined; publicConsent = false; recovery = undefined; }
  $effect(() => { const owner = $signerIdentity.owner, generation = $signerIdentity.generation; untrack(() => { invalidateCanonical(); canonicalImport = undefined; resetReviews(); jobs = []; relays = []; relayRevision = 0; if (!canonicalDisposed) void refresh().catch(cause => { if (!canonicalDisposed) error = message(cause); }); }); });
  const listRouteOwner = createRouteActionOwner();
  $effect(() => { const ready = $localLibraryState.ready, revision = $localLibraryState.revision, scope = $localLibraryState.scopeStatus;
    if(view === 'lists' && ready) untrack(()=>{titleNames={};void loadTitleNames(titlePage);});
  });
  async function openDeviceList(event: MouseEvent, id: string) {
    event.preventDefault();
    const route = listRouteOwner.capture(), listId = id;
    await run('open-device-list', async () => {
      if (!route.alive()) return; assertActive();
      const context = captureLibraryContext(), saved = await currentDeviceSnapshot();
      if (!route.alive()) return; assertActive();
      assertLibraryContext(context);
      if (!saved.records.some(row => row.family === 'lists' && row.id === listId)) throw Error('This list no longer belongs to the current saved-device scope');
      assertActive(); await useMode('device');
      if (!route.alive()) return; assertActive();
      resetReviews();
      const current = await currentDeviceSnapshot();
      if (!route.alive()) return; assertActive();
      if ($localLibraryState.mode !== 'device' || !sameScope(saved.scope, current.scope) || !current.records.some(row => row.family === 'lists' && row.id === listId)) throw Error('The device list or selected library changed. Reopen the current saved list.');
      notice = 'Opening this custom device list in device-library mode. Account follows are preserved separately.';
      assertActive(); await goto('/library?list=' + encodeURIComponent(listId));
    });
  }
  function cancelListRename() { listNameReview = undefined; listNameContext = undefined; renamedListName = ''; }
  async function beginListRename(id: string) {
    const route = listRouteOwner.capture(), context = captureLibraryContext();
    const current = () => { if (!route.alive() || view !== 'lists') throw Error('The list view changed. Reopen Rename.'); assertActive(); assertLibraryContext(context); };
    await run('rename-start', async () => { current(); const review = await previewListName(id, current); current(); listNameContext = context; renamedListName = review.name; listNameReview = review; });
  }
  async function saveListName() {
    const review = listNameReview, context = listNameContext, name = renamedListName;
    if (!review || !context) return;
    const current = () => { assertActive(); assertLibraryContext(context); if (view !== 'lists' || listNameReview !== review) throw Error('The selected list edit changed. Reopen Rename.'); };
    await run('rename-list', async () => { current(); const result = await renameList(review, name, current); current(); cancelListRename(); notice = result.changed ? 'List renamed on this device.' : 'List name unchanged.'; });
  }
  $effect(() => {
    if (publication && (publication.deviceRevision !== $localLibraryState.revision || publication.owner !== $signerIdentity.owner || publication.generation !== $signerIdentity.generation)) { publication = undefined; publicConsent = false; }
  });
  onMount(() => {
    void run('open', async () => { await openLibrary(); if (view === 'recovery' || view === 'destinations') void capabilities().then(result => { if (!canonicalDisposed) policy = result; }).catch(() => {}); });
    return () => { canonicalDisposed = true; canonicalGeneration++; resetReviews(); canonicalImport = undefined; adoption = undefined; };
  });
</script>
<section class="space-y-5" data-library-tool={view}>
{#if view === 'lists'}
  <p class="text-muted-foreground text-sm">Organize your saved titles into lists. Changes stay on this device.</p>
      <form class="my-2 flex flex-wrap gap-2" onsubmit={event => { event.preventDefault(); void run('create-list', async () => { memberList = await createList(listName); listName = ''; notice = 'List created on this device.'; }); }}>
        <label for="device-list-name">List name <Input id="device-list-name" class="mt-1" bind:value={listName} maxlength={128} required /></label>
        <Button type="submit" disabled={!!pending}>Create list</Button>
      </form>
      <form class="my-2 flex flex-wrap gap-2" onsubmit={event => { event.preventDefault(); void changeMember(memberList, memberSeries, 0); }}>
        <label>List <select class="rounded border px-2" bind:value={memberList} required><option value="">Choose list</option>{#each lists as list}<option value={list.id}>{list.name}</option>{/each}</select></label>
        <label>Title <select class="rounded border px-2" bind:value={memberSeries} required><option value="">Choose a saved title</option>{#each series.slice(titlePage * 100, (titlePage + 1) * 100) as row}<option value={row.id}>{savedTitle(row.id)} ({row.status?.replaceAll('_', ' ')})</option>{/each}</select></label>
        <Button type="submit" disabled={!!pending || !memberList || !memberSeries}>Add to list</Button>
      </form>
      {#if titlesLoading}<p role="status">Loading title names…</p>{/if}
      {#if titleError}<p role="status">{titleError}</p><Button variant="outline" onclick={()=>loadTitleNames(titlePage)}>Retry title names</Button>{/if}
      {#if series.length > 100}<div class="flex gap-2"><Button variant="outline" disabled={titlesLoading || titlePage===0} onclick={()=>{memberSeries='';void loadTitleNames(titlePage-1);}}>Previous titles</Button><Button variant="outline" disabled={titlesLoading || (titlePage+1)*100>=series.length} onclick={()=>{memberSeries='';void loadTitleNames(titlePage+1);}}>More titles</Button></div>{/if}
      {#each lists as list}
        <div class="my-1 flex flex-wrap items-center gap-2"><a class="underline" href={'/library?list=' + list.id} onclick={event => openDeviceList(event, list.id!)}>{list.name}</a><Button variant="ghost" disabled={!!pending} onclick={() => beginListRename(list.id!)}>Rename</Button><Button variant="ghost" disabled={!!pending} onclick={() => { if (confirm('Remove this local list and its memberships?')) void run('remove-list', async () => { await removeLocalRecord('lists', list.key); }); }}>Remove locally</Button></div>
        {#if listNameReview && listNameReview.id === list.id}
          <form class="my-2 flex flex-wrap items-end gap-2" onsubmit={event => { event.preventDefault(); void saveListName(); }}>
            <label for={'rename-list-' + list.id}>List name<Input id={'rename-list-' + list.id} class="mt-1" bind:value={renamedListName} maxlength={128} required disabled={!!pending} /></label>
            <Button type="submit" disabled={!!pending || !renamedListName.trim() || $localLibraryState.revision !== listNameReview.revision}>Save name</Button><Button variant="ghost" disabled={!!pending} onclick={cancelListRename}>Cancel</Button>
            {#if $localLibraryState.revision !== listNameReview.revision}<p role="status" class="w-full text-sm">The device library changed. Cancel and reopen Rename before saving.</p>{/if}
          </form>
        {/if}
        {#each orderedListMembers($localLibraryState.records, list.id!) as member, index}
          <div class="ml-3 flex flex-wrap items-center gap-2 text-sm"><span>{index + 1}. {savedTitle(member.series)}</span>
            <Button variant="ghost" disabled={!!pending || index===0} aria-label={'Move '+savedTitle(member.series)+' up'} onclick={()=>changeMember(list.id!,member.series!,-1)}>Move up</Button>
            <Button variant="ghost" disabled={!!pending || index===orderedListMembers($localLibraryState.records,list.id!).length-1} aria-label={'Move '+savedTitle(member.series)+' down'} onclick={()=>changeMember(list.id!,member.series!,1)}>Move down</Button>
            <Button variant="ghost" disabled={!!pending} onclick={() => run('remove-member', async () => { await removeLocalRecord('members', member.key); })}>Remove from list</Button></div>
        {/each}
      {/each}

{:else if view === 'sync'}
  <p class="text-muted-foreground text-sm">Public sync is optional. Nothing is published automatically.</p>
  <div class="space-y-1"><strong class="block font-semibold">Destinations</strong><p class="text-muted-foreground text-sm">{relays.filter(row => row.selected).length} selected instances. Choose where a reviewed library may be sent.</p></div>
  <Button variant="outline" onclick={() => onTask('destinations')}>Choose destinations</Button>
  <div class="space-y-1 border-t pt-4"><strong class="block font-semibold">Publish or restore</strong><p class="text-muted-foreground text-sm">Review a public copy before sending, or restore one you already published.</p></div>
  <div class="flex flex-wrap gap-2"><Button variant="outline" onclick={() => onTask('publish')}>Publish library</Button><Button variant="outline" onclick={() => onTask('public-recovery')}>Recover public library</Button></div>
  {#if jobs.length}<div class="space-y-1 border-t pt-4"><strong class="block font-semibold">Saved attempts</strong><p class="text-muted-foreground text-sm">Check delivery or retry a previously approved snapshot.</p></div><Button variant="outline" onclick={() => onTask('attempts')}>Saved sync attempts</Button>{/if}
{:else if view === 'destinations'}
<Button variant="ghost" size="sm" onclick={() => onTask('sync')}>Back to public sync</Button>        <p class="text-sm">{$signerIdentity.owner ? 'Signing identity connected' : 'Connect a signing identity to publish or recover a public library.'}</p>
        <Button variant="outline" disabled={!!pending} onclick={() => run('connect-signer', async () => { resetReviews(); await connectSigner(assertActive); notice = 'Signer connected. No library data sent.'; })}>Connect external browser signer</Button>
        <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => { disconnectSigner(); resetReviews(); }}>Disconnect signer</Button>
        <p class="text-sm">Available sharing service: {policy?.available ? policy.origin : 'not configured'}. Each destination must accept your identity before you can publish there.</p>
        <form class="flex flex-wrap gap-2" onsubmit={event => { event.preventDefault(); if (confirm(`Verify and remember ${relayOrigin} with expected service key ${serviceKey || 'identity discovered by signed proof'}? This does not send your library.`)) void run('remember-relay', async () => { await rememberRelay(relayOrigin, serviceKey.trim(), relayRevision); resetReviews(); notice = 'Verified instance remembered, initially unselected.'; }); }}>
          <label for="sync-instance-origin">Instance origin <Input id="sync-instance-origin" class="mt-1" type="url" bind:value={relayOrigin} placeholder="https://instance.example" required /></label>
          <Button type="button" variant="ghost" onclick={() => { showKnownKey = !showKnownKey; }}>Use a known service key</Button>{#if showKnownKey}<label for="sync-instance-key">Expected service public key <Input id="sync-instance-key" class="mt-1" bind:value={serviceKey} placeholder="64 lowercase hex characters" /><span class="text-muted-foreground text-sm">Optional. Use this only when you already know and want to verify the instance’s exact public identity.</span></label>{/if}
          <Button type="submit" disabled={!!pending}>Verify and remember instance</Button>
        </form>
        {#each relays as relay}
          <div class="rounded border p-2">
            <label><input type="checkbox" checked={relay.selected} disabled={!!pending} onchange={event => run('select-relay', async () => { await selectRelays(relays.map(row => row.origin === relay.origin ? { ...row, selected: (event.target as HTMLInputElement).checked } : row), relayRevision); resetReviews(); })} /> Select {relay.origin}</label>
            <Button variant="ghost" size="sm" onclick={() => { inspectedRelay = inspectedRelay === relay.origin ? '' : relay.origin; }}>Check service identity</Button>{#if inspectedRelay === relay.origin}<p class="break-all text-xs">Verified service public key: {relay.serviceKey}</p>{/if}
            <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => { if (confirm(`Request admission for public key ${$signerIdentity.owner} at ${relay.origin}? This does not publish the library.`)) void run('enroll', async () => { await admitAccount(relay, assertActive); notice = 'Public-key account admitted. Library remains local.'; }); }}>Request public-key account admission</Button>

            <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Forget this instance and cancel unfinished snapshots sent to it? Published copies remain public.')) void run('forget', async () => { await forgetRelay(relay.origin, relayRevision); resetReviews(); }); }}>Forget instance</Button>
          </div>
        {/each}

{:else if view === 'publish'}
<Button variant="ghost" size="sm" onclick={() => onTask('sync')}>Back to public sync</Button><p>Publishing makes reading status, chapter progress, list names and memberships public at the destinations you approve.</p>        <p class="text-sm">{$signerIdentity.owner ? 'Signing identity connected' : 'Connect a signing identity to publish or recover a public library.'}</p>
        <Button variant="outline" disabled={!!pending} onclick={() => run('connect-signer', async () => { resetReviews(); await connectSigner(assertActive); notice = 'Signer connected. No library data sent.'; })}>Connect external browser signer</Button>
        <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => { disconnectSigner(); resetReviews(); }}>Disconnect signer</Button>
        <Button disabled={!!pending || !$signerIdentity.owner} onclick={() => run('publish-preview', async () => { publicConsent = false; await installCanonicalReview(previewPublication(), review => { publication = review; }); })}>Preview public snapshot</Button>
        {#if publication}
          <div class="rounded border p-2">
            <p>This copy includes {publication.graph.records} saved library items. {publication.excluded.length} entries cannot be shared because they are site-only, hidden, or could not be matched.</p>
            <p class="break-all">Destinations: {publication.recipients.map(row => row.origin).join(', ')}</p>
            {#if publication.excluded.length}<details><summary>Unshared entry references for support</summary><p class="break-all text-xs">{publication.excluded.slice(0, 30).join(', ')}{publication.excluded.length > 30 ? '…' : ''}</p></details>{/if}
            <label><input type="checkbox" bind:checked={publicConsent} /> I understand that this reviewed snapshot becomes public at these recipients.</label>
            <Button disabled={!!pending || !publicConsent} onclick={() => run('publish', async () => { const reviewed = $state.snapshot(publication!); publicConsent = false; publication = undefined; await publishReviewed(reviewed, true, assertActive); notice = 'Publication attempt finished. Review every instance receipt below.'; })}>Sign and publish reviewed snapshot</Button>
            <Button variant="outline" disabled={!!pending} onclick={resetReviews}>Cancel preview</Button>
          </div>
        {/if}

{:else if view === 'public-recovery'}
<Button variant="ghost" size="sm" onclick={() => onTask('sync')}>Back to public sync</Button>        <p class="text-sm">{$signerIdentity.owner ? 'Signing identity connected' : 'Connect a signing identity to publish or recover a public library.'}</p>
        <Button variant="outline" disabled={!!pending} onclick={() => run('connect-signer', async () => { resetReviews(); await connectSigner(assertActive); notice = 'Signer connected. No library data sent.'; })}>Connect external browser signer</Button>
        <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => { disconnectSigner(); resetReviews(); }}>Disconnect signer</Button>
<p class="text-muted-foreground text-sm">Choose a verified instance to preview the public library saved under this signing identity.</p>
  {#if !relays.length}<Button variant="outline" onclick={() => onTask('destinations')}>Choose destinations</Button>{/if}
  {#each relays as relay}<div class="space-y-2 border-t pt-4"><p>{relay.origin}</p>            <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => run('recover-preview', async () => { await installCanonicalReview(previewRecovery(relay, assertActive), review => { recovery = review; }); })}>Preview verified public recovery</Button></div>{/each}
        {#if recovery}
          <div class="rounded border p-2">
            <p class="break-all">Restore your public library from {recovery.relay.origin}, saved {new Date(recovery.event.created_at * 1000).toLocaleString()}. This replaces the portable library for your selected identity. Your separate on-device library is kept.</p>
            <Button disabled={!!pending} onclick={() => { if (confirm('Replace this identity’s portable library with the reviewed public copy? Export a backup first.')) void run('apply-recovery', async () => { await applyRecovery(recovery!); recovery = undefined; notice = 'Library restored on this device. Nothing is published automatically.'; }); }}>Restore reviewed library</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { recovery = undefined; }}>Cancel recovery</Button>
          </div>
        {/if}
{:else if view === 'attempts'}
<Button variant="ghost" size="sm" onclick={() => onTask('sync')}>Back to public sync</Button><p class="text-muted-foreground text-sm">These are exact previously approved snapshots. Retrying never includes newer device changes.</p>        {#each jobs as job}
          <div class="space-y-2 border-t pt-4" data-library-job-id={job.id}>
            <p class="font-medium">Saved public snapshot · {new Date(job.event.created_at * 1000).toLocaleString()}</p><Button variant="ghost" size="sm" onclick={() => { inspectedJob = inspectedJob === job.id ? '' : job.id; }}>Inspect saved snapshot</Button>{#if inspectedJob === job.id}<p class="break-all text-xs">Signed event {job.id} · device revision {job.deviceRevision ?? 'protocol only'} · signer {job.event.pubkey}</p>{/if}
            {#each job.recipients as recipient}<p>{recipient.origin}: {queueState(recipient.state)} · {recipient.message}</p>{/each}
            {#if !job.cancelled && job.recipients.some((row: { state: string }) => !['accepted', 'superseded'].includes(row.state))}
              <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Retry this previously approved public snapshot to its original recipients? New device changes are not included.')) void run('retry', async () => { await retryPublication(job.id, assertActive); notice = 'Retry finished; review receipts.'; }); }}>Retry approved snapshot</Button>
              <Button variant="outline" onclick={() => cancel(job.id)}>Cancel further sending</Button>
            {:else}
              <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Remove this local retry record? Published copies are unaffected.')) void run('remove-job', async () => { await removePublication(job.id); }); }}>Remove local retry record</Button>
            {/if}
          </div>
        {/each}

{:else}
<div class="space-y-3"><strong class="block font-semibold">Check this library</strong><p class="text-muted-foreground text-sm">Use these tools if saved titles are missing, a backup belongs to another site, or support asks you to check the library identity.</p><p role="status">{$localLibraryState.scopeStatus === 'verified' ? 'This library matches the current catalogue.' : 'Check this library before recovering saved titles. Your saved data is kept unchanged.'}</p><Button variant="outline" disabled={!!pending} onclick={() => run('verify-scope', async () => { await verifyCatalogueScope(); })}>Verify current catalogue scope</Button><Button variant="ghost" onclick={() => { showScope = !showScope; }}>View saved catalogue details</Button>{#if showScope}<p class="break-all text-xs">{scopeText}</p><p class="text-xs">{$localLibraryState.scopeStatus}. {$localLibraryState.scopeReason ?? ''}</p>{/if}</div>
<section class="space-y-3 border-t pt-4"><strong class="block font-semibold">Review source identity</strong>        {#if policy?.service_key && ($localLibraryState.scopeStatus === 'origin_only' || scopeText.includes('origin-only'))}
          <Button variant="outline" disabled={!!pending} onclick={() => run('adoption-preview', async () => { adoption = await previewServiceAdoption(); adoptionPage = 0; })}>Preview source identity adoption</Button>
        {/if}
        {#if adoption}
          <div class="rounded border p-2 space-y-2">
            <p class="break-all">Adoption preview: {adoption.origin}, catalogue namespace {adoption.catalogNamespace}, verified original service {adoption.serviceOrigin}, key {adoption.serviceKey}. {adoption.existingRecords} saved records; {adoption.identities.filter(row => row.missing).length} native IDs are not currently visible.</p>
            <p>An origin-only backup cannot prove historical alias identity. Review the IDs and current titles before binding to this namespace. Unknown/hidden native IDs remain local and are excluded from protocol publication. Export the old-scope backup first.</p>
            <Button variant="outline" onclick={() => { showAdoption = !showAdoption; }}>Review saved title identities ({adoption.identities.length})</Button>{#if showAdoption}<div class="space-y-2">
              <p class="text-sm">Showing {Math.min(adoptionPage * 100 + 1, adoption.identities.length)}–{Math.min((adoptionPage + 1) * 100, adoption.identities.length)} of {adoption.identities.length} native IDs.</p>
              {#each adoption.identities.slice(adoptionPage * 100, (adoptionPage + 1) * 100) as identity}<p class="break-all text-xs">{identity.kind} {identity.id}: {identity.title}{identity.missing ? ' (unmapped)' : ''}</p>{/each}
              <Button variant="outline" disabled={adoptionPage === 0} onclick={() => { adoptionPage--; }}>Previous identities</Button>
              <Button variant="outline" disabled={(adoptionPage + 1) * 100 >= adoption.identities.length} onclick={() => { adoptionPage++; }}>Next identities</Button>
            </div>{/if}
            <Button disabled={!!pending} onclick={() => { if (confirm('Bind these reviewed origin-only IDs to this current catalogue namespace and verified service identity? This changes backup scope and does not publish.')) void run('adopt-service', async () => { await adoptServiceIdentity(adoption!); adoption = undefined; notice = 'Reviewed source namespace/service identity adopted explicitly.'; }); }}>Adopt reviewed source identity</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { adoption = undefined; }}>Cancel adoption</Button>
          </div>
        {/if}
</section>
<section class="space-y-3 border-t pt-4"><strong class="block font-semibold">Recover saved titles</strong>      <p class="text-sm">These are saved entries from this library’s original catalogue. Changing them here does not match them to a different title or publish anything.</p>
      {#each series as row, i}
        <div class="flex flex-wrap items-center gap-2">
          <span>Saved title {i + 1}</span><label>Status for saved title {i + 1}<select disabled={!!pending} value={row.status} onchange={event => run('saved-status', async () => { await saveSavedSeries(row.id!, (event.target as HTMLSelectElement).value, !!row.notification); notice = 'Existing saved-device status changed locally.'; })}>{#each statuses as status}<option value={status}>{status.replaceAll('_', ' ')}</option>{/each}</select></label>
          <label><input type="checkbox" checked={row.notification} disabled={!!pending} onchange={event => run('saved-notification', async () => { await saveSavedSeries(row.id!, row.status!, (event.target as HTMLInputElement).checked); })} /> Updates for saved title {i + 1}</label>
          <Button variant="ghost" disabled={!!pending} onclick={() => run('saved-remove', async () => { await removeLocalRecord('series', row.key); })}>Remove saved title {i + 1}</Button><details><summary>Title reference for support</summary><p class="break-all text-xs">{row.id}</p></details>
        </div>
      {/each}
</section>
<section class="space-y-3 border-t pt-4"><strong class="block font-semibold">Portable library backups</strong><p class="text-muted-foreground text-sm">Portable backups belong to your selected identity. They are separate from on-device library backups. Older backups without an identity are kept separately until you choose to import them.</p>        <p class="text-sm">{$signerIdentity.owner ? 'Using your selected identity' : 'Choose an identity first'}</p><Button variant="outline" disabled={!!pending} onclick={() => run('canonical-connect', async () => { await connectSigner(assertActive); resetReviews(); })}>Choose backup identity</Button><Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => run('canonical-export', canonicalExport)}>Export portable backup</Button>
        <Button variant="outline" disabled={!!pending} onclick={() => run('legacy-canonical-export', async () => { downloadBlob(await exportLegacyCanonicalBackup(), 'manga-legacy-unscoped-library.jsonl'); notice = 'Legacy unscoped backup exported locally. To adopt it, connect the intended key and explicitly review its import. Nothing published.'; })}>Export older backup</Button>
        <label>Preview portable backup <input type="file" accept=".jsonl,.json,application/json,application/x-ndjson" disabled={!!pending || !$signerIdentity.owner} onchange={canonicalFileChanged} /></label>
        {#if canonicalImport}
          <div class="rounded border p-2">
            <p>Backup: {canonicalImport.file.name}, {canonicalImport.records} entries. {Object.entries(canonicalImport.counts).map(([name, count]) => name + ': ' + count).join(', ')}. This replaces only your selected identity’s portable library. Your separate on-device library is kept. Nothing is made public unless you review and approve publishing it.</p>
            <Button disabled={!!pending} onclick={() => { if (confirm('Replace this identity’s portable library with the reviewed backup? Export a backup first.')) void run('canonical-import', async () => { await applyCanonicalImport(canonicalImport!); canonicalImport = undefined; await loadCanonical(); notice = 'Backup restored on this device. Nothing published.'; }); }}>Restore reviewed backup</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { canonicalImport = undefined; }}>Cancel import</Button>
          </div>
        {/if}
        <Button variant="outline" onclick={() => { showCanonical = !showCanonical; }}>Inspect saved protocol data</Button>{#if showCanonical}<div class="space-y-3 border-t pt-4">
          <label>Family <select bind:value={canonicalFamily} onchange={canonicalFamilyChanged}><option value="series">Series</option><option value="chapters">Chapters</option><option value="lists">Lists</option><option value="members">Members</option></select></label>
          <Button variant="outline" disabled={!!pending} onclick={() => run('canonical-read', async () => { canonicalAfter = null; await loadCanonical(); })}>Refresh canonical records</Button>
          <p>{Object.entries(canonicalCounts).filter(([name]) => ['series', 'chapters', 'lists', 'members'].includes(name)).map(([name, count]) => name + ': ' + count).join(', ')}</p>
          {#each canonicalRows as row}
            <p class="break-all text-xs">{JSON.stringify(row)} <Button variant="ghost" disabled={!!pending} onclick={() => { void removeCanonicalRow(row).catch(cause => { error = message(cause); }); }}>Remove locally</Button></p>
          {/each}
          {#if canonicalRows.length && canonicalRows.length % 50 === 0}<Button variant="outline" disabled={!!pending} onclick={() => run('canonical-more', () => loadCanonical(true))}>Load more</Button>{/if}
        </div>{/if}
</section>
{/if}
{#if pending}<p role="status">Updating your library…</p>{/if}
<p role="status">{notice}</p>
{#if error || $localLibraryState.error}<p role="alert" class="text-destructive">{error || $localLibraryState.error}</p>{/if}
</section>
