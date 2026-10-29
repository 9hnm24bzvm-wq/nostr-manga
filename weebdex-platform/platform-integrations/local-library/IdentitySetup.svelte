<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import { get } from 'svelte/store';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { generateLocalKeyDraft, importLocalKeyDraft, encodePublicKey } from './local-signing-key';
  import { connectSigner, signerIdentity, capabilities, assertAllowedRelay, admitAccount, relayPreferences, rememberRelay, type Relay } from './publication';
  import { proveService } from './protocol/nostr.js';
  import { enrollmentPolicy } from './enrollment-policy';

  let { onfinish, onprofile, assertCurrent = () => {}, finishLabel = 'Continue', allowFinishLater = true, resumeSelected = false }: {
    onfinish: () => void;
    onprofile?: (origin: string) => void;
    assertCurrent?: () => void;
    finishLabel?: string;
    allowFinishLater?: boolean;
    resumeSelected?: boolean;
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
  const title = $derived({ choose: 'Set up your public identity', import: 'Use your existing key', backup: 'Keep your key safe', review: enrollmentOpen ? 'Join this site' : 'Site approval needed', connected: 'Finish setting up your identity', done: 'You joined this site' }[stage]);
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
  function downloadRecoveryKey() {
    if (pending || !draft) return;
    const epoch = generation, selectedGeneration = selectionGeneration;
    const check = () => { assertActive(epoch); if (get(signerIdentity).generation !== selectedGeneration) throw Error('Identity changed'); };
    let url = '', link: HTMLAnchorElement | null = null;
    error = ''; notice = '';
    try {
      check();
      const file = new Blob([draft.exportSecret(), '\n'], { type: 'text/plain;charset=utf-8' });
      check(); url = URL.createObjectURL(file); check();
      link = document.createElement('a'); link.href = url; link.download = 'weebdex-recovery-key.txt';
      link.hidden = true; document.body.appendChild(link); check(); link.click(); check();
      notice = 'Download requested. Open the saved file to check it, then keep it somewhere private. Downloading does not verify your backup.';
    } catch {
      if (!disposed && epoch === generation) error = 'The recovery file could not be downloaded safely. Try again, or reveal and save your private key.';
    } finally {
      link?.remove(); if (url) URL.revokeObjectURL(url);
    }
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
    } catch { if (!disposed && epoch === generation) error = 'This site could not be verified right now. Your private key has not been sent. Try again, or return later with your saved recovery key.'; }
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
      if (!enrollment.enabled) { enrollmentOpen = false; notice = 'This site now needs approval from its team. No joining request was sent.'; return; }
      const selected = await activateReviewedKey(epoch);
      const assertSelected = () => { assertActive(epoch); const current = get(signerIdentity); if (current.owner !== expectedOwner || current.generation !== selected.generation) throw Error('Identity changed'); };
      await admitAccount(relay, assertSelected); assertSelected(); stage = 'done'; void focusStep();
    } catch { if (!disposed && epoch === generation) error = 'Joining could not be confirmed; the site may already have received your request. Keep this same recovery key. You can explicitly retry joining with it, or return later. Your library was not shared.'; }
    finally { if (!disposed && epoch === generation) pending = false; }
  }
  async function openProfile() {
    if (pending || !relay) return;
    const epoch = generation, destination = relay; pending = true; error = '';
    try {
      assertActive(epoch); const preferences = await relayPreferences(); assertActive(epoch);
      const saved = preferences.relays.find(row => row.origin === destination.origin);
      if (saved && saved.serviceKey !== destination.serviceKey) throw Error('The saved destination has a different service identity. Review it in Library settings first.');
      if (!saved) await rememberRelay(destination.origin, destination.serviceKey, preferences.revision, () => assertActive(epoch));
      assertActive(epoch); if (onprofile) onprofile(destination.origin); else { notice = 'Destination saved. Open Account → Advanced identity tools → Edit public profile.'; }
    } catch (cause) { if (!disposed && epoch === generation) error = cause instanceof Error ? cause.message : 'The destination could not be saved. Try again.'; }
    finally { if (!disposed && epoch === generation) pending = false; }
  }
  // Reuse the selected identity without importing, generating or signing again.
  // No new key or secret transfer occurs here. Reuse existing session authority;
  // this is not evidence that a backup exists and grants no additional site access.
  if (untrack(() => resumeSelected)) {
    const selected = get(signerIdentity);
    if (selected.owner) { identity = { publicKey: selected.owner, npub: encodePublicKey(selected.owner), source: selected.source === 'local' ? 'imported' : 'external' }; selectionGeneration = selected.generation; backupAcknowledged = true; installed = true; stage = 'connected'; }
  }
  onDestroy(() => { disposed = true; generation++; draft?.discard(); revealedSecret = ''; clearImport(); });
</script>

<section class="space-y-4" data-identity-setup aria-labelledby="identity-setup-title-{id}" aria-busy={pending}>
  <header class="space-y-2">
    <h2 id="identity-setup-title-{id}" bind:this={heading} tabindex="-1" class="text-lg font-semibold leading-tight">{title}</h2>
    {#if stage === 'choose'}
      <p class="text-muted-foreground text-sm">This optional identity is for publishing outside the site. Your normal account already handles ratings, comments, reports and follows. Connect an existing signer without giving this site your private key.</p>
    {/if}
  </header>
  {#if stage === 'choose'}
    <div class="flex flex-col gap-2">
      <Button class="min-h-11 w-full" disabled={pending} onclick={useExtension}>Use my browser signer</Button>
      <details><summary class="cursor-pointer text-sm">Advanced: create or import a private key</summary><div class="mt-3 space-y-2"><p class="text-sm">Most people do not need this. Keep using an existing signer if you already have an identity.</p><Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={openImport}>I already have a private key</Button><Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={createKey}>Create a new public identity</Button></div></details>
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
      <Button class="min-h-11 w-full" disabled={pending} onclick={downloadRecoveryKey}>Download recovery key</Button>
      <p class="text-muted-foreground text-sm">This file contains your private key. Anyone with it can use your identity. Keep it off shared devices and shared folders.</p>
      <div class="flex flex-wrap gap-2">
        <Button class="min-h-11" variant="outline" disabled={pending} onclick={reveal}>{revealedSecret ? 'Hide private key' : 'Reveal private key'}</Button>
        <Button class="min-h-11" variant="outline" disabled={pending} onclick={copyPrivate}>Copy private key</Button>
      </div>
      {#if revealedSecret}<Input aria-label="Private key for backup" value={revealedSecret} readonly autocomplete="off" data-1p-ignore data-lpignore="true" data-bwignore spellcheck={false} class="font-mono text-xs" />{/if}
      <p class="text-muted-foreground text-sm">Copying may leave the key in clipboard history or sync. Keep it private. This browser forgets it when you reload or lock it.</p>
    {/if}
    <details class="space-y-2"><summary class="cursor-pointer text-sm">Public identity details</summary>
      <p class="text-sm font-medium">Your public key</p>
      <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
      <Button class="min-h-11" variant="outline" disabled={pending} onclick={copyPublic}>Copy public key</Button>
      <p class="text-muted-foreground text-sm">You can share this public key. It does not contain your private key.</p>
    </details>
    <label class="flex items-start gap-3 text-sm"><input class="mt-1 size-4 shrink-0" type="checkbox" bind:checked={backupAcknowledged} disabled={pending} /> <span>{identity.source === 'external' ? 'I manage my private-key backup in my signer.' : 'I have saved my private key somewhere safe.'}</span></label>
    <p class="text-muted-foreground text-xs">This acknowledgement does not create or verify a backup for you.</p>
    <p class="text-sm">Next: review joining this site. This optional public-site access is separate from your normal account; reading, ratings and local comments already use your ordinary login.</p>
    <Button class="min-h-11 w-full" disabled={pending || !backupAcknowledged} onclick={reviewInstance}>Continue to site access</Button>
    <details><summary class="cursor-pointer text-sm">Connect without joining (advanced)</summary>
      <p class="mt-2 text-muted-foreground text-sm">For an already approved identity or when this site is unavailable. Connecting locally does not request admission or publish anything; site access is not confirmed.</p>
      <Button class="mt-2 min-h-11 w-full" variant="outline" disabled={pending || !backupAcknowledged} onclick={connectSession}>Use this key for this session</Button>
    </details>
    <Button class="min-h-11 w-full" variant="ghost" disabled={pending} onclick={startAgain}>Discard setup and go back</Button>
  {:else if stage === 'review' && identity && relay}
    <details class="space-y-2"><summary class="cursor-pointer text-sm">Public identity details</summary>
      <p class="text-sm font-medium">Your public key</p>
      <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
      <Button class="min-h-11" variant="outline" disabled={pending} onclick={copyPublic}>Copy public key</Button>
    </details>
    {#if enrollmentOpen}
    <p class="text-sm">Join <span class="break-all font-medium">{relay.origin}</span> with this identity?</p>
    <p class="text-muted-foreground text-sm">This sends a signed joining request using your public key. Your private recovery key stays with you. Your email login stays separate; your saved manga and reading history are not shared. The site may still need to link or approve your account for some actions.</p>
    <Button class="min-h-11 w-full" disabled={pending} onclick={join}>{pending ? 'Joining…' : 'Join this site'}</Button>
    {:else}
      <p class="text-sm font-medium" role="status">This site needs approval from its team</p>
      <p class="text-muted-foreground text-sm">This site is not accepting new identities automatically. Ask its team to approve your public key below. An already-approved key can still connect below; approval is not checked here. Your email login is separate.</p>
      {#if !allowFinishLater}<Button class="min-h-11 w-full" variant="outline" onclick={finishLater}>Done for now</Button>{/if}
    {/if}
    <Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={connectSession}>Use this key for this session</Button>
    <p class="text-muted-foreground text-xs">Connecting locally does not join the site or share anything. It does not link this identity to your email account.</p>
    <Button class="min-h-11 w-full" variant="ghost" disabled={pending} onclick={startAgain}>Choose a different key</Button>
    {#if pending}<p class="text-muted-foreground text-xs">Leaving now cannot undo a request the instance has already received.</p>{/if}
  {:else if stage === 'connected' && identity}
    <p class="text-sm">Your recovery key is connected, but site access has not been checked. Review joining below to continue setup with this same identity.</p>
    <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
    <Button class="min-h-11" variant="outline" onclick={copyPublic}>Copy public key</Button>
    <p class="text-muted-foreground text-sm">Your email login stays separate. Nothing was published. {identity.source === 'external' ? 'Your browser signer keeps the private key.' : 'After reloading or locking, paste your saved private key to reconnect.'}</p>
    <Button class="min-h-11 w-full" disabled={pending} onclick={reviewInstance}>Review joining this site</Button>
    <Button class="min-h-11 w-full" variant="outline" disabled={pending} onclick={onfinish}>Return without joining</Button>
  {:else if stage === 'done' && identity && relay}
    <p class="text-sm">Your joining request was accepted by <span class="break-all">{relay.origin}</span>. Return to your task to check the account access it needs. Joining does not automatically link an email account or grant every permission. Your saved manga and reading history were not shared.</p>
    <Input aria-label="Your public key" value={identity.npub} readonly spellcheck={false} class="font-mono text-xs" />
    <Button class="min-h-11" variant="outline" onclick={copyPublic}>Copy public key</Button>
    <p class="text-muted-foreground text-sm">You can add a public name or profile later in Account → Advanced identity tools → Public profile. {identity.source === 'external' ? 'Reconnect your signer next time.' : 'After reloading or locking this session, paste your saved private key to use this identity again.'}</p>
    <p class="text-muted-foreground text-sm">Use this verified site for your profile and public actions. This only remembers a destination; it does not publish or share your library.</p>
    <Button class="min-h-11 w-full" disabled={pending} onclick={openProfile}>Use this site for my public profile</Button>
    <Button class="min-h-11 w-full" variant="outline" onclick={onfinish}>{finishLabel}</Button>
  {/if}
  {#if pending}<p class="text-muted-foreground text-sm" role="status">Working…</p>{/if}
  {#if error}<p role="alert" class="text-destructive text-sm">{error}</p>{/if}
  {#if notice}<p role="status" class="text-muted-foreground text-sm">{notice}</p>{/if}
  {#if allowFinishLater && stage !== 'done' && stage !== 'connected'}<Button class="min-h-11 w-full" variant="ghost" onclick={finishLater}>Finish later</Button>{/if}
</section>
