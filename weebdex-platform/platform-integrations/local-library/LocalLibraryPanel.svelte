<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import { localLibraryState, openLibrary, useMode, message, currentDeviceSnapshot, previewImport, previewAccountImport, applyImport, exportBackup, downloadBlob, type ImportReview } from './library';
  import { sameScope } from './device-store';
  import LibraryManagementTools from './LibraryManagementTools.svelte';
  let pending = $state(''), error = $state(''), notice = $state('');
  let imported = $state<ImportReview>(), scopeMatches = $state(false), reviewOpen = $state(false);
  let tool = $state<'lists' | 'sync' | 'destinations' | 'publish' | 'public-recovery' | 'attempts' | 'recovery'>('lists'), toolOpen = $state(false);
  let disposed = false, reviewGeneration = 0, toolGeneration = $state(0);
  const titles = $derived($localLibraryState.records.filter(row => row.family === 'series'));
  const lists = $derived($localLibraryState.records.filter(row => row.family === 'lists'));
  const toolTitle = $derived({ lists: 'Device lists', sync: 'Public sync', destinations: 'Sync destinations', publish: 'Publish library', 'public-recovery': 'Recover public library', attempts: 'Saved sync attempts', recovery: 'Library recovery' }[tool]);
  async function run(name: string, operation: () => Promise<void>) {
    if (pending || disposed) return; pending = name; error = '';
    try { await operation(); } catch (cause) { if (!disposed) error = message(cause); }
    finally { if (!disposed) pending = ''; }
  }
  function setToolOpen(open: boolean) { toolGeneration++; toolOpen = open; }
  function openTool(next: typeof tool) { tool = next; setToolOpen(true); }
  function toolGuard(generation: number) { return () => { if (disposed || !toolOpen || generation !== toolGeneration) throw Error('This library task was closed. Reopen it and review again.'); }; }
  async function preview(operation: () => Promise<ImportReview>) {
    const generation = ++reviewGeneration;
    const result = await operation(), snapshot = await currentDeviceSnapshot();
    if (disposed || generation !== reviewGeneration) return;
    imported = result; scopeMatches = sameScope(result.snapshot.scope, snapshot.scope); reviewOpen = true;
  }
  async function fileChanged(event: Event) {
    const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = '';
    if (file) await run('preview-import', () => preview(() => previewImport(file)));
  }
  function closeReview(open: boolean) { reviewOpen = open; if (!open) { reviewGeneration++; imported = undefined; } }
  onMount(() => { void run('open', openLibrary); return () => { disposed = true; reviewGeneration++; imported = undefined; }; });
</script>
<section class="space-y-5" data-local-library-panel>
  <header class="space-y-1"><h1 class="text-lg font-semibold">Library</h1><p class="text-muted-foreground text-sm">Choose where to save your reading and keep a backup.</p></header>
  <section class="space-y-3">
    <div class="space-y-1"><strong class="block font-semibold">Save reading progress</strong><p class="text-foreground/85 max-w-md">Library source: {$localLibraryState.mode === 'device' ? 'this browser device' : 'signed-in WeebDex account'}. Device saves do not need an account.</p></div>
    <div class="flex flex-wrap gap-2">
      <Button variant="outline" disabled={!!pending || $localLibraryState.mode === 'device'} onclick={() => run('device-mode', async () => { await useMode('device'); notice = 'Using this device library.'; })}>Use device library</Button>
      <Button variant="outline" disabled={!!pending || $localLibraryState.mode === 'account'} onclick={() => run('account-mode', async () => { await useMode('account'); notice = 'Using your account library. Device saves are preserved.'; })}>Use account library</Button>
    </div>
    <p class="text-muted-foreground text-sm">{titles.length} saved titles · {lists.length} device lists</p>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Device backups</strong><p class="text-foreground/85 max-w-md">Download a private copy of this browser’s library. Backups stay on your device and are never published.</p></div>
    <div class="flex flex-wrap items-center gap-2">
      <Button variant="outline" disabled={!!pending} onclick={() => run('export', async () => { downloadBlob(await exportBackup(), 'weebdex-device-library.json'); notice = 'Device backup downloaded.'; })}>Export device backup</Button>
      <label class="text-sm">Preview device backup <input type="file" accept="application/json,.json" disabled={!!pending} onchange={fileChanged} /></label>
      <Button variant="outline" disabled={!!pending} onclick={() => run('account-import', () => preview(previewAccountImport))}>Preview account import to device</Button>
    </div>
    <p class="text-muted-foreground text-sm">Browsers can clear saved data. Keep an exported backup even when storage is protected.</p>
    <Button variant="ghost" disabled={!!pending} onclick={() => run('persist', async () => { if (!navigator.storage?.persist) throw Error('Persistent storage is unavailable in this browser'); notice = await navigator.storage.persist() ? 'Persistent storage granted. Keep backups anyway.' : 'Persistence was not granted. Keep exported backups.'; })}>Request persistent storage</Button>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Titles and lists</strong><p class="text-foreground/85 max-w-md">Manage your saved titles in My Library, or organize device titles into lists.</p></div>
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
      <p>Import preview: {imported.source} · {imported.snapshot.records.length} records · {imported.collisions ?? 0} overlapping keys</p>
      {#if scopeMatches}
        <p>Merge keeps newer device saves and keeps your current version on equal timestamps. Replace removes the current device library. Neither option publishes.</p>
        <div class="flex flex-wrap gap-2"><Button disabled={!!pending} onclick={() => run('merge', async () => { const selected = imported!; await applyImport(selected, 'merge'); closeReview(false); notice = 'Backup merged locally.'; })}>Merge reviewed import</Button><Button variant="outline" disabled={!!pending} onclick={() => { if (confirm('Replace all device-library records with this reviewed backup? Export a backup first.')) void run('replace', async () => { const selected = imported!; await applyImport(selected, 'replace'); closeReview(false); notice = 'Device library replaced from the reviewed backup.'; }); }}>Replace with reviewed import</Button></div>
      {:else}<p role="alert">This backup belongs to another catalogue. Restore it on its original site. Portable protocol backups can be restored from Library recovery.</p>{/if}
      <Button variant="outline" onclick={() => closeReview(false)}>Cancel import</Button>
    {/if}
  </Dialog.Content>
</Dialog.Root>
