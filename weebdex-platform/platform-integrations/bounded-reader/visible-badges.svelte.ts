import { getContext, setContext, onMount, onDestroy } from 'svelte';
import { readable } from 'svelte/store';
import { prefs } from '$lib/stores';
import { nativeAuthority } from '../frontend-behavior/mutation-authority';
import { captureNavigationEpoch } from '../frontend-behavior/navigation-authority';
import { readerLibrarySource, localLibraryState, readerLibrarySelection, assertLibraryAdmission } from '../local-library/library';
import { VisibleChapterBadges, type BadgeView } from './visible-badges';
import { readerTransport, titleFilters } from './runtime.svelte';
import { historySourceKey, localHistoryAccess } from './history';
import { identityChanged } from './endpoint';
import { reference, ReaderIdentityChanged } from './contract';
import type { ReaderIdentity } from './client';
const badgeContext = Symbol('bounded-visible-chapter-badges');
export const emptyBadgeView = readable<BadgeView>({ status: 'unknown', message: '' });
export const getVisibleChapterBadges = () => getContext<ReturnType<typeof createVisibleChapterBadges> | undefined>(badgeContext);
export function createVisibleChapterBadges(options: { manga(): string; href(): string; chapters(): readonly Chapter[] | undefined }) {
  let live = false, disposed = false, request = 0, accepted: { page: string; ids: readonly string[]; identity: ReaderIdentity } | undefined;
  let verified: ReaderIdentity | undefined, catalogueChanged = false;
  const pageKey = () => { const url = new URL(options.href()); return JSON.stringify([options.manga(), url.origin, url.pathname, url.search, titleFilters(url.searchParams).toString(), readerTransport.endpoint()]); };
  const sourceKey = () => { const source = readerLibrarySource(); return JSON.stringify([historySourceKey(source), source.observedCatalogue, captureNavigationEpoch()]); };
  const context = () => {
    const source = readerLibrarySource(), target = pageKey();
    if (identityChanged(readerTransport.endpoint(), verified, source.observedCatalogue)) catalogueChanged = true;
    if (catalogueChanged) throw new ReaderIdentityChanged('The catalogue changed. Reload before checking read status.');
    if (!live || disposed || !accepted || accepted.page !== target) throw Error('No current accepted chapter page.');
    const ids = options.chapters()?.map(row => row.id);
    if (!ids || JSON.stringify(ids) !== JSON.stringify(accepted.ids)) throw Error('Displayed chapter IDs changed before badge admission.');
    return { key: JSON.stringify([target, request, ids, sourceKey()]), target: JSON.stringify([target, source.context, source.admission.selectionEpoch, source.admission.authority.actor, source.admission.authority.epoch]), ids: accepted.ids };
  };
  const controller = new VisibleChapterBadges(context, async budget => {
    const source = readerLibrarySource(), endpoint = readerTransport.endpoint();
    assertLibraryAdmission(source.admission); budget.check();
    if (source.selectionPending) throw Error('A library selection is pending. No account or device fallback was used.');
    if (source.mode === 'account' && endpoint.profile !== 'native-aggregate-v1') throw Error('This catalogue has no native-cookie badge source. Select device history explicitly.');
    const identity = await readerTransport.verifyIdentity(budget); budget.check();
    assertLibraryAdmission(source.admission);
    if (verified && JSON.stringify(identity) !== JSON.stringify(verified) || identityChanged(endpoint, identity, readerLibrarySource().observedCatalogue)) { catalogueChanged = true; throw new ReaderIdentityChanged('The catalogue identity changed. Reload before checking read status.'); }
    if (!accepted || JSON.stringify(identity) !== JSON.stringify(accepted.identity)) { catalogueChanged = true; throw new ReaderIdentityChanged('Chapter rows and the live catalogue identity disagree. Reload before checking read status.'); }
    verified ??= identity;
    if (source.mode === 'device') {
      if (!source.scope || source.scope.origin !== identity.origin || source.scope.catalogNamespace !== identity.catalogNamespace || source.scope.serviceKey !== identity.serviceKey) throw Error('Device history does not match this verified catalogue. No cookie fallback was used.');
      return localHistoryAccess(options.manga());
    }
    return readerTransport.nativeHistory(options.manga(), identity);
  });
  const update = () => { if (live && !disposed) controller.sync(); };
  const stops = [localLibraryState.subscribe(update), readerLibrarySelection.subscribe(update), nativeAuthority.subscribe(update), prefs.subscribe(update)];
  $effect(() => { options.manga(); options.href(); options.chapters(); update(); });
  onMount(() => { live = true; update(); });
  onDestroy(() => { disposed = true; live = false; request++; controller.dispose(); for (const stop of stops) stop(); });
  const api = {
    state: controller,
    value: (view: BadgeView, id: string) => controller.valueFor(view, id),
    beginMutation: () => controller.beginMutation(),
    retry: () => {
      try { context(); } catch {
        controller.invalidate('Reload the chapter list before retrying read status. No current page binding is available.');
        return;
      }
      return controller.refresh();
    },
    requestPage() {
      // Public metadata rendering is independent of private history hydration.
      // Only a same-response catalogue witness can admit these IDs for badges.
      const serial = ++request, page = pageKey(), source = readerLibrarySource(), navigation = captureNavigationEpoch();
      const authority = () => { const now = readerLibrarySource(); return JSON.stringify([now.admission.selectionEpoch, now.admission.authority.actor, now.admission.authority.epoch]); };
      // Capture navigation start separately from feed path/query. Already
      // accepted pages remain usable across hash-only changes; in-flight old
      // navigation responses may not rebase into a newer navigation epoch.
      const admitted = authority(), observed = source.observedCatalogue;
      accepted = undefined; controller.invalidate();
      return { accept(value: unknown) {
        if (disposed || request !== serial) return;
        try {
          if (disposed || request !== serial || pageKey() !== page || authority() !== admitted || captureNavigationEpoch() !== navigation) throw Error('The chapter page authority changed. Reload the chapter list before checking read status.');
          const body = value as { data?: readonly Chapter[]; chapterPage?: { version?: unknown; mangaId?: unknown; idProfile?: unknown; consistency?: unknown; catalogIdentity?: { catalogNamespace?: unknown; serviceKey?: unknown } } };
          const rows = body?.data, pageWitness = body?.chapterPage, witness = pageWitness?.catalogIdentity, endpoint = readerTransport.endpoint();
          if (pageWitness?.version !== 1 || pageWitness.mangaId !== options.manga() || pageWitness.idProfile !== (endpoint.profile === 'native-aggregate-v1' ? 'native-id-v1' : 'public-uuid-v1') || pageWitness.consistency !== 'response-snapshot-v1') throw Error('This chapter response has no snapshot-bound catalogue identity. Read status remains unknown.');
          if (!witness || typeof witness.catalogNamespace !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(witness.catalogNamespace) || witness.serviceKey !== null && (typeof witness.serviceKey !== 'string' || !/^[0-9a-f]{64}$/.test(witness.serviceKey))) throw Error('This chapter response has no verified catalogue identity. Read status remains unknown.');
          const identity = Object.freeze({ origin: endpoint.origin, catalogNamespace: witness.catalogNamespace, serviceKey: witness.serviceKey as string | null });
          if (identityChanged(endpoint, identity, observed) || identityChanged(endpoint, identity, readerLibrarySource().observedCatalogue) || verified && JSON.stringify(verified) !== JSON.stringify(identity)) throw new ReaderIdentityChanged('The chapter response catalogue identity changed. Reload before checking read status.');
          if (!Array.isArray(rows) || rows.length > 100) throw Error('The visible chapter page exceeds 100 rows.');
          const ids = rows.map(row => { if (row.relationships?.manga?.id !== options.manga()) throw Error('A chapter row does not belong to this title.'); return reference(row.id, endpoint.profile); });
          if (new Set(ids).size !== ids.length) throw Error('The visible chapter page contains duplicate IDs.');
          accepted = { page, ids: Object.freeze(ids), identity }; queueMicrotask(update);
        } catch (error) {
          // Do not reject or replace the retained public feed. No private lookup
          // is admitted without its bound witness, in either library mode.
          const message = error instanceof Error ? error.message : 'Read status remains unknown.';
          accepted = undefined; controller.invalidate(message + (/reload/i.test(message) ? '' : ' Reload the chapter list before retrying read status.'));
        }
      } };
    }
  };
  setContext(badgeContext, api); return api;
}
