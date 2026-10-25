<script lang="ts">
  // NEW route wrapper; actual production-composed consumers remain in the unchanged harness.
  import { getContext } from 'svelte';
  import VisibleBadgeHarness from '$badgeHarness';
  let { data } = $props();
  const observe = getContext<(value: unknown) => void>('badge-fixture-history-observer');
  const observeHistory = (value: unknown) => {
    const copy = value === undefined ? null : JSON.parse(JSON.stringify(value));
    observe(copy);
    window.dispatchEvent(new CustomEvent('badge-fixture-history', { detail: copy }));
  };
</script>
{#key data.manga}
  <VisibleBadgeHarness manga={data.manga} wholeTitleHistory={data.wholeTitleHistory} {observeHistory} />
{/key}
