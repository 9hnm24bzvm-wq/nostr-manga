<script lang="ts">
  import { onDestroy } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import IdentitySetup from './IdentitySetup.svelte';
  let { show = true, onfinish = () => {}, label = 'Set up public identity' }: { show?: boolean; onfinish?: () => void; label?: string } = $props();
  let open = $state(false), generation = $state(0), disposed = false;
  let trigger = $state<HTMLButtonElement | null>(null), returnAnchor = $state<HTMLSpanElement | null>(null);
  function restoreTaskFocus(event: Event) {
    // A connected identity hides the trigger. Keep keyboard focus in the task,
    // including the outer rating dialog, rather than returning it to the page.
    if (disposed) return;
    const target = trigger?.isConnected ? trigger : returnAnchor;
    if (!target?.isConnected) return;
    event.preventDefault(); target.focus({ preventScroll: true });
  }
  function setOpen(value: boolean) { generation++; open = value; }
  function guard(epoch: number) { return () => { if (disposed || !open || epoch !== generation) throw Error('Identity setup was closed. Please open it again.'); }; }
  function finish() { setOpen(false); onfinish(); }
  onDestroy(() => { disposed = true; generation++; });
</script>
<span bind:this={returnAnchor} tabindex="-1" class="sr-only">Continue with your task</span>
{#if show}<Button bind:ref={trigger} variant="outline" onclick={() => setOpen(true)}>{label}</Button>{/if}
<Dialog.Root {open} onOpenChange={setOpen}>
  <Dialog.Content onCloseAutoFocus={restoreTaskFocus} class="max-h-[calc(100dvh-2rem)] !max-w-[min(42rem,calc(100dvw-2rem))] overflow-y-auto">
    <Dialog.Header><Dialog.Title>Set up your identity</Dialog.Title><Dialog.Description>Save your recovery key, review joining this site, then return to your task. Nothing is posted without your approval.</Dialog.Description></Dialog.Header>
    {#if open}{#key generation}<IdentitySetup resumeSelected onfinish={finish} assertCurrent={guard(generation)} finishLabel="Back to what I was doing" />{/key}{/if}
  </Dialog.Content>
</Dialog.Root>
