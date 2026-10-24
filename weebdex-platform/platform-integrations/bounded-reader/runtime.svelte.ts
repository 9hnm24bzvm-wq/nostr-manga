import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { PUBLIC_API_URL } from '$env/static/public';
import { onDestroy, onMount } from 'svelte';
import { prefs } from '$lib/stores';
import { localLibraryState, readerLibrarySource, readerLibrarySelection } from '../local-library/library';
import { publicUuidLibraryEnabled } from '../public-api/library-staging';
import { nativeAuthority, captureNativeAuthority } from '../frontend-behavior/mutation-authority';
import { captureNavigationEpoch } from '../frontend-behavior/navigation-authority';
import { navigateReaderLastPage } from '../frontend-behavior/reader-navigation';
import { BoundedReaderController, type CurrentIntent } from './controller';
import { readerFilters, createReaderClient, type ReaderIdentity, type ReaderBudget } from './client';
import { ReaderIdentityChanged } from './contract';
import { readerEndpoint, identityChanged } from './endpoint';
import { localHistoryAccess } from './history';
import { createNativeHistoryAccess } from './native-history';
import { createNativeContinueAccess } from './native-continue';
import { titleReaderFilters } from './title-query';
const endpoint = () => readerEndpoint(publicUuidLibraryEnabled, PUBLIC_API_URL, typeof window === 'undefined' ? 'https://reader.invalid/' : window.location.href);
// One configured-source transport owner survives remount. Forum defaults are untouched.
const client = createReaderClient({ origin: () => endpoint().origin }, endpoint);
const nativeHistory = createNativeHistoryAccess(endpoint);
const nativeContinue = createNativeContinueAccess(endpoint);
// One-entry public identity handoff serializes simultaneous title/badge mounts.
// It owns no transport slot: the original client and initiating budget still own
// the request and unsettled cleanup. Each waiter retains its own cancellation,
// authority checks, deadline and conservative request/byte accounting.
let identityHandoff: { key: string; promise: Promise<{ identity: ReaderIdentity; bytes: number }> } | undefined;
export async function verifyReaderIdentity(budget: ReaderBudget): Promise<ReaderIdentity> {
  budget.check();
  const key = JSON.stringify([endpoint(), captureNavigationEpoch(), readerLibrarySource().observedCatalogue]);
  let handoff = identityHandoff; const initiator = !handoff || handoff.key !== key;
  if (initiator) {
    const initialBytes = budget.bytes;
    const promise = client.verifyIdentity(budget).then(identity => { budget.check(); return { identity, bytes: budget.bytes - initialBytes }; });
    handoff = identityHandoff = { key, promise };
    const clear = () => { if (identityHandoff === handoff) identityHandoff = undefined; };
    promise.then(clear, clear);
  } else budget.request();
  let cancel: () => void = () => {};
  const cancelled = new Promise<never>((_, reject) => { cancel = () => reject(Error('Catalogue identity wait expired or was cancelled')); });
  cancelled.catch(() => {}); budget.signal.addEventListener('abort', cancel, { once: true });
  if (budget.signal.aborted) cancel();
  try {
    const value = await Promise.race([handoff!.promise, cancelled]); budget.check();
    if (key !== JSON.stringify([endpoint(), captureNavigationEpoch(), readerLibrarySource().observedCatalogue])) throw Error('Catalogue identity admission changed while waiting.');
    if (!initiator) budget.consume(value.bytes);
    return value.identity;
  } finally { budget.signal.removeEventListener('abort', cancel); }
}
export const readerTransport = Object.freeze({ endpoint, verifyIdentity: verifyReaderIdentity, nativeHistory });
export type RuntimeReaderController = BoundedReaderController & { verifyIdentity(): Promise<void> };
export function currentIntent(chapter: Chapter): CurrentIntent {
  return { id: chapter.id, volume: chapter.volume ?? null, chapter: chapter.chapter ?? null, language: chapter.language, version: chapter.version, groupIds: chapter.relationships.groups?.map(group => group.id) ?? [] };
}
export const titleFilters = (titleQuery: URLSearchParams) => titleReaderFilters(query => prefs.applyChapterPreferences(query), titleQuery);
export function createBoundedReader(options: { manga: () => string; current?: () => Chapter; filters: () => URLSearchParams; queryEpoch: () => string }): RuntimeReaderController {
  let live = false, disposed = false, refreshRequested = false, catalogueChanged = false, scope: ReaderIdentity | undefined, verifiedIdentity: ReaderIdentity | undefined;
  const context = () => {
    const target = endpoint(), source = readerLibrarySource(), native = captureNativeAuthority();
    const history = source.mode === 'account' ? nativeHistory(options.manga(), scope) : localHistoryAccess(options.manga());
    const matchedHistory = !!scope && !!source.scope && source.scope.origin === scope.origin && source.scope.catalogNamespace === scope.catalogNamespace && source.scope.serviceKey === scope.serviceKey;
    return { destination: { ...target, mangaId: options.manga(), identity: scope && { catalogNamespace: scope.catalogNamespace, serviceKey: scope.serviceKey }, filters: readerFilters(target.profile, options.filters()) },
      epoch: JSON.stringify([source.context, scope, native.actor, native.epoch, captureNavigationEpoch(), options.queryEpoch()]),
      current: options.current ? currentIntent(options.current()) : undefined,
      nativeContinue: nativeContinue(options.manga(), scope),
      history: history.status === 'unavailable' || history.status === 'pending' || history.status === 'available-native' || matchedHistory ? history : { ...history, status: 'incomplete' as const, reason: 'Device history does not have the verified reader catalogue scope. No account/device or catalogue fallback was made.' } };
  };
  const controller = new BoundedReaderController(context, async (row, lastPage) => {
    const target = resolve(`/chapter/${encodeURIComponent(row.id)}/1`);
    if (lastPage) await navigateReaderLastPage(row.id, target); else await goto(target);
  }, async (destination, request, budget) => {
    if (catalogueChanged) throw new ReaderIdentityChanged('The catalogue namespace or service identity changed. Reload this page before navigating; saved history was not reinterpreted.');
    if (!scope || scope.origin !== destination.origin) throw Error('Verify the current catalogue identity before navigation. The open chapter remains readable.');
    try { return await client.read(destination, request, budget); } catch (error) {
      if (error instanceof ReaderIdentityChanged) { catalogueChanged = true; scope = undefined; refreshRequested = false; controller.sync(); controller.requireReload(error.message); }
      throw error;
    }
  });
  const verifyIdentity = async () => {
    if (!live || disposed || controller.snapshot.pending) return;
    if (catalogueChanged) { controller.requireReload('The catalogue namespace or service identity changed. Reload this page before navigating; saved history was not reinterpreted.'); return; }
    scope = undefined; refreshRequested = false; controller.sync();
    // Identity discovery and initial around share one action, owner, total
    // request/row/byte budget and deadline. Only this verified commit may rebase
    // the action's scope; account, route, query and history still must be current.
    await controller.prepareIdentity(async budget => {
      const value = await verifyReaderIdentity(budget); budget.check();
      if (verifiedIdentity && (value.origin !== verifiedIdentity.origin || value.catalogNamespace !== verifiedIdentity.catalogNamespace || value.serviceKey !== verifiedIdentity.serviceKey)) {
        catalogueChanged = true; scope = undefined; refreshRequested = false;
        const message = 'The catalogue namespace or service identity changed. Reload this page before navigating; saved history was not reinterpreted.';
        controller.requireReload(message); throw new ReaderIdentityChanged(message);
      }
      // Keep the first admitted identity across failed verification attempts. A
      // retry cannot silently rebase an already displayed title/chapter ID.
      return () => { scope = value; verifiedIdentity ??= value; };
    }, !!options.current);
  };
  const refreshWhenSettled = () => { if (!live || disposed || !scope || !refreshRequested || controller.snapshot.pending) return; refreshRequested = false; void controller.refresh(); };
  const stopView = controller.subscribe(() => { if (refreshRequested) queueMicrotask(refreshWhenSettled); });
  const update = () => {
    if (!live || disposed) return;
    const observed = readerLibrarySource().observedCatalogue;
    if (identityChanged(endpoint(), verifiedIdentity, observed)) { catalogueChanged = true; scope = undefined; refreshRequested = false; }
    const previous = controller.snapshot; controller.sync();
    if (catalogueChanged) { controller.requireReload('The catalogue namespace or service identity changed. Reload this page before navigating; saved history was not reinterpreted.'); return; }
    // Only external context changes request one new around view. A semantic
    // witness failure or Continue pause never schedules an automatic retry.
    if (options.current && scope && previous !== controller.snapshot) { refreshRequested = true; queueMicrotask(refreshWhenSettled); }
  };
  const unsubscribes = [localLibraryState.subscribe(update), readerLibrarySelection.subscribe(update), nativeAuthority.subscribe(update), prefs.subscribe(update)];
  $effect(() => { options.manga(); options.current?.(); options.queryEpoch(); update(); });
  onMount(() => { live = true; controller.sync(); void verifyIdentity(); });
  onDestroy(() => { disposed = true; live = false; refreshRequested = false; stopView(); controller.dispose(); for (const stop of unsubscribes) stop(); });
  return Object.assign(controller, { verifyIdentity });
}
