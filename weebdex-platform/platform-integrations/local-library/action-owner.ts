// Capture a reader's destination before storage initialization can yield.
import { HTTPError } from '$lib/HTTP';
import { onDestroy } from 'svelte';
import { openLibrary, captureLibraryAdmission, assertLibraryAdmission, captureLibraryContext, assertLibraryContext } from './library';
export function createLibraryActionOwner(target: () => string) {
  let disposed = false;
  onDestroy(() => { disposed = true; });
  return { capture() {
    const admission = captureLibraryAdmission(), key = target();
    let context: ReturnType<typeof captureLibraryContext> | undefined;
    const assertDestination = () => {
      if (key !== target()) throw new HTTPError(409, 'The reader chapter changed before its saved action completed');
      assertLibraryAdmission(admission);
      if (context) assertLibraryContext(context);
    };
    const assert = () => { if (disposed) throw new HTTPError(409, 'The reader was closed; no new automatic history action was admitted'); assertDestination(); };
    assert();
    // Stable destination admission, excluding harmless account-data refreshes.
    // Re-selecting device mode creates a fresh page action, not an old replay.
    const destinationKey = JSON.stringify([key, admission.selectionEpoch, admission.context?.generation, admission.context?.key, admission.authority.actor, admission.authority.epoch]);
    return { destinationKey, assert, assertDestination, alive: () => { try { assert(); return true; } catch { return false; } }, async open() {
      assert(); await openLibrary(); assert();
      context ??= captureLibraryContext(); assert(); return context;
    } };
  } };
}
