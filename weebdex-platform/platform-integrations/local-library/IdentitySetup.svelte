<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { get } from 'svelte/store';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { generateLocalKeyDraft, importLocalKeyDraft, encodePublicKey } from './local-signing-key';
  import { connectSigner, signerIdentity, capabilities, assertAllowedRelay, admitAccount, type Relay } from './publication';
  import { proveService } from './protocol/nostr.js';
  import { enrollmentPolicy } from './enrollment-policy';

  let { onfinish, assertCurrent = () => {}, finishLabel = 'Continue', allowFinishLater = true }: {
    onfinish: () => void;
    assertCurrent?: () => void;
    finishLabel?: string;
    allowFinishLater?: boolean;
  } = $props();
  const id = $props.id();
  let stage = $state<'choose' | 'import' | 'backup' | 'review' | 'connected' | 'done'>('choose');
  let draft = $state.raw<ReturnType<typeof generateLocalKeyDraft> | null>(null);
  let identity = $state<{ publicKey: string; npub: string; source: 'generated' | 'imported' | 'external' } | null>(null);
  let relay = $state<Relay | null>(null);
  let enrollmentOpen = $state(false);
  let importInput = $state<HTMLInputElement | null>(null);
  let heading = $state<HTMLHeadingElement>();
  let revealedSecret = $state('');
  let backupAcknowledged = $state(false), pending = $state(false), error = $state(''), notice = $state('');
  let disposed = false, generation = 0, installed = false, selectionGeneration = 0;
  const title = $derived({ choose: 'Set up your public identity', import: 'Use your existing key', backup: 'Keep your key safe', review: enrollmentOpen ? 'Join this instance' : 'Instance approval needed', connected: 'Key connected for this session', done: 'Your public identity is ready' }[stage]);
  const assertActive = (epoch = generation) => { if (disposed || epoch !== generation) throw Error('Identity setup is no longer current'); assertCurrent(); };
  async function focusStep() { const epoch = generation; await tick(); try { assertActive(epoch); heading?.focus({ preventScroll: true }); } catch { /* Closed or replaced setup must not steal focus. */ } }
  function clearImport() { if (importInput) { importInput.value = ''; importInput.dispatchEvent(new Event('input', { bubbles: true })); } }
  function reset() {
    generation++; draft?.discard(); draft = null; identity = null; relay = null; enrollmentOpen = false;
    revealedSecret = ''; backupAcknowledged = false; pending = false; error = ''; notice = ''; installed = false; selectionGeneration = get(signerIdentity).generation; clearImport();
  }
  function finishLater() { reset(); stage = 'choose'; onfinish(); }
  function startAgain() { reset(); stage = 'choose'; void focusStep(); }
  function createKey() {
    if (pending) return;
    reset(); const epoch = generation;
    try { assertActive(epoch); draft = generateLocalKeyDraft(() => assertActive(epoch)); identity = { publicKey: draft.publicKey, npub: draft.npub, source: 'generated' }; stage = 'backup'; void focusStep(); }
    catch { error = 'A key could not be created securely in this browser. Try again or use a browser signer.'; }
  }
  function openImport() { if (pending) return; reset(); stage = 'import'; void focusStep(); }
  function importKey(event?: { preventDefault(): void }) {
    event?.preventDefault(); if (pending || !importInput) return;
    const epoch = generation; error = ''; notice = '';
    try {
      assertActive(epoch); draft = importLocalKeyDraft(importInput.value, () => assertActive(epoch));
      identity = { publicKey: draft.publicKey, npub: draft.npub, source: 'imported' };
      clearImport(); stage = 'backup'; void focusStep();
    } catch { clearImport(); error = 'Paste an nsec private key or a valid 64-character private key. An npub is public and cannot sign in.'; }
  }
  async function useExtension() {
    if (pending) return;
    reset(); pending = true; const epoch = generation;
    try { await connectSigner(() => assertActive(epoch)); assertActive(epoch); const owner = get(signerIdentity).owner; if (!owner) throw Error('No selected identity'); identity = { publicKey: owner, npub: encodePublicKey(owner), source: 'external' }; selectionGeneration = get(signerIdentity).generation; stage = 'backup'; void focusStep(); }
    catch { if (!disposed && epoch === generation) error = 'The browser signer did not connect. You can try again, create a key, or paste one.'; }
    finally { if (!disposed && epoch === generation) pending = false; }
  }
  function reveal() {
    if (pending || !draft) return;
    try { assertActive(); revealedSecret = revealedSecret ? '' : draft.exportSecret(); error = ''; }
    catch { revealedSecret = ''; error = 'This key is no longer available. Start again or import your saved key.'; }
  }
  async function copyPrivate() {
    if (pending || !draft) return;
    const epoch = generation; error = ''; notice = '';
    try { assertActive(epoch); if (!navigator.clipboard?.writeText) throw Error('Clipboard unavailable'); await navigator.clipboard.writeText(draft.exportSecret()); assertActive(epoch); notice = 'Private key copied. Save it somewhere safe; clipboard history or sync may keep a copy.'; }
    catch { if (!disposed && epoch === generation) error = 'Could not copy. Reveal the private key and copy it yourself.'; }
  }
  async function copyPublic() {
    if (!identity || pending) return;
    const epoch = generation; error = ''; notice = '';
    try { assertActive(epoch); if (!navigator.clipboard?.writeText) throw Error('Clipboard unavailable'); await navigator.clipboard.writeText(identity.npub); assertActive(epoch); notice = 'Public key copied.'; }
    catch { if (!disposed && epoch === generation) error = 'Could not copy. Select the public key and copy it yourself.'; }
  }
  async function reviewInstance() {
    if (pending || !identity || !backupAcknowledged) return;
    const epoch = generation; pending = true; error = ''; notice = ''; revealedSecret = '';
    try {
      assertActive(epoch); const policy = await capabilities(); assertActive(epoch);
      if (!policy.available || !policy.origin || !policy.service_key) throw Error('Instance unavailable');
      const canonical = assertAllowedRelay(policy.origin, policy);
      const verified = await proveService(canonical, policy.service_key); assertActive(epoch);
      const recipient = { ...verified, selected: false };
      const enrollment = await enrollmentPolicy(recipient, () => assertActive(epoch)); assertActive(epoch);
      relay = recipient; enrollmentOpen = enrollment.enabled; stage = 'review'; void focusStep();
    } catch { if (!disposed && epoch === generation) error = 'This instance could not be verified right now. Your key has not been sent. Try again, or finish later with your saved key.'; }
    finally { if (!disposed && epoch === generation) pending = false; }
  }
  async function activateReviewedKey(epoch: number) {
    assertActive(epoch);
    if (!identity || !backupAcknowledged) throw Error('Review and back up this key first');
    const expectedOwner = identity.publicKey;
    if (get(signerIdentity).generation !== selectionGeneration) throw Error('Identity changed');
    if (draft && !installed) { await draft.activate(() => assertActive(epoch)); installed = true; selectionGeneration = get(signerIdentity).generation; }
    assertActive(epoch); const selected = get(signerIdentity);
    if (selected.owner !== expectedOwner) throw Error('Identity changed');
    return selected;
  }
  async function connectSession() {
    if (pending || !identity || !backupAcknowledged) return;
    const epoch = generation; pending = true; error = ''; notice = ''; revealedSecret = '';
    try { await activateReviewedKey(epoch); assertActive(epoch); stage = 'connected'; void focusStep(); }
    catch { if (!disposed && epoch === generation) error = 'This key could not be connected. Start again or import your saved key.'; }
    finally { if (!disposed && epoch === generation) pending = false; }
  }
  async function join() {
    if (pending || !identity || !relay || !backupAcknowledged || !enrollmentOpen) return;
    const epoch = generation, expectedOwner = identity.publicKey; pending = true; error = ''; notice = ''; revealedSecret = '';
    try {
      assertActive(epoch);
      const enrollment = await enrollmentPolicy(relay, () => assertActive(epoch)); assertActive(epoch);
      if (!enrollment.enabled) { enrollmentOpen = false; notice = 'This instance now needs operator approval. Your key has not been submitted for admission.'; return; }
      const selected = await activateReviewedKey(epoch);
      const assertSelected = () => { assertActive(epoch); const current = get(signerIdentity); if (current.owner !== expectedOwner || current.generation !== selected.generation) throw Error('Identity changed'); };
      await admitAccount(relay, assertSelected); assertSelected(); stage = 'done'; void focusStep();
    } catch { if (!disposed && epoch === generation) error = 'Joining could not be confirmed. Your saved key still works. You can retry the same key explicitly, or finish later. Your library was not published.'; }
    finally { if (!disposed && epoch === generation) pending = false; }
  }
  onDestroy(() => { disposed = true; generation++; draft?.discard(); revealedSecret = ''; clearImport(); });
</script>

<section class="space-y-4" data-identity-setup aria-labelledby="identity-setup-title-{id}" aria-busy={pending}>
  <header class="space-y-2">
    <h2 id="identity-setup-title-{id}" bind:this={heading} tabindex="-1" class="text-lg font-semibold leading-tight">{title}</h2>
    {#if stage === 'choose'}
      <p class="text-muted-foreground text-sm">Your public identity lets you use Nostr features across compatible apps. It uses its own key, separate from your site password.</p>
    {/if}
  </header>
  {#if stage === 'choose'}
    <div class="flex flex-col gap-2">
      <Button class="min-h-11 w-full" disabled={pending} onclick={createKey}>Create a new key</Button>
      <Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={openImport}>Paste an existing key</Button>
      <Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={useExtension}>Use a browser signer</Button>
    </div>
    <p class="text-muted-foreground text-sm">Private keys stay in this browser session. Nothing is published during key setup.</p>
  {:else if stage === 'import'}
    <div class="space-y-3">
      <label class="block space-y-2 text-sm font-medium" for="identity-private-key-{id}">Private key (nsec or hex)</label>
      <Input bind:ref={importInput} id="identity-private-key-{id}" type="password" autocomplete="off" autocapitalize="off" spellcheck={false} data-1p-ignore data-lpignore="true" data-bwignore disabled={pending} onkeydown={(event) => { if (event.key === 'Enter') { event.preventDefault(); importKey(); } }} />
      <p class="text-muted-foreground text-sm">Paste only on a device you trust. This key is read locally and is never sent to the server.</p>
      <Button class="min-h-11 w-full" type="button" disabled={pending} onclick={() => importKey()}>Use this key</Button>
      <Button class="min-h-11 w-full" type="button" variant="ghost" disabled={pending} onclick={startAgain}>Back</Button>
    </div>
  {:else if stage === 'backup' && identity}
    {#if identity.source === 'external'}
      <p class="text-sm">Your browser signer keeps the private key. Make sure you have backed it up using that signer.</p>
    {:else}
      <p class="text-sm">Your private key controls this identity. Save a private copy somewhere safe before continuing.</p>
      <p class="text-destructive text-sm">If you lose it, this site cannot recover it. Resetting your site password will not restore this key.</p>
      <div class="flex flex-wrap gap-2">
        <Button class="min-h-11" variant="outline" disabled={pending} onclick={reveal}>{revealedSecret ? 'Hide private key' : 'Reveal private key'}</Button>
        <Button class="min-h-11" variant="outline" disabled={pending} onclick={copyPrivate}>Copy private key</Button>
      </div>
      {#if revealedSecret}<Input aria-label="Private key for backup" value={revealedSecret} readonly autocomplete="off" data-1p-ignore data-lpignore="true" data-bwignore spellcheck={false} class="font-mono text-xs" />{/if}
      <p class="text-muted-foreground text-sm">Copying may leave the key in clipboard history or sync. Keep it private. This browser forgets it when you reload or lock it.</p>
    {/if}
    <div class="space-y-2">
      <p class="text-sm font-medium">Your public key</p>
      <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
      <Button class="min-h-11" variant="outline" disabled={pending} onclick={copyPublic}>Copy public key</Button>
      <p class="text-muted-foreground text-sm">You can share this public key. It does not contain your private key.</p>
    </div>
    <label class="flex items-start gap-3 text-sm"><input class="mt-1 size-4 shrink-0" type="checkbox" bind:checked={backupAcknowledged} disabled={pending} /> <span>{identity.source === 'external' ? 'I manage my private-key backup in my signer.' : 'I have saved my private key somewhere safe.'}</span></label>
    <p class="text-muted-foreground text-xs">This acknowledgement does not create or verify a backup for you.</p>
    <Button class="min-h-11 w-full" variant="outline" disabled={pending || !backupAcknowledged} onclick={connectSession}>Use this key for this session</Button>
    <p class="text-muted-foreground text-xs">Connecting locally does not request admission or publish anything. Instance discovery can wait until you choose a public action.</p>
    <Button class="min-h-11 w-full" disabled={pending || !backupAcknowledged} onclick={reviewInstance}>Review public identity</Button>
    <Button class="min-h-11 w-full" variant="ghost" disabled={pending} onclick={startAgain}>Discard setup and go back</Button>
  {:else if stage === 'review' && identity && relay}
    <div class="space-y-2">
      <p class="text-sm font-medium">Your public key</p>
      <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
      <Button class="min-h-11" variant="outline" disabled={pending} onclick={copyPublic}>Copy public key</Button>
    </div>
    {#if enrollmentOpen}
    <p class="text-sm">Join <span class="break-all font-medium">{relay.origin}</span> with this public key?</p>
    <p class="text-muted-foreground text-sm">This proves you control the key and requests a personal account on this instance. Your site login stays separate. Your saved manga and reading history are not published.</p>
    <Button class="min-h-11 w-full" disabled={pending} onclick={join}>{pending ? 'Joining…' : 'Join this instance'}</Button>
    {:else}
      <p class="text-sm font-medium" role="status">This instance needs operator approval</p>
      <p class="text-muted-foreground text-sm">You can keep and back up this key, but this instance is not accepting new public-key accounts. Ask its operator to approve your public key. An already-approved key can still connect below; approval is not checked here. Your site login still works.</p>
      {#if !allowFinishLater}<Button class="min-h-11 w-full" variant="outline" onclick={finishLater}>Done for now</Button>{/if}
    {/if}
    <Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={connectSession}>Use this key for this session</Button>
    <p class="text-muted-foreground text-xs">Connecting locally does not request admission or publish anything. This does not link the key to your site-password account.</p>
    <Button class="min-h-11 w-full" variant="ghost" disabled={pending} onclick={startAgain}>Choose a different key</Button>
    {#if pending}<p class="text-muted-foreground text-xs">Leaving now cannot undo a request the instance has already received.</p>{/if}
  {:else if stage === 'connected' && identity}
    <p class="text-sm">This key is connected in this browser session. Instance approval has not been confirmed. Public actions still need this instance's permission.</p>
    <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
    <Button class="min-h-11" variant="outline" onclick={copyPublic}>Copy public key</Button>
    <p class="text-muted-foreground text-sm">Your site login stays separate. Nothing was published. {identity.source === 'external' ? 'Your browser signer keeps the private key.' : 'After reloading or locking, paste your saved private key to reconnect.'}</p>
    <Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={reviewInstance}>Review joining this instance</Button>
    <Button class="min-h-11 w-full" onclick={onfinish}>{finishLabel}</Button>
  {:else if stage === 'done' && identity && relay}
    <p class="text-sm">Your public key was accepted by <span class="break-all">{relay.origin}</span>. Your saved manga and reading history were not published during setup.</p>
    <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
    <Button class="min-h-11" variant="outline" onclick={copyPublic}>Copy public key</Button>
    <p class="text-muted-foreground text-sm">You can add a public name or profile later in Settings → Public identity. {identity.source === 'external' ? 'Reconnect your signer next time.' : 'After reloading or locking this session, paste your saved private key to use this identity again.'}</p>
    <Button class="min-h-11 w-full" onclick={onfinish}>{finishLabel}</Button>
  {/if}
  {#if pending}<p class="text-muted-foreground text-sm" role="status">Working…</p>{/if}
  {#if error}<p role="alert" class="text-destructive text-sm">{error}</p>{/if}
  {#if notice}<p role="status" class="text-muted-foreground text-sm">{notice}</p>{/if}
  {#if allowFinishLater && stage !== 'done' && stage !== 'connected'}<Button class="min-h-11 w-full" variant="ghost" onclick={finishLater}>Finish later</Button>{/if}
</section>
