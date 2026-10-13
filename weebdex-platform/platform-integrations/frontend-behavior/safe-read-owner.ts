import { onDestroy } from 'svelte';
import { captureNavigationEpoch, sameNavigationEpoch } from './navigation-authority';
// A read response owns UI only; this helper never grants account authority.
export function createSafeReadOwner() {
  let disposed = false, generation = 0;
  onDestroy(() => { disposed = true; generation++; });
  return {
    isLive: () => !disposed,
    invalidate() { generation++; },
    capture(validate: () => boolean = () => true) {
      const own = ++generation, href = window.location.href, navigation = captureNavigationEpoch();
      const current = () => !disposed && generation === own && href === window.location.href;
      return { current, alive: () => current() && sameNavigationEpoch(navigation) && validate() };
    }
  };
}
