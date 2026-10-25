<script lang="ts">
  import type { BoundedReaderController } from './controller';
  let { controller, refresh = false }: { controller: BoundedReaderController & { verifyIdentity?: () => Promise<void> }; refresh?: boolean } = $props();
</script>
<div class="text-muted-foreground space-y-1 text-sm" aria-live="polite" aria-busy={$controller.pending}>
  {#if $controller.pending && !$controller.routing}<p>Loading a bounded live selection…</p><button type="button" class="underline" onclick={() => controller.cancel()}>Cancel selection</button>{/if}
  {#if $controller.message}<p>{$controller.message}</p>{/if}
  {#if !$controller.pending && $controller.requiresReload}
    <button type="button" class="underline" onclick={() => window.location.reload()}>Reload this page</button>
  {:else if !$controller.pending && ['idle', 'error', 'restart', 'excluded'].includes($controller.status) && (refresh || controller.verifyIdentity)}
    <button type="button" class="underline" onclick={() => controller.verifyIdentity ? controller.verifyIdentity() : controller.refresh()}>Verify catalogue and refresh navigation</button>
  {/if}
</div>
