<script lang="ts">
  import type { BoundedReaderController } from './controller';
  import { label, plainAction, editionChoices } from './presentation';
  import { Languages } from '$lib/Locale';
  let { controller }: { controller: BoundedReaderController } = $props();
  const choices = $derived(editionChoices($controller.editions, Languages));
</script>
<section class="space-y-2 rounded border p-2" aria-label="Choose a translation">
  {#if $controller.groupAnchor}<h3>{label($controller.groupAnchor)} translations</h3>{/if}
  <p class="text-muted-foreground text-xs">Choose a translation to read.</p>
  <ul class="max-h-72 overflow-auto">
    {#each choices as choice (choice.row.id)}
      {@const row = choice.row}
      <li><button type="button" class="w-full rounded p-2 text-start hover:bg-muted" disabled={$controller.pending} onclick={event => plainAction(event, () => controller.chooseEdition(row))}><span class="block font-medium">{choice.name}</span><span class="text-muted-foreground block text-xs">{choice.detail}</span></button></li>
    {/each}
  </ul>
  <button type="button" class="underline" disabled={$controller.pending || !$controller.editionsMore} onclick={event => plainAction(event, () => controller.editions(undefined, true))}>More translations</button>
</section>
