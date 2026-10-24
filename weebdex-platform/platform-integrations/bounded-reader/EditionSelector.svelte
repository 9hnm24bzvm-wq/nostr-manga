<script lang="ts">
  import { useReader } from '$lib/components/reader/context.svelte';
  import EditionPage from './EditionPage.svelte';
  import Status from './Status.svelte';
  let { class: className }: { class?: string } = $props();
  const reader = useReader(), controller = reader.navigation;
</script>
<details class={['rounded border p-2', className]} ontoggle={event => { if (event.currentTarget.open) void controller.editions(); }}>
  <summary>Choose edition</summary>
  <EditionPage {controller} />
  <Status {controller} refresh />
</details>
