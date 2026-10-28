<script lang="ts">
  import type { BoundedReaderController } from './controller';
  import { label, plainAction } from './presentation';
  let { controller }: { controller: BoundedReaderController } = $props();
</script>
<section class="space-y-2 rounded border p-2" aria-label="Edition selection">
  {#if $controller.groupAnchor}<h3>{label($controller.groupAnchor)} editions</h3>{/if}
  <p class="text-muted-foreground text-xs">Choose a translation to read.</p>
  {#if $controller.winner}
    {@const winner = $controller.winner}
    <button type="button" class="underline" disabled={$controller.pending} onclick={event => plainAction(event, () => controller.chooseEdition(winner))}>Open recommended translation · {winner.language}</button>
  {/if}
  <ul class="max-h-72 overflow-auto">
    {#each $controller.editions as row (row.id)}
      <li><button type="button" class="w-full break-all p-2 text-start hover:underline" disabled={$controller.pending} onclick={event => plainAction(event, () => controller.chooseEdition(row))}>{row.language} · {row.groupIds.length ? row.groupIds.map(id => id.slice(0, 8)).join(', ') : 'No group'} · {row.id.slice(0, 8)}</button></li>
    {/each}
  </ul>
  <button type="button" class="underline" disabled={$controller.pending || !$controller.editionsMore} onclick={event => plainAction(event, () => controller.editions(undefined, true))}>Next edition page</button>
</section>
