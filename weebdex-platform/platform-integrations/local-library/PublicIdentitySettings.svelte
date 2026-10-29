<script lang="ts">
  import { onMount } from 'svelte';
  import { Session } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import { connectSigner, disconnectSigner, signerIdentity } from './publication';
  import { message } from './library';
  import ProtocolIdentityPanel from './ProtocolIdentityPanel.svelte';
  import IdentitySetup from './IdentitySetup.svelte';
  import NativeIdentityLink from './NativeIdentityLink.svelte';
  let view = $state<'profile' | 'follows' | 'actions' | 'connections' | 'keys' | 'lookup'>('profile'), open = $state(false), pending = $state(false), error = $state('');
  let disposed = false, viewGeneration = $state(0);
  let selectedOrigin = $state(''), resumeSelected = $state(true);
  let advanced = $state<HTMLDetailsElement | null>(null), advancedOpen = $state(false);
  const title = $derived({ profile: 'Public profile', follows: 'Public follows', actions: 'Saved public actions', connections: 'Identity & instances', keys: 'Public identity', lookup: 'Find a public profile' }[view]);
  function setOpen(value: boolean) { viewGeneration++; open = value; }
  function edit(next: typeof view, resume = true) { if (!advanced?.open) return; resumeSelected = resume; view = next; setOpen(true); }
  function viewGuard(generation: number) { return () => { if (disposed || !advanced?.open || !open || generation !== viewGeneration) throw Error('This identity task was closed. Reopen it and review again.'); }; }
  async function connect() {
    if (pending || disposed || !advanced?.open) return; pending = true; error = '';
    try { await connectSigner(() => { if (disposed || !advanced?.open) throw Error('Identity settings were closed. Connect again from the current page.'); }); }
    catch (cause) { if (!disposed) error = message(cause); }
    finally { if (!disposed) pending = false; }
  }
  onMount(() => () => { disposed = true; });
</script>
<section class="space-y-5" data-account-settings>
  <header class="space-y-1"><h1 class="text-lg font-semibold">Account</h1><p class="text-muted-foreground text-sm">One account for your reading and community activity.</p></header>
  {#if $Session?.identity?.id}
    <p class="text-sm">Signed in as <strong>{$Session.identity.username}</strong>.</p>
    <div class="flex flex-wrap gap-2">
      <Button href="/settings/account">Manage account</Button>
      <Button variant="outline" href={'/user/' + encodeURIComponent($Session.identity.id)}>My profile</Button>
      <Button variant="outline" href="/auth/logout">Sign out</Button>
    </div>
    {#if !$Session.identity.is_verified}<p class="text-sm">Finish setting up your account: <a class="underline" href="/auth/verification">verify your email</a>.</p>{/if}
  {:else}
    <p class="text-sm">Sign in to use your account across the site. You can keep browsing without signing in.</p>
    <div class="flex flex-wrap gap-2"><Button href="/auth/login">Sign in</Button><Button variant="outline" href="/auth/register">Create account</Button></div>
  {/if}
  <details bind:this={advanced} class="border-t pt-4" data-advanced-identity ontoggle={() => { advancedOpen = !!advanced?.open; if (!advanced?.open && open) setOpen(false); }}>
    <summary class="cursor-pointer text-sm">Advanced identity tools</summary>
    <section class="mt-4 space-y-5" data-protocol-identity-panel>

  <header class="space-y-1"><h2 class="text-lg font-semibold">Portable identity</h2><p class="text-muted-foreground text-sm">Advanced: connect a portable identity for publishing outside this site. Your normal account already works for reading, ratings, comments, reports and follows. You do not need a recovery key for those actions.</p></header>
  {#if advancedOpen && $Session?.identity?.id}{#key $Session.identity.id}<NativeIdentityLink account={$Session.identity.id} assertViewCurrent={() => { if (!advanced?.open || disposed) throw Error('Account identity tools were closed. Reopen them before continuing.'); }} />{/key}{/if}
  <section class="space-y-3">
    <p class="text-sm">{$signerIdentity.owner ? 'Your portable identity is selected for this session. This does not confirm that the site has approved or linked your account.' : 'No public identity selected for this session. Connect an existing identity below if you want to use one.'}</p>
    <Button disabled={pending} onclick={() => edit('keys')}>{$signerIdentity.owner ? 'Review site access' : 'Connect an existing identity'}</Button>
    {#if $signerIdentity.owner}<Button variant="outline" onclick={() => edit('keys', false)}>Use a different identity</Button><Button variant="outline" onclick={disconnectSigner}>{$signerIdentity.source === 'local' ? 'Lock identity' : 'Disconnect identity'}</Button>{/if}
    <p class="text-muted-foreground text-sm">Joining requires your approval. It does not post anything or grant every permission. Use the connection panel above to link it deliberately to this account. Site access and publishing permissions remain separate.</p>
    <details><summary class="cursor-pointer text-sm">Advanced connection options</summary><div class="mt-3 flex gap-2"><Button variant="outline" disabled={pending} onclick={connect}>Connect browser extension</Button><Button variant="ghost" onclick={() => edit('connections')}>Connection settings</Button></div></details>
  </section>
  <section class="space-y-3 border-t pt-4">
    <div class="space-y-1"><strong class="block font-semibold">Public profile</strong><p class="text-foreground/85 max-w-md">Choose the name and bio shown with your public identity.</p></div>
    <div class="flex flex-wrap gap-2"><Button variant="outline" onclick={() => edit('profile')}>Edit public profile</Button><Button variant="ghost" onclick={() => edit('lookup')}>Find a public profile (read-only)</Button></div>
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
      <p class="text-sm"><a class="underline" href="/account/closure">Disable this site account</a>. This stops normal account access to this site. It does not erase your portable identity or already-public posts; review the account consequences before confirming.</p>
    </section>
  </details>
</section>
<Dialog.Root {open} onOpenChange={setOpen}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] !max-w-[min(42rem,calc(100dvw-2rem))] overflow-y-auto">
    <Dialog.Header><Dialog.Title>{title}</Dialog.Title></Dialog.Header>
    {#if open}{#key viewGeneration}{#if view === 'keys'}<IdentitySetup {resumeSelected} onprofile={(origin) => { selectedOrigin = origin; view = 'profile'; viewGeneration++; }} onfinish={() => setOpen(false)} assertCurrent={viewGuard(viewGeneration)} finishLabel="Done" allowFinishLater={false} />{:else}<ProtocolIdentityPanel bind:selectedOrigin settingsView={view} assertViewCurrent={viewGuard(viewGeneration)} />{/if}{/key}{/if}
  </Dialog.Content>
</Dialog.Root>
