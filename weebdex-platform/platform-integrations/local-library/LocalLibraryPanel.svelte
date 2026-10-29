<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import { localLibraryState, openLibrary, useMode, message, currentDeviceSnapshot, previewImport, previewAccountImport, applyImport, exportBackup, downloadBlob, prepareDeviceBackupImport, applyDeviceBackupImport, type DeviceBackupRestoreReview, type ImportReview } from './library';
  import { sameScope } from './device-store';
  import {publicAccountLibraryUnavailable,publicUuidLibraryEnabled} from '../public-api/library-staging';
  import LibraryManagementTools from './LibraryManagementTools.svelte';
  import BackupReviewSummary from './BackupReviewSummary.svelte';
  import AccountLibraryTools from './AccountLibraryTools.svelte';
  let pending = $state(''), error = $state(''), notice = $state('');
  // Opaque frozen review handles are WeakMap authorities; never deep-proxy them.
  let deviceRestore = $state.raw<DeviceBackupRestoreReview>(), foreignOwnerConsent = $state(false);
  let imported = $state.raw<ImportReview>(), scopeMatches = $state(false), reviewOpen = $state(false);
  let tool = $state<'lists' | 'sync' | 'destinations' | 'publish' | 'public-recovery' | 'attempts' | 'recovery'>('lists'), toolOpen = $state(false);
  const restoreWatchers = new Set<() => void>();
  let disposed = false, reviewGeneration = 0, toolGeneration = $state(0);
  const titles = $derived($localLibraryState.records.filter(row => row.family === 'series'));
  const lists = $derived($localLibraryState.records.filter(row => row.family === 'lists'));
  const accountMode = $derived(publicUuidLibraryEnabled && !publicAccountLibraryUnavailable && $localLibraryState.mode === 'account');
  function accountViewGuard(){if(disposed || !accountMode)throw Error('This account library view changed. Reopen it to continue.');}
  const toolTitle = $derived({ lists: 'Device lists', sync: 'Public sync', destinations: 'Sync destinations', publish: 'Publish library', 'public-recovery': 'Recover public library', attempts: 'Saved sync attempts', recovery: 'Library recovery' }[tool]);
  async function run(name: string, operation: () => Promise<void>) {
    if (pending || disposed) return; pending = name; error = '';
    try { await operation(); } catch (cause) { if (!disposed) error = message(cause); }
    finally { if (!disposed) pending = ''; }
  }
  function setToolOpen(open: boolean) { toolGeneration++; toolOpen = open; }
  function openTool(next: typeof tool) { tool = next; setToolOpen(true); }
  function toolGuard(generation: number) { return () => { if (disposed || !toolOpen || generation !== toolGeneration) throw Error('This library task was closed. Reopen it and review again.'); }; }
  function invalidateReview() { reviewGeneration++; for (const changed of [...restoreWatchers]) changed(); restoreWatchers.clear(); }
  async function preview(operation: () => Promise<ImportReview>) {
    invalidateReview(); const generation = reviewGeneration;
    const result = await operation(), snapshot = await currentDeviceSnapshot();
    if (disposed || generation !== reviewGeneration) return;
    deviceRestore = undefined; foreignOwnerConsent = false; imported = result; scopeMatches = sameScope(result.snapshot.scope, snapshot.scope); reviewOpen = true;
  }
  async function fileChanged(event: Event) {
    const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = '';
    if (file) await run('preview-import', () => preview(() => previewImport(file)));
  }
  function closeReview(open: boolean) { reviewOpen = open; if (!open) { invalidateReview(); imported = undefined; deviceRestore = undefined; foreignOwnerConsent = false; } }
  function restoreGuard(generation: number, selected: ImportReview) { return () => { if (disposed || !reviewOpen || generation !== reviewGeneration || imported !== selected) throw Error('This backup review was closed. Preview the file again.'); }; }
  async function prepareRestore(mode: 'merge' | 'replace') {
    const selected = imported!, current = restoreGuard(reviewGeneration, selected);
    const result = await prepareDeviceBackupImport(selected, mode, foreignOwnerConsent, current);
    current(); deviceRestore = result;
  }
  async function confirmRestore() {
    const selected = deviceRestore!, current = restoreGuard(reviewGeneration, imported!);
    await applyDeviceBackupImport(selected, current, changed => { current(); restoreWatchers.add(changed); return () => { restoreWatchers.delete(changed); }; }); current();
    closeReview(false); notice = selected.mode === 'merge' ? 'Backup merged locally.' : 'Device library restored from the reviewed backup.';
  }
  onMount(() => { void run('open', openLibrary); return () => { disposed = true; invalidateReview(); imported = undefined; deviceRestore = undefined; }; });
</script>
<section class="space-y-5" data-local-library-panel>
  <header class="space-y-1"><h1 class="text-lg font-semibold">Library</h1><p class="text-muted-foreground text-sm">Your saved titles, lists and backups.</p></header>
  <section class="space-y-3">
    <div class="space-y-1"><strong class="block font-semibold">Save reading progress</strong><p class="text-foreground/85 max-w-md">Library source: {$localLibraryState.mode === 'device' ? 'this browser device' : 'signed-in WeebDex account'}. Device saves do not need an account.</p></div>
    <div class="flex flex-wrap gap-2">
      <Button variant="outline" disabled={!!pending || $localLibraryState.mode === 'device'} onclick={() => run('device-mode', async () => { await useMode('device'); notice = 'Using this device library.'; })}>Use device library</Button>
      {#if !publicAccountLibraryUnavailable}<Button variant="outline" disabled={!!pending || $localLibraryState.mode === 'account'} onclick={() => run('account-mode', async () => { await useMode('account'); notice = 'Using your account library. Device saves are preserved.'; })}>Use account library</Button>{/if}
    </div>
    <p class="text-muted-foreground text-sm">Device library: {titles.length} saved titles · {lists.length} lists</p>
  </section>
  {#if accountMode}<AccountLibraryTools assertViewCurrent={accountViewGuard} />{/if}
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Device backups</strong><p class="text-foreground/85 max-w-md">Download a private copy of this browser’s library. Backups stay on your device and are never published.</p></div>
    <div class="flex flex-wrap items-center gap-2">
      <Button variant="outline" disabled={!!pending} onclick={() => run('export', async () => { downloadBlob(await exportBackup(), 'weebdex-device-library.json'); notice = 'Device backup downloaded.'; })}>Export device backup</Button>
      <label class="text-sm">Preview device backup <input type="file" accept="application/json,.json" disabled={!!pending} onchange={fileChanged} /></label>
      {#if !publicUuidLibraryEnabled && !publicAccountLibraryUnavailable}<Button variant="outline" disabled={!!pending} onclick={() => run('account-import', () => preview(previewAccountImport))}>Preview account import to device</Button>{/if}
    </div>
    <p class="text-muted-foreground text-sm">Browsers can clear saved data. Keep an exported backup even when storage is protected.</p>
    <Button variant="ghost" disabled={!!pending} onclick={() => run('persist', async () => { if (!navigator.storage?.persist) throw Error('Persistent storage is unavailable in this browser'); notice = await navigator.storage.persist() ? 'Persistent storage granted. Keep backups anyway.' : 'Persistence was not granted. Keep exported backups.'; })}>Request persistent storage</Button>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Device titles and lists</strong><p class="text-foreground/85 max-w-md">Manage your saved titles in My Library, or organize device titles into lists.</p></div>
    <div class="flex flex-wrap gap-2"><Button variant="outline" href="/library">Open library</Button><Button variant="outline" onclick={() => openTool('lists')}>Manage lists</Button></div>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Public sync</strong><p class="text-foreground/85 max-w-md">Optional. Publish a reviewed copy to your chosen instance, or recover one you published earlier. Your reading becomes public only after you approve it.</p></div>
    <Button variant="outline" onclick={() => openTool('sync')}>Set up public sync</Button>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Recovery</strong><p class="text-foreground/85 max-w-md">Restore portable data or check saved titles when the catalogue has changed.</p></div>
    <Button variant="outline" onclick={() => openTool('recovery')}>Library recovery</Button>
  </section>
  {#if pending}<p role="status">Working…</p>{/if}<p role="status">{notice}</p>
  {#if error || $localLibraryState.error}<p role="alert" class="text-destructive">{error || $localLibraryState.error}</p>{/if}
</section>
<Dialog.Root open={toolOpen} onOpenChange={setToolOpen}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] !max-w-[min(42rem,calc(100dvw-2rem))] overflow-y-auto">
    <Dialog.Header><Dialog.Title>{toolTitle}</Dialog.Title></Dialog.Header>
    {#if toolOpen}{#key toolGeneration}<LibraryManagementTools view={tool} onTask={openTool} assertViewCurrent={toolGuard(toolGeneration)} />{/key}{/if}
  </Dialog.Content>
</Dialog.Root>
<Dialog.Root open={reviewOpen} onOpenChange={closeReview}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] !max-w-[min(36rem,calc(100dvw-2rem))] overflow-y-auto">
    <Dialog.Header><Dialog.Title>Review library import</Dialog.Title><Dialog.Description>Check this backup before changing your device library.</Dialog.Description></Dialog.Header>
    {#if imported}
      <BackupReviewSummary {imported} restore={deviceRestore} />
      {#if scopeMatches && imported.snapshot.version === 2}
        {#if deviceRestore}
          <div class="flex flex-wrap gap-2">
            <Button disabled={!!pending} onclick={() => run('restore-backup', confirmRestore)}>Confirm reviewed {deviceRestore.mode === 'merge' ? 'merge' : 'replacement'}</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { downloadBlob(new Blob([JSON.stringify(deviceRestore!.backup)], { type: 'application/json' }), 'weebdex-before-restore.json'); }}>Download pre-restore backup</Button>
            <Button variant="outline" disabled={!!pending} onclick={() => { deviceRestore = undefined; }}>Change restore choice</Button>
          </div>
        {:else}
          <p>This complete backup includes saved titles, progress, lists and recovery history. Choose how to restore it, then confirm the reviewed result.</p>
          {#if imported.foreignOwner}<p>This backup was made in another browser or device.</p><label class="block text-sm"><input type="checkbox" bind:checked={foreignOwnerConsent} /> I recognize this backup and want to restore its library on this device.</label>{/if}
          <div class="flex flex-wrap gap-2"><Button disabled={!!pending || !!imported.foreignOwner && !foreignOwnerConsent} onclick={() => run('prepare-merge', () => prepareRestore('merge'))}>Review backup merge</Button><Button variant="outline" disabled={!!pending || !!imported.foreignOwner && !foreignOwnerConsent} onclick={() => run('prepare-replace', () => prepareRestore('replace'))}>Review backup replacement</Button></div>
        {/if}
      {:else if scopeMatches}
        <p>Merge keeps the more recent save for each item; when times match, this device’s version is kept. Replace removes the current device library. Neither option publishes.</p>
        <div class="flex flex-wrap gap-2"><Button disabled={!!pending} onclick={() => run('merge', async () => { const selected = imported!; await applyImport(selected, 'merge'); closeReview(false); notice = 'Backup merged locally.'; })}>Merge reviewed import</Button><Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Replace the saved titles, progress and lists on this device with this backup? Download a backup of this device first.')) void run('replace', async () => { const selected = imported!; await applyImport(selected, 'replace'); closeReview(false); notice = 'Device library replaced from the reviewed backup.'; }); }}>Replace with reviewed import</Button></div>
      {:else}<p role="alert">This backup belongs to another catalogue. Restore it on its original site. If you have a portable library backup instead, open Library recovery.</p>{/if}
      <Button variant="outline" onclick={() => closeReview(false)}>Cancel import</Button>
    {/if}
  </Dialog.Content>
</Dialog.Root>
