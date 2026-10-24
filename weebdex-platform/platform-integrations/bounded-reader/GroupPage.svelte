<script lang="ts">
  import type { BoundedReaderController } from './controller';
  import { label, plainAction } from './presentation';
  import Status from './Status.svelte';
  let { controller, showStatus = true }: { controller: BoundedReaderController; showStatus?: boolean } = $props();
</script>
<details class="min-w-0 flex-1 rounded border p-2" ontoggle={event => { if (event.currentTarget.open) void controller.groups(); }}>
  <summary>Chapters</summary>
  <p class="text-muted-foreground text-xs">One live page, newest to oldest. Earlier pages are not retained.</p>
  <ul class="max-h-72 overflow-auto">
    {#each $controller.groups as row (row.id)}
      <li><button type="button" class="w-full p-2 text-start hover:underline" disabled={$controller.pending} onclick={event => plainAction(event, () => controller.chooseGroup(row))}>{label(row)} · {row.language}</button></li>
    {/each}
  </ul>
  <div class="flex gap-3">
    <button type="button" class="underline" disabled={$controller.pending} onclick={event => plainAction(event, () => controller.groups())}>Restart chapter list</button>
    <button type="button" class="underline" disabled={$controller.pending || !$controller.groupsMore} onclick={event => plainAction(event, () => controller.groups(true))}>Next chapter page</button>
  </div>
</details>

{#if showStatus}<Status {controller} refresh />{/if}
