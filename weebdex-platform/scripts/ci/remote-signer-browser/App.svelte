<script lang="ts">
  // Isolated acceptance shell, not a replacement Account route.
  import { onMount } from 'svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import RemoteSignerPanel from '@owned/RemoteSignerPanel.svelte';
  import RemoteSignerStatus from '@owned/RemoteSignerStatus.svelte';
  import { Session, signerIdentity, startSyntheticSession } from './boundaries';
  let open = $state(false), generation = $state(0);
  let fixtureError = $state('');
  const check = (captured: number) => () => { if (!open || captured !== generation) throw Error('Synthetic Account dialog closed.'); };
  function setOpen(value: boolean) { generation++; open = value; }
  async function fillOwnedLink() {
    try {
      const response = await fetch('/fixture/public-link', { credentials: 'omit', cache: 'no-store', redirect: 'error' });
      if (!response.ok) throw Error();
      const value = await response.json(), input = document.getElementById('remote-bunker-link');
      if (!(input instanceof HTMLInputElement) || typeof value.link !== 'string') throw Error();
      input.value = value.link; input.dispatchEvent(new Event('input', { bubbles: true }));
    } catch { fixtureError = 'Owned synthetic connection input unavailable.'; }
  }
  onMount(startSyntheticSession);
</script>
<h1>Owned synthetic remote-signer focus fixture</h1>
<p>No real account, external relay or public event delivery is available here.</p>
<p data-synthetic-account>Account: {$Session.identity.id}</p>
<p data-synthetic-selection>Selected: {$signerIdentity.owner ? 'yes' : 'no'}</p>
<button onclick={() => setOpen(true)}>Open synthetic Account connection</button>
<Dialog.Root {open} onOpenChange={setOpen}>
  <Dialog.Content data-remote-parent-dialog>
    <Dialog.Header><Dialog.Title>Connect remote signer</Dialog.Title><Dialog.Description>Synthetic Account shell with the actual native Dialog primitives.</Dialog.Description></Dialog.Header>
    <button onclick={fillOwnedLink}>Fill owned synthetic public connection link</button>
    {#if fixtureError}<p role="alert">{fixtureError}</p>{/if}
    {#if open}{#key generation}<RemoteSignerPanel assertCurrent={check(generation)} />{/key}{/if}
  </Dialog.Content>
</Dialog.Root>
<RemoteSignerStatus />

<style>
  /* Fixture geometry only. Not production CSS/pixel-parity evidence. */
  :global(body) { font: 16px system-ui; margin: 2rem; }
  :global(button), :global(input), :global(a) { margin: .25rem; padding: .5rem; }
  :global([data-slot='dialog-overlay']) { position: fixed; inset: 0; background: #0006; }
  :global([data-slot='dialog-content']) { position: fixed; left: 10%; top: 8%; width: 75%; max-height: 80vh; overflow: auto; background: white; border: 2px solid; padding: 1rem; }
  :global([data-remote-signer-status]) { top: 20%; left: 20%; width: 55%; }
  :global(:focus-visible) { outline: 3px solid blue; }
</style>
