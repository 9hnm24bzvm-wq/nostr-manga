import { onDestroy } from 'svelte';
import { captureNavigationEpoch, sameNavigationEpoch } from './navigation-authority';
// Capture synchronously at a user gesture, before storage or mode selection yields.
export function createRouteActionOwner() {
  let disposed = false, generation = 0;
  onDestroy(() => { disposed = true; generation++; });
  return { capture() {
    const ownGeneration = ++generation, navigation = captureNavigationEpoch(), href = window.location.href;
    return { alive: () => !disposed && ownGeneration === generation && sameNavigationEpoch(navigation) && href === window.location.href };
  } };
}
