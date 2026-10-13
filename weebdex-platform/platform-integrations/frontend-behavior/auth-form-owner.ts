// Auth flow authority stays on the server. This owner fences only stale UI,
// history and navigation effects, without replaying or canceling an auth write.
import { onDestroy } from 'svelte';
import { captureNavigationEpoch, sameNavigationEpoch } from './navigation-authority';
export function createAuthFormOwner() {
  let disposed = false, generation = 0, observed = window.location.href;
  const invalidate = () => { generation++; observed = window.location.href; };
  window.addEventListener('popstate', invalidate);
  window.addEventListener('hashchange', invalidate);
  onDestroy(() => { disposed = true; generation++; window.removeEventListener('popstate', invalidate); window.removeEventListener('hashchange', invalidate); });
  return { observe(href: string) {
    if (href !== observed) { observed = href; generation++; }
  }, capture(flowId?: string, currentFlow?: () => string | undefined, requireUrlFlow = true) {
    const ownGeneration = ++generation, navigation = captureNavigationEpoch(); let expected = window.location.href;
    observed = expected;
    const requestedFlow = new URL(expected).searchParams.get('flow');
    const admitted = !flowId || !requireUrlFlow || !requestedFlow || requestedFlow === flowId;
    const alive = () => admitted && !disposed && generation === ownGeneration && sameNavigationEpoch(navigation) && window.location.href === expected && (!flowId || currentFlow?.() === flowId);
    return { alive, replaceHistory(action: () => void) {
      if (!alive()) return false;
      action(); expected = window.location.href; observed = expected; return true;
    } };
  } };
}
