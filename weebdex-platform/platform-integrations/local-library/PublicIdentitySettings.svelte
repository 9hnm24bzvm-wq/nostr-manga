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
  const title = $derived({ profile: 'Public profile', follows: 'Public follows', actions: 'Saved public actions', connections: 'Identity & instances', keys: 'Public identity' }[view]);
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
  <header class="space-y-1"><h1 class="text-lg font-semibold">Public identity</h1><p class="text-muted-foreground text-sm">One place to manage your identity for ratings, uploads and some public actions. You can browse and keep a private library without it. Your email login is separate.</p></header>
  <section class="space-y-3">
    <p class="text-sm">{$signerIdentity.owner ? 'Your public identity is connected for this session.' : 'No public identity connected yet. The guided setup will help you create one or reconnect an existing one.'}</p>
    <Button disabled={pending} onclick={() => edit('keys')}>{$signerIdentity.owner ? 'Connect a different identity' : 'Set up public identity'}</Button>
    {#if $signerIdentity.owner}<Button variant="outline" onclick={disconnectSigner}>{$signerIdentity.source === 'local' ? 'Lock identity' : 'Disconnect identity'}</Button>{/if}
    <p class="text-muted-foreground text-sm">Setup does not post anything or grant upload permissions. A public identity may use a different account from your email login.</p>
    <details><summary class="cursor-pointer text-sm">Advanced connection options</summary><div class="mt-3 flex gap-2"><Button variant="outline" disabled={pending} onclick={connect}>Connect browser extension</Button><Button variant="ghost" onclick={() => edit('connections')}>Connection settings</Button></div></details>
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

<p class="mt-4"><a class="underline" href="/account/closure">Deactivate public account</a>. This does not delete your identity or private library.</p>
