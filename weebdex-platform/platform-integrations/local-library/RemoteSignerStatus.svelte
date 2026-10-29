<script lang="ts">
  import { remoteSignerStatus, cancelRemoteSigner } from './remote-signer-session';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
</script>
<Dialog.Root open={$remoteSignerStatus.pending} onOpenChange={(value) => { if (!value) cancelRemoteSigner(); }}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] !max-w-xl overflow-y-auto" data-remote-signer-status>
    <Dialog.Header><Dialog.Title>Remote signer approval</Dialog.Title><Dialog.Description>Approve only the public change you reviewed. Nothing is retried automatically.</Dialog.Description></Dialog.Header>
    <p role="status">{$remoteSignerStatus.message}</p>
    {#if $remoteSignerStatus.authURL}<p class="text-sm">The signer asks you to visit <strong>{new URL($remoteSignerStatus.authURL).hostname}</strong>. Check that you trust this domain. Opening it may share information with that site.</p><a class="underline" href={$remoteSignerStatus.authURL} target="_blank" rel="noopener noreferrer" referrerpolicy="no-referrer">Open reviewed signer approval site</a>{/if}
    <Button variant="outline" onclick={cancelRemoteSigner}>Cancel signer request and disconnect</Button>
  </Dialog.Content>
</Dialog.Root>
