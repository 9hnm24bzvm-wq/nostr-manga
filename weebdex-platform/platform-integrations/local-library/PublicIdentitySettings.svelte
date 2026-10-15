<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import { connectSigner, disconnectSigner, signerIdentity } from './publication';
  import { message } from './library';
  import ProtocolIdentityPanel from './ProtocolIdentityPanel.svelte';
  import IdentitySetup from './IdentitySetup.svelte';
  let view = $state<'profile' | 'follows' | 'actions' | 'connections' | 'keys'>('profile'), open = $state(false), pending = $state(false), error = $state('');
  let disposed = false, viewGeneration = $state(0);
  let selectedOrigin = $state('');
  const title = $derived({ profile: 'Public profile', follows: 'Public follows', actions: 'Saved public actions', connections: 'Identity & instances', keys: 'Set up your public identity' }[view]);
  function setOpen(value: boolean) { viewGeneration++; open = value; }
  function edit(next: typeof view) { view = next; setOpen(true); }
  function viewGuard(generation: number) { return () => { if (disposed || !open || generation !== viewGeneration) throw Error('This identity task was closed. Reopen it and review again.'); }; }
  async function connect() {
    if (pending || disposed) return; pending = true; error = '';
    try { await connectSigner(() => { if (disposed) throw Error('Identity settings were closed. Connect again from the current page.'); }); }
    catch (cause) { if (!disposed) error = message(cause); }
    finally { if (!disposed) pending = false; }
  }
  onMount(() => () => { disposed = true; });
</script>
<section class="space-y-5" data-protocol-identity-panel>
  <header class="space-y-1"><h1 class="text-lg font-semibold">Public identity</h1><p class="text-muted-foreground text-sm">An optional identity for public comments, reactions and profiles. Your site login and private library are separate.</p></header>
  <section class="space-y-3">
    <div class="space-y-1"><strong class="block font-semibold">Signing identity</strong><p class="text-foreground/85 max-w-md">{$signerIdentity.owner ? ($signerIdentity.source === 'local' ? 'Session private key connected' : 'Browser signer connected') : 'No signing identity connected'}. Create or paste a private key for this browser session, or use a browser signing extension.</p></div>
    <div class="flex flex-wrap gap-2"><Button variant="outline" disabled={pending} onclick={() => edit('keys')}>Create or paste a key</Button><Button variant="outline" disabled={pending} onclick={connect}>Connect external signer</Button><Button variant="outline" disabled={pending || !$signerIdentity.owner} onclick={disconnectSigner}>{$signerIdentity.source === 'local' ? 'Lock private key' : 'Disconnect signer'}</Button><Button variant="ghost" onclick={() => edit('connections')}>Connection settings</Button></div>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Public profile</strong><p class="text-foreground/85 max-w-md">Choose the name and bio shown with your public identity.</p></div>
    <Button variant="outline" onclick={() => edit('profile')}>Edit public profile</Button>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Public follows</strong><p class="text-foreground/85 max-w-md">Manage people you follow and where public posts are shared.</p></div>
    <Button variant="outline" onclick={() => edit('follows')}>Manage follows</Button>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Saved actions</strong><p class="text-foreground/85 max-w-md">Check deliveries or retry an interrupted public post.</p></div>
    <Button variant="outline" onclick={() => edit('actions')}>Saved public actions</Button>
  </section>
  {#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
</section>
<Dialog.Root {open} onOpenChange={setOpen}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] !max-w-[min(42rem,calc(100dvw-2rem))] overflow-y-auto">
    <Dialog.Header><Dialog.Title>{title}</Dialog.Title></Dialog.Header>
    {#if open}{#key viewGeneration}{#if view === 'keys'}<IdentitySetup onfinish={() => setOpen(false)} assertCurrent={viewGuard(viewGeneration)} finishLabel="Done" allowFinishLater={false} />{:else}<ProtocolIdentityPanel bind:selectedOrigin settingsView={view} assertViewCurrent={viewGuard(viewGeneration)} />{/if}{/key}{/if}
  </Dialog.Content>
</Dialog.Root>
