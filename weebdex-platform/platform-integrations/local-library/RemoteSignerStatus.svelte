<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { Session } from '$lib/stores';
  import { remoteSignerStatus, cancelRemoteSigner } from './remote-signer-session';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  let disposed = false, focusGeneration = 0;
  let returnFocus: { target: HTMLElement | null; dialog: HTMLElement | null; account: string | null; href: string } | null = null;
  function visible(element: HTMLElement) { const style = getComputedStyle(element); return element.isConnected && !element.closest('[hidden]') && style.display !== 'none' && style.visibility !== 'hidden'; }
  function otherDialogs(approval: Element | null) { return Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"], [role="alertdialog"]')).filter(element => element !== approval && !element.contains(approval) && visible(element)); }
  function rememberFocus() {
    focusGeneration++;
    const approval = document.querySelector('[data-remote-signer-status]');
    const active = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null;
    const dialogs = otherDialogs(approval), parent = active?.closest<HTMLElement>('[role="dialog"], [role="alertdialog"]');
    returnFocus = { target: active, dialog: parent && dialogs.includes(parent) ? parent : dialogs.at(-1) ?? null, account: $Session?.identity?.id ?? null, href: window.location.href };
  }
  function restoreFocus(event: Event) {
    const captured = returnFocus, generation = ++focusGeneration, approval = document.querySelector('[data-remote-signer-status]');
    returnFocus = null;
    if (!captured || !captured.target && !captured.dialog) return;
    // The initiating button may still be disabled while cancellation unwinds.
    // Defer until pending UI settles; never focus a replaced account/view.
    event.preventDefault();
    void tick().then(() => requestAnimationFrame(() => {
      if (disposed || generation !== focusGeneration || $remoteSignerStatus.pending || captured.account !== ($Session?.identity?.id ?? null) || captured.href !== window.location.href) return;
      const active = document.activeElement;
      if (active instanceof HTMLElement && active !== document.body && active.isConnected && !approval?.contains(active)) return;
      const dialogs = otherDialogs(approval);
      if (dialogs.length && dialogs.at(-1) !== captured.dialog) return;
      if (captured.target && (!captured.dialog || captured.dialog.contains(captured.target)) && visible(captured.target) && !captured.target.matches(':disabled') && !captured.target.closest('[inert]')) captured.target.focus({ preventScroll: true });
      if (document.activeElement === captured.target) return;
      if (captured.dialog && visible(captured.dialog) && !captured.dialog.closest('[inert]')) {
        const fallback = Array.from(captured.dialog.querySelectorAll<HTMLElement>('button, a[href], input, select, textarea, [tabindex]')).find(element => element.tabIndex >= 0 && visible(element) && !element.matches(':disabled') && !element.closest('[inert]'));
        (fallback ?? captured.dialog).focus({ preventScroll: true });
      }
    }));
  }
  onDestroy(() => { disposed = true; focusGeneration++; returnFocus = null; });
</script>
<Dialog.Root open={$remoteSignerStatus.pending} onOpenChange={(value) => { if (!value) cancelRemoteSigner(); }}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] !max-w-xl overflow-y-auto" data-remote-signer-status onOpenAutoFocus={rememberFocus} onCloseAutoFocus={restoreFocus}>
    <Dialog.Header><Dialog.Title>Remote signer approval</Dialog.Title><Dialog.Description>Approve only the public change you reviewed. Nothing is retried automatically.</Dialog.Description></Dialog.Header>
    <p role="status">{$remoteSignerStatus.message}</p>
    {#if $remoteSignerStatus.authURL}<p class="text-sm">The signer asks you to visit <strong>{new URL($remoteSignerStatus.authURL).hostname}</strong>. Check that you trust this domain. Opening it may share information with that site.</p><a class="underline" href={$remoteSignerStatus.authURL} target="_blank" rel="noopener noreferrer" referrerpolicy="no-referrer">Open reviewed signer approval site</a>{/if}
    <Button variant="outline" onclick={cancelRemoteSigner}>Cancel signer request and disconnect</Button>
  </Dialog.Content>
</Dialog.Root>
