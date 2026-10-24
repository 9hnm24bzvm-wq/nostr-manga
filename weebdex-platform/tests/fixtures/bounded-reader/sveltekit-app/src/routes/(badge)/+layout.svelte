<script lang="ts">
  // NEW fixture lifetime owner. The application guard implementation is unchanged.
  import { onMount } from 'svelte';
  import { installReaderNavigationGuard } from '$platform/reader-navigation';
  declare const __BADGE_INSTALL_GUARD__: boolean;
  const { children } = $props();
  if (__BADGE_INSTALL_GUARD__) installReaderNavigationGuard();
  onMount(() => {
    window.dispatchEvent(new CustomEvent('badge-fixture-guard', { detail: { phase: 'mounted', installed: __BADGE_INSTALL_GUARD__ } }));
    return () => window.dispatchEvent(new CustomEvent('badge-fixture-guard', { detail: { phase: 'destroyed', installed: __BADGE_INSTALL_GUARD__ } }));
  });
</script>
{@render children()}
