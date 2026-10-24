<script lang="ts">
  // NEW fixture command bridge. It calls genuine APIs, never their setters or internals.
  import { onMount, tick, setContext } from 'svelte';
  import { get } from 'svelte/store';
  import { goto, beforeNavigate, afterNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import Session from '$lib/stores/Session';
  import { openLibrary, useMode, verifyCatalogueScope, refreshLibrary, localLibraryState, readerLibrarySource, readerLibrarySelection, createList } from '$localLibrary/library';
  import { captureNavigationEpoch } from '$platform/navigation-authority';
  import { captureNativeAuthority } from '$platform/mutation-authority';
  const { children } = $props();
  let cancelTarget: string | null | undefined;
  let historyObservation: unknown;
  let alive = false;
  const pending = new Set<string>();
  const seen = new Set<string>();
  const safe = (value: unknown) => value === undefined ? null : JSON.parse(JSON.stringify(value));
  setContext('badge-fixture-history-observer', (value: unknown) => { historyObservation = safe(value); });
  function snapshot() {
    const local = get(localLibraryState), source = readerLibrarySource();
    return safe({ href: window.location.href, pageUrl: page.url.href, routeId: page.route.id, error: page.error, status: page.status, pageState: $state.snapshot(page.state), navigationEpoch: captureNavigationEpoch(), nativeAuthority: captureNativeAuthority(), libraryState: local, readerLibrarySelection: get(readerLibrarySelection), readerSource: { ...source, db: source.db ? { name: source.db.name, version: source.db.version } : undefined }, wholeTitleHistory: historyObservation });
  }
  beforeNavigate(navigation => {
    window.dispatchEvent(new CustomEvent('badge-fixture-navigation', { detail: { phase: 'before', from: navigation.from?.url.href, to: navigation.to?.url.href, type: navigation.type } }));
    if (cancelTarget !== undefined && (cancelTarget === null || navigation.to?.url.href === cancelTarget)) { cancelTarget = undefined; navigation.cancel(); window.dispatchEvent(new CustomEvent('badge-fixture-navigation', { detail: { phase: 'cancelled', from: navigation.from?.url.href, to: navigation.to?.url.href, type: navigation.type } })); }
  });
  afterNavigate(navigation => window.dispatchEvent(new CustomEvent('badge-fixture-navigation', { detail: { phase: 'after', to: navigation.to?.url.href, type: navigation.type } })));
  function destination(value: unknown) {
    if (typeof value !== 'string' || value.length > 4096) throw Error('Invalid fixture navigation destination');
    const url = new URL(value, window.location.href);
    if (url.origin !== window.location.origin || !/^\/(?:title\/[^/]+|chapter\/[^/]+\/[^/]+|__badge_outside)$/.test(url.pathname)) throw Error('Off-contract fixture destination');
    return url.href;
  }
  async function execute(name: string, args: Record<string, unknown>) {
    if (name === 'snapshot') return snapshot();
    if (name === 'navigate') await goto(destination(args.href), { replaceState: args.replaceState === true });
    else if (name === 'refreshSession') await Session.initialize(true);
    else if (name === 'openLibrary') await openLibrary();
    else if (name === 'selectMode') { if (args.mode !== 'device' && args.mode !== 'account') throw Error('Invalid library mode'); await useMode(args.mode); }
    else if (name === 'verifyCatalogue') await verifyCatalogueScope();
    else if (name === 'refreshLibrary') await refreshLibrary();
    else if (name === 'createList') { if (typeof args.name !== 'string' || args.name.length > 64) throw Error('Invalid fixture list name'); await createList(args.name); }
    else if (name === 'cancelNextNavigation') cancelTarget = args.href === undefined ? null : destination(args.href);
    else if (name === 'hash') { if (typeof args.hash !== 'string' || args.hash.length > 128 || args.hash && !args.hash.startsWith('#')) throw Error('Invalid hash'); window.location.hash = args.hash; }
    else if (name === 'back') window.history.back();
    else if (name === 'forward') window.history.forward();
    else throw Error(`Unsupported fixture command: ${name}`);
    await tick(); return snapshot();
  }
  onMount(() => {
    alive = true;
    const history = (event: Event) => { historyObservation = safe((event as CustomEvent).detail); };
    const command = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (!detail || typeof detail.id !== 'string' || !/^badge-[0-9]+$/.test(detail.id) || typeof detail.name !== 'string' || !detail.args || typeof detail.args !== 'object' || Array.isArray(detail.args)) return;
      const reply = (value: unknown) => { if (alive) window.dispatchEvent(new CustomEvent('badge-fixture-result', { detail: value })); };
      if (seen.has(detail.id) || seen.size >= 4096 || pending.size >= 16) { reply({ id: detail.id, ok: false, error: { name: 'Error', message: 'Fixture command budget or duplicate' } }); return; }
      seen.add(detail.id); pending.add(detail.id);
      void execute(detail.name, detail.args).then(value => reply({ id: detail.id, ok: true, value }), error => reply({ id: detail.id, ok: false, error: { name: error instanceof Error ? error.name : 'Error', message: error instanceof Error ? error.message : String(error) } })).finally(() => pending.delete(detail.id));
    };
    window.addEventListener('badge-fixture-command', command);
    window.addEventListener('badge-fixture-history', history);
    window.dispatchEvent(new CustomEvent('badge-fixture-ready', { detail: { schema: 1, snapshot: snapshot() } }));
    return () => { alive = false; window.removeEventListener('badge-fixture-command', command); window.removeEventListener('badge-fixture-history', history); };
  });
</script>
{@render children()}
