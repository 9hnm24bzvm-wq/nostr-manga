<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { createRouteActionOwner } from '$platform/route-action-owner';
  import { toast } from 'svelte-sonner';
  import { Button } from '$lib/components/ui/button';
  import { localLibraryState, libraryIdentity, openLibrary, verifyCatalogueScope, useMode, message, currentDeviceSnapshot, previewImport, previewAccountImport, applyImport, exportBackup, downloadBlob, createList, saveListMember, saveSavedSeries, removeLocalRecord, captureLibraryContext, assertLibraryContext, type ImportReview } from './library';
  import { sameScope, statuses } from './device-store';
  import { recordKey } from './protocol/codec.js';
  import type { Counts, LibraryRecord } from './protocol/types.d.ts';
  import { connectSigner, disconnectSigner, signerIdentity, capabilities, relayPreferences, rememberRelay, selectRelays, forgetRelay, admitAccount, previewPublication, publishReviewed, publicationJobs, retryPublication, cancelPublication, removePublication, previewServiceAdoption, adoptServiceIdentity, type ServiceAdoptionReview, previewRecovery, applyRecovery, exportCanonicalBackup, canonicalRecords, removeCanonicalRecord, previewCanonicalImport, applyCanonicalImport, type CanonicalImportReview, type Relay, type PublicationReview, type RecoveryReview, type Capabilities } from './publication';
  let notice = $state('Device saves stay in IndexedDB. Publication is off.'), error = $state(''), pending = $state('');
  let relays = $state<Relay[]>([]), relayRevision = $state(0), jobs = $state<any[]>([]);
  let policy = $state<Capabilities>(), relayOrigin = $state(''), serviceKey = $state('');
  let publicConsent = $state(false), publication = $state<PublicationReview>(), imported = $state<ImportReview>(), recovery = $state<RecoveryReview>();
  let listName = $state(''), memberList = $state(''), memberSeries = $state(''), memberPosition = $state(0), scopeText = $state('');
  let adoption = $state<ServiceAdoptionReview>(), adoptionPage = $state(0);
  let scopeMatches = $state(true), canonicalFamily = $state('series'), canonicalRows = $state<LibraryRecord[]>([]), canonicalCounts = $state<Partial<Counts>>({}), canonicalAfter = $state<string | null>(null), canonicalImport = $state<CanonicalImportReview>();
  let canonicalGeneration = 0, canonicalLoadedFamily = '', canonicalDisposed = false;
  function invalidateCanonical() { canonicalGeneration++; canonicalLoadedFamily = ''; canonicalRows = []; canonicalAfter = null; }
  async function canonicalFamilyChanged() { invalidateCanonical(); await run('canonical-read', () => loadCanonical()); }
  async function loadCanonical(more = false) {
    const family = canonicalFamily, generation = ++canonicalGeneration;
    if (more && canonicalLoadedFamily !== family) throw Error('Canonical family changed; refresh before loading more');
    const previous = more ? [...canonicalRows] : [], after = more ? canonicalAfter : null;
    if (!more) { canonicalLoadedFamily = ''; canonicalRows = []; canonicalAfter = null; }
    const result = await canonicalRecords(family, after);
    if (canonicalDisposed || generation !== canonicalGeneration || family !== canonicalFamily) return;
    canonicalLoadedFamily = family; canonicalRows = [...previous, ...result.records]; canonicalCounts = result.counts;
    const last = result.records.at(-1); canonicalAfter = last ? recordKey(family, last) : after;
  }
  async function removeCanonicalRow(row: LibraryRecord) {
    const family = canonicalLoadedFamily, key = recordKey(family, row);
    if (!family || family !== canonicalFamily || !canonicalRows.includes(row)) throw Error('Canonical selection changed; refresh and review the current record');
    if (!confirm('Remove this local canonical record? Already published snapshots remain public.')) return;
    await run('canonical-remove', async () => {
      if (canonicalDisposed || family !== canonicalFamily || canonicalLoadedFamily !== family || !canonicalRows.includes(row)) throw Error('Canonical selection changed before removal; no record removed');
      // This captured family/key is never reinterpreted after an await.
      await removeCanonicalRecord(family, key); await loadCanonical();
    });
  }
  async function canonicalFileChanged(event: Event) { const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = ''; if (file) await run('canonical-import-preview', async () => { canonicalImport = await previewCanonicalImport(file); }); }
  const lists = $derived($localLibraryState.records.filter(row => row.family === 'lists'));
  const series = $derived($localLibraryState.records.filter(row => row.family === 'series'));
  async function refresh() {
    const preferences = await relayPreferences(); relays = preferences.relays; relayRevision = preferences.revision;
    jobs = await publicationJobs();
    const snapshot = await currentDeviceSnapshot();
    scopeText = `${snapshot.scope.origin} · namespace ${snapshot.scope.catalogNamespace ?? 'origin-only'} · ${snapshot.scope.serviceKey ?? 'origin-only scope; service identity not adopted'}`;
  }
  async function run(name: string, operation: () => Promise<void>) {
    if (pending) return; pending = name; error = '';
    try { await operation(); }
    catch (cause) { error = message(cause); }
    finally { pending = ''; await refresh().catch(cause => { error ||= message(cause); }); }
  }
  async function cancel(id: string) { try { await cancelPublication(id); await refresh(); notice = 'Further sending cancelled. Already sent copies cannot be recalled.'; } catch (cause) { error = message(cause); } }
  function resetReviews() { publication = undefined; publicConsent = false; recovery = undefined; }
  async function chooseMode(mode: 'device' | 'account') {
    await useMode(mode); resetReviews(); imported = undefined;
    notice = mode === 'device' ? 'Using this device library.' : 'Using the signed-in WeebDex account library. Device records are preserved.';
  }
  const listRouteOwner = createRouteActionOwner();
  async function openDeviceList(event: MouseEvent, id: string) {
    event.preventDefault();
    const route = listRouteOwner.capture(), listId = id;
    await run('open-device-list', async () => {
      if (!route.alive()) return;
      const context = captureLibraryContext(), saved = await currentDeviceSnapshot();
      if (!route.alive()) return;
      assertLibraryContext(context);
      if (!saved.records.some(row => row.family === 'lists' && row.id === listId)) throw Error('This list no longer belongs to the current saved-device scope');
      await useMode('device');
      if (!route.alive()) return;
      resetReviews(); imported = undefined;
      const current = await currentDeviceSnapshot();
      if (!route.alive()) return;
      if ($localLibraryState.mode !== 'device' || !sameScope(saved.scope, current.scope) || !current.records.some(row => row.family === 'lists' && row.id === listId)) throw Error('The device list or selected library changed. Reopen the current saved list.');
      notice = 'Opening this custom device list in device-library mode. Account follows are preserved separately.';
      await goto('/library?list=' + encodeURIComponent(listId));
    });
  }
  async function fileChanged(event: Event) {
    const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = '';
    if (!file) return;
    await run('preview-import', async () => { imported = await previewImport(file); scopeMatches = sameScope(imported.snapshot.scope, (await currentDeviceSnapshot()).scope); });
  }
  $effect(() => {
    if (publication && (publication.deviceRevision !== $localLibraryState.revision || publication.owner !== $signerIdentity.owner || publication.generation !== $signerIdentity.generation)) { publication = undefined; publicConsent = false; }
  });
  onMount(() => {
    const reportError = (event: Event) => { error = String((event as CustomEvent).detail); toast.error('Library save failed', { description: error }); };
    window.addEventListener('weebdex-library-error', reportError);
    void run('open', async () => { await openLibrary(); await refresh(); void capabilities().then(result => { policy = result; }).catch(() => {}); });
    return () => { canonicalDisposed = true; canonicalGeneration++; window.removeEventListener('weebdex-library-error', reportError); };
  });
</script>
<details class="my-3 rounded-md border p-3" data-local-library-panel>
  <summary class="cursor-pointer font-medium">Device library, backups and optional public sync</summary>
  <div class="mt-3 space-y-3">
    <p>Library source: {$localLibraryState.mode === 'device' ? 'this browser device' : 'signed-in WeebDex account'}. Local identity and Nostr signing are independent of account login.</p>
    <p class="break-all text-sm">Native catalogue scope: {scopeText}</p>
    {#if $localLibraryState.scopeStatus !== "verified"}<p role="status">{$localLibraryState.scopeReason ?? "Verifying current catalogue identity. Current title/chapter controls stay unavailable until verified; saved-device records and backups remain available."}</p>{/if}
    <p role="status">Scope verification: {$localLibraryState.scopeStatus}. {$localLibraryState.scopeReason ?? ""}</p>
    <Button variant="outline" disabled={!!pending} onclick={() => run("verify-scope", async () => { await verifyCatalogueScope(); })}>Verify current catalogue scope</Button>
    <div class="flex flex-wrap gap-2">
      <Button variant="outline" disabled={!!pending || $localLibraryState.mode === 'device'} onclick={() => run('device-mode', () => chooseMode('device'))}>Use device library</Button>
      <Button variant="outline" disabled={!!pending || $localLibraryState.mode === 'account'} onclick={() => run('account-mode', () => chooseMode('account'))}>Use account library</Button>
      <Button variant="outline" disabled={!!pending} onclick={() => run('account-import', async () => { imported = await previewAccountImport(); scopeMatches = true; })}>Preview account import to device</Button>
    </div>
    <p>{series.length} followed series · {$localLibraryState.records.filter(row => row.family === 'chapters').length} chapter progress records · {lists.length} lists</p>
    <p class="text-sm">Keep private backups. Browsers can evict storage; a successful save does not guarantee permanent retention.</p>
    <div class="flex flex-wrap items-center gap-2">
      <Button variant="outline" disabled={!!pending} onclick={() => run('export', async () => { downloadBlob(await exportBackup(), 'weebdex-device-library.json'); notice = 'Scoped device backup downloaded locally.'; })}>Export device backup</Button>
      <label class="text-sm">Preview device backup <input type="file" accept="application/json,.json" disabled={!!pending} onchange={fileChanged} /></label>
      <Button variant="outline" disabled={!!pending} onclick={() => run('persist', async () => { if (!navigator.storage?.persist) throw Error('Persistent storage is unavailable in this browser'); notice = await navigator.storage.persist() ? 'Persistent storage granted. Keep backups anyway.' : 'Persistence was not granted. Keep exported backups.'; })}>Request persistent storage</Button>
    </div>
    {#if imported}
      <div class="rounded border p-2">
        <p>Import preview: {imported.source} · {imported.snapshot.records.length} records · {imported.collisions ?? 0} overlapping keys</p>
        <p class="break-all text-sm">Backup scope: {imported.snapshot.scope.origin} · namespace {imported.snapshot.scope.catalogNamespace ?? 'origin-only'} · {imported.snapshot.scope.serviceKey ?? 'origin-only'}</p>
        {#if scopeMatches}
          <p>Merge keeps newer device records and retains local state on equal timestamps. Replace removes the current device records. Neither option publishes.</p>
          <Button disabled={!!pending} onclick={() => run('merge', async () => { await applyImport(imported!, 'merge'); imported = undefined; notice = 'Backup merged locally.'; })}>Merge reviewed import</Button>
          <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Replace all device-library records with this reviewed backup? Export a backup first.')) void run('replace', async () => { await applyImport(imported!, 'replace'); imported = undefined; notice = 'Device library replaced from the reviewed backup.'; }); }}>Replace with reviewed import</Button>
        {:else}
          <p role="alert">Native IDs belong to a different instance. This import is blocked. Restore it on its source instance; canonical protocol backups remain portable.</p>
        {/if}
        <Button variant="outline" disabled={!!pending} onclick={() => { imported = undefined; }}>Cancel import</Button>
      </div>
    {/if}
    <details class="rounded border p-2">
      <summary class="cursor-pointer">Edit saved-device series without catalogue lookup</summary>
      <p class="text-sm">These IDs remain in the displayed saved-device scope. No current title is looked up or rebound.</p>
      {#each series as row}
        <div class="flex flex-wrap items-center gap-2">
          <span>{row.id}</span><label>Status for saved {row.id}<select disabled={!!pending} value={row.status} onchange={event => run('saved-status', async () => { await saveSavedSeries(row.id!, (event.target as HTMLSelectElement).value, !!row.notification); notice = 'Existing saved-device status changed locally.'; })}>{#each statuses as status}<option value={status}>{status}</option>{/each}</select></label>
          <label><input type="checkbox" checked={row.notification} disabled={!!pending} onchange={event => run('saved-notification', async () => { await saveSavedSeries(row.id!, row.status!, (event.target as HTMLInputElement).checked); })} /> Updates for saved {row.id}</label>
          <Button variant="ghost" disabled={!!pending} onclick={() => run('saved-remove', async () => { await removeLocalRecord('series', row.key); })}>Remove saved {row.id}</Button>
        </div>
      {/each}
    </details>
    <details class="rounded border p-2">
      <summary class="cursor-pointer">Custom device lists</summary>
      <form class="my-2 flex flex-wrap gap-2" onsubmit={event => { event.preventDefault(); void run('create-list', async () => { memberList = await createList(listName); listName = ''; notice = 'List created on this device.'; }); }}>
        <label>List name <input class="rounded border px-2" bind:value={listName} maxlength="128" required /></label>
        <Button type="submit" disabled={!!pending}>Create list</Button>
      </form>
      <form class="my-2 flex flex-wrap gap-2" onsubmit={event => { event.preventDefault(); void run('member', async () => { await saveListMember(memberList, memberSeries, memberPosition); notice = 'List membership saved locally.'; }); }}>
        <label>List <select class="rounded border px-2" bind:value={memberList} required><option value="">Choose list</option>{#each lists as list}<option value={list.id}>{list.name}</option>{/each}</select></label>
        <label>Series <select class="rounded border px-2" bind:value={memberSeries} required><option value="">Choose saved series</option>{#each series as row}<option value={row.id}>{row.id} ({row.status})</option>{/each}</select></label>
        <label>Position <input class="w-20 rounded border px-2" type="number" min="0" step="1" bind:value={memberPosition} required /></label>
        <Button type="submit" disabled={!!pending}>Save member</Button>
      </form>
      {#each lists as list}
        <div class="my-1"><a class="underline" href={'/library?list=' + list.id} onclick={event => openDeviceList(event, list.id!)}>{list.name}</a> · {list.id} <Button variant="ghost" disabled={!!pending} onclick={() => { if (confirm('Remove this local list and its memberships?')) void run('remove-list', async () => { await removeLocalRecord('lists', list.key); }); }}>Remove locally</Button></div>
        {#each $localLibraryState.records.filter(row => row.family === 'members' && row.list === list.id) as member}
          <div class="ml-3 text-sm">#{member.position}: {member.series} <Button variant="ghost" disabled={!!pending} onclick={() => run('remove-member', async () => { await removeLocalRecord('members', member.key); })}>Remove member</Button></div>
        {/each}
      {/each}
    </details>
    <details class="rounded border p-2">
      <summary class="cursor-pointer">Optional Nostr publication and verified recovery</summary>
      <div class="mt-2 space-y-2">
        <p>Publishing exposes signed reading status, chapter completion/pages, list names and memberships publicly. Local saves never publish automatically.</p>
        <p class="break-all">Signing key: {$signerIdentity.owner ?? 'none connected'}</p>
        <Button variant="outline" disabled={!!pending} onclick={() => run('connect-signer', async () => { resetReviews(); await connectSigner(); notice = 'Signer connected. No library data sent.'; })}>Connect external browser signer</Button>
        <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => { disconnectSigner(); resetReviews(); }}>Disconnect signer</Button>
        {#if policy?.service_key && ($localLibraryState.scopeStatus === 'origin_only' || scopeText.includes('origin-only'))}
          <Button variant="outline" disabled={!!pending} onclick={() => run('adoption-preview', async () => { adoption = await previewServiceAdoption(); adoptionPage = 0; })}>Preview source identity adoption</Button>
        {/if}
        {#if adoption}
          <div class="rounded border p-2 space-y-2">
            <p class="break-all">Adoption preview: {adoption.origin}, catalogue namespace {adoption.catalogNamespace}, verified original service {adoption.serviceOrigin}, key {adoption.serviceKey}. {adoption.existingRecords} saved records; {adoption.identities.filter(row => row.missing).length} native IDs are not currently visible.</p>
            <p>An origin-only backup cannot prove historical alias identity. Review the IDs and current titles before binding to this namespace. Unknown/hidden native IDs remain local and are excluded from protocol publication. Export the old-scope backup first.</p>
            <details><summary class="cursor-pointer">Review current native ID/title interpretation ({adoption.identities.length})</summary>
              <p class="text-sm">Showing {Math.min(adoptionPage * 100 + 1, adoption.identities.length)}–{Math.min((adoptionPage + 1) * 100, adoption.identities.length)} of {adoption.identities.length} native IDs.</p>
              {#each adoption.identities.slice(adoptionPage * 100, (adoptionPage + 1) * 100) as identity}<p class="break-all text-xs">{identity.kind} {identity.id}: {identity.title}{identity.missing ? ' (unmapped)' : ''}</p>{/each}
              <Button variant="outline" disabled={adoptionPage === 0} onclick={() => { adoptionPage--; }}>Previous identities</Button>
              <Button variant="outline" disabled={(adoptionPage + 1) * 100 >= adoption.identities.length} onclick={() => { adoptionPage++; }}>Next identities</Button>
            </details>
            <Button disabled={!!pending} onclick={() => { if (confirm('Bind these reviewed origin-only IDs to this current catalogue namespace and verified service identity? This changes backup scope and does not publish.')) void run('adopt-service', async () => { await adoptServiceIdentity(adoption!); adoption = undefined; notice = 'Reviewed source namespace/service identity adopted explicitly.'; }); }}>Adopt reviewed source identity</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { adoption = undefined; }}>Cancel adoption</Button>
          </div>
        {/if}
        <p class="text-sm">Available protocol bridge: {policy?.available ? policy.origin : 'not configured'}. Public-key account admission is a separate instance prerequisite.</p>
        <form class="flex flex-wrap gap-2" onsubmit={event => { event.preventDefault(); if (confirm(`Verify and remember ${relayOrigin} with expected service key ${serviceKey || 'identity discovered by signed proof'}? This does not send your library.`)) void run('remember-relay', async () => { await rememberRelay(relayOrigin, serviceKey.trim(), relayRevision); resetReviews(); notice = 'Verified instance remembered, initially unselected.'; }); }}>
          <label>Instance origin <input class="rounded border px-2" type="url" bind:value={relayOrigin} placeholder="https://instance.example" required /></label>
          <label>Expected service public key <input class="rounded border px-2" bind:value={serviceKey} placeholder="64 lowercase hex characters" /></label>
          <Button type="submit" disabled={!!pending}>Verify and remember instance</Button>
        </form>
        {#each relays as relay}
          <div class="rounded border p-2">
            <label><input type="checkbox" checked={relay.selected} disabled={!!pending} onchange={event => run('select-relay', async () => { await selectRelays(relays.map(row => row.origin === relay.origin ? { ...row, selected: (event.target as HTMLInputElement).checked } : row), relayRevision); resetReviews(); })} /> Select {relay.origin}</label>
            <p class="break-all text-xs">Pinned service key: {relay.serviceKey}</p>
            <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => { if (confirm(`Request admission for public key ${$signerIdentity.owner} at ${relay.origin}? This does not publish the library.`)) void run('enroll', async () => { await admitAccount(relay); notice = 'Public-key account admitted. Library remains local.'; }); }}>Request public-key account admission</Button>
            <Button variant="outline" disabled={!!pending || !$signerIdentity.owner} onclick={() => run('recover-preview', async () => { recovery = await previewRecovery(relay); })}>Preview verified public recovery</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Forget this instance and cancel unfinished snapshots sent to it? Published copies remain public.')) void run('forget', async () => { await forgetRelay(relay.origin, relayRevision); resetReviews(); }); }}>Forget instance</Button>
          </div>
        {/each}
        <Button disabled={!!pending || !$signerIdentity.owner} onclick={() => run('publish-preview', async () => { publicConsent = false; publication = await previewPublication(); })}>Preview public snapshot</Button>
        {#if publication}
          <div class="rounded border p-2">
            <p>Reviewed device revision {publication.deviceRevision}, canonical revision {publication.protocolRevision}. {publication.graph.records} canonical records included; {publication.excluded.length} native-only, hidden or unmapped records excluded.</p>
            <p class="break-all">Manifest {publication.graph.root.hash}. Signing key {publication.owner}. Recipients: {publication.recipients.map(row => row.origin).join(', ')}</p>
            {#if publication.excluded.length}<p class="break-all text-xs">Excluded IDs: {publication.excluded.slice(0, 30).join(', ')}{publication.excluded.length > 30 ? '…' : ''}</p>{/if}
            <label><input type="checkbox" bind:checked={publicConsent} /> I understand that this reviewed snapshot becomes public at these recipients.</label>
            <Button disabled={!!pending || !publicConsent} onclick={() => run('publish', async () => { const reviewed = $state.snapshot(publication!); publicConsent = false; publication = undefined; await publishReviewed(reviewed, true); notice = 'Publication attempt finished. Review every instance receipt below.'; })}>Sign and publish reviewed snapshot</Button>
            <Button variant="outline" disabled={!!pending} onclick={resetReviews}>Cancel preview</Button>
          </div>
        {/if}
        {#if recovery}
          <div class="rounded border p-2">
            <p class="break-all">Verified public recovery: {recovery.relay.origin}, owner {recovery.event.pubkey}, event {recovery.event.id}, signed at {recovery.event.created_at}. Canonical protocol records will be replaced; native device records remain separate.</p>
            <Button disabled={!!pending} onclick={() => { if (confirm('Replace this device canonical protocol library with the verified recovery? Export its canonical backup first.')) void run('apply-recovery', async () => { await applyRecovery(recovery!); recovery = undefined; notice = 'Verified canonical library restored locally. Automatic publication is still off.'; }); }}>Restore reviewed canonical records</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { recovery = undefined; }}>Cancel recovery</Button>
          </div>
        {/if}
        <Button variant="outline" disabled={!!pending} onclick={() => run('canonical-export', async () => { downloadBlob(await exportCanonicalBackup(), 'manga-canonical-library.jsonl'); })}>Export portable canonical backup</Button>
        <label>Preview portable canonical backup <input type="file" accept=".jsonl,.json,application/json,application/x-ndjson" disabled={!!pending} onchange={canonicalFileChanged} /></label>
        {#if canonicalImport}
          <div class="rounded border p-2">
            <p>Canonical backup preview: {canonicalImport.file.name}, {canonicalImport.records} records. {Object.entries(canonicalImport.counts).map(([name, count]) => name + ': ' + count).join(', ')}. This replaces only canonical records; native device data remains scoped separately.</p>
            <Button disabled={!!pending} onclick={() => { if (confirm('Replace the canonical protocol library with this reviewed backup? Export its current backup first.')) void run('canonical-import', async () => { await applyCanonicalImport(canonicalImport!); canonicalImport = undefined; await loadCanonical(); notice = 'Canonical backup imported locally. Nothing published.'; }); }}>Restore reviewed canonical backup</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { canonicalImport = undefined; }}>Cancel canonical import</Button>
          </div>
        {/if}
        <details class="rounded border p-2">
          <summary class="cursor-pointer">Inspect canonical records on this device</summary>
          <label>Family <select bind:value={canonicalFamily} onchange={canonicalFamilyChanged}><option value="series">Series</option><option value="chapters">Chapters</option><option value="lists">Lists</option><option value="members">Members</option></select></label>
          <Button variant="outline" disabled={!!pending} onclick={() => run('canonical-read', async () => { canonicalAfter = null; await loadCanonical(); })}>Refresh canonical records</Button>
          <p>{Object.entries(canonicalCounts).filter(([name]) => ['series', 'chapters', 'lists', 'members'].includes(name)).map(([name, count]) => name + ': ' + count).join(', ')}</p>
          {#each canonicalRows as row}
            <p class="break-all text-xs">{JSON.stringify(row)} <Button variant="ghost" disabled={!!pending} onclick={() => { void removeCanonicalRow(row).catch(cause => { error = message(cause); }); }}>Remove locally</Button></p>
          {/each}
          {#if canonicalRows.length && canonicalRows.length % 50 === 0}<Button variant="outline" disabled={!!pending} onclick={() => run('canonical-more', () => loadCanonical(true))}>Load more</Button>{/if}
        </details>
        <p>Saved outbox snapshots:</p>
        {#each jobs as job}
          <div class="rounded border p-2">
            <p class="break-all text-xs">{job.id} · device revision {job.deviceRevision ?? 'canonical only'} · {job.cancelled ? 'cancelled' : 'saved'} · signer {job.event.pubkey}</p>
            {#each job.recipients as recipient}<p>{recipient.origin}: {recipient.state} · {recipient.message}</p>{/each}
            {#if !job.cancelled && job.recipients.some((row: { state: string }) => !['accepted', 'superseded'].includes(row.state))}
              <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Retry this previously approved public snapshot to its original recipients? New device changes are not included.')) void run('retry', async () => { await retryPublication(job.id); notice = 'Retry finished; review receipts.'; }); }}>Retry approved snapshot</Button>
              <Button variant="outline" onclick={() => cancel(job.id)}>Cancel further sending</Button>
            {:else}
              <Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Remove this local retry record? Published copies are unaffected.')) void run('remove-job', async () => { await removePublication(job.id); }); }}>Remove local outbox record</Button>
            {/if}
          </div>
        {/each}
      </div>
    </details>
    {#if pending}<p role="status">Working: {pending.replaceAll('-', ' ')}…</p>{/if}
    <p role="status">{notice}</p>
    {#if error || $localLibraryState.error}<p role="alert" class="text-red-600">{error || $localLibraryState.error}</p>{/if}
  </div>
</details>
