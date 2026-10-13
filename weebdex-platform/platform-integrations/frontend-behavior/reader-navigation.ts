import { goto, beforeNavigate } from '$app/navigation';
import { onDestroy } from 'svelte';
import { invalidateNavigationEpoch } from './navigation-authority';
type LastPageIntent = Readonly<{ chapter: string; target: string; generation: number }>;
let generation = 0, pending: LastPageIntent | undefined;
export function installReaderNavigationGuard() {
  beforeNavigate(({ to }) => {
    invalidateNavigationEpoch();
    if (pending && to?.url.href !== pending.target) { generation++; pending = undefined; }
  });
  const shallowNavigation = () => { invalidateNavigationEpoch(); };
  window.addEventListener('popstate', shallowNavigation);
  window.addEventListener('hashchange', shallowNavigation);
  onDestroy(() => { invalidateNavigationEpoch(); generation++; pending = undefined; window.removeEventListener('popstate', shallowNavigation); window.removeEventListener('hashchange', shallowNavigation); });
}
export async function navigateReaderLastPage(chapter: string, target: string) {
  const intent = Object.freeze({ chapter, target: new URL(target, window.location.href).href, generation: ++generation });
  pending = intent;
  try { await goto(target); }
  finally { if (pending === intent) pending = undefined; }
}
export function consumeReaderLastPage(chapter: string) {
  const intent = pending;
  // Consume before any history or progress effects run. A failed destination
  // cannot leave a blanket boolean for some other subsequently opened chapter.
  pending = undefined;
  return !!intent && intent.chapter === chapter && intent.generation === generation && intent.target === window.location.href;
}
