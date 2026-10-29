import { singletonAccountContinueFilters } from './account-reader-contract';
import { ReaderBudget, READER_LIMITS, readReader, type ReaderRead } from './client';
import { NATIVE_COMPLETION_LIMITS } from '../public-api/owned-json';
import { ReaderIdentityChanged, sameGroup, sameRow, type Destination, type Operation, type Request, type Row, type SelectorPage } from './contract';
import { sameContinueSelection, singletonContinueFilters, type NativeContinueResult } from './native-continue-contract';
import type { NativeContinueAccess } from './native-continue';
export type CurrentIntent = Readonly<{ id: string; volume: string | null; chapter: string | null; language: string; version: number; groupIds: readonly string[] }>;
// Device completion stays inside this closure. Native account lookup sends only
// selected server chapter IDs and captured account/catalogue admission, never local history.
export type HistoryAccess = Readonly<{ key: string; status: 'complete-local' | 'available-native' | 'pending' | 'incomplete' | 'unavailable'; completed(ids: readonly string[], budget: ReaderBudget): Promise<boolean>; reason?: string }>;
export type ReaderContext = Readonly<{ destination: Destination; epoch: string; current?: CurrentIntent; history: HistoryAccess; nativeContinue?: NativeContinueAccess }>;
export type Status = 'idle' | 'pending' | 'ready' | 'boundary' | 'excluded' | 'restart' | 'error' | 'paused' | 'candidate' | 'observed-end';
export type ReaderView = Readonly<{
  status: Status; pending: boolean; routing?: boolean; message: string; requiresReload?: boolean; current?: Row; next?: Row | null; previous?: Row | null;
  groups: readonly Row[]; editions: readonly Row[]; groupAnchor?: Row; winner?: Row | null;
  groupsMore: boolean; editionsMore: boolean; candidate?: Row; hasMultipleEditions?: boolean;
}>;
const empty = (): ReaderView => ({ status: 'idle', pending: false, message: '', groups: [], editions: [], groupsMore: false, editionsMore: false });
class Restart extends Error {}
class Excluded extends Error {}
const contextKey = (c: ReaderContext) => JSON.stringify([c.destination, c.epoch, c.current, c.history.key, c.history.status, c.nativeContinue?.key]);
function matchesIntent(row: Row, intent: CurrentIntent): boolean {
  return row.id === intent.id && row.volume === intent.volume && row.chapter === intent.chapter && row.language === intent.language && row.version === intent.version &&
    row.groupIds.length === intent.groupIds.length && row.groupIds.every(id => intent.groupIds.includes(id));
}
export class BoundedReaderController {
  private value: ReaderView = empty();
  private readonly listeners = new Set<(view: ReaderView) => void>();
  private key = ''; private reloadMessage?: string; private generation = 0; private disposed = false; private active?: ReaderBudget; private routing?: ReaderBudget;
  private groupsCursor: string | null = null; private groupsScope?: string;
  private editionCursor: string | null = null; private editionScope?: string;
  private continueIntent?: { key: string; result: NativeContinueResult };
  private recent: string[] = [];
  private scan?: { groups: readonly Row[]; index: number; groupAfter?: string; groupScope?: string; end: boolean; anchor?: Row; editionAfter?: string; editionScope?: string; completed: boolean };
  constructor(private readonly context: () => ReaderContext, private readonly navigate: (row: Row, lastPage: boolean, page?: number) => Promise<void>, private readonly read: ReaderRead = readReader) {}
  get snapshot() { return this.value; }
  subscribe = (listener: (view: ReaderView) => void) => { this.listeners.add(listener); listener(this.value); return () => { this.listeners.delete(listener); }; };
  private publish(update: Partial<ReaderView>) { this.value = Object.freeze({ ...this.value, ...update }); for (const listener of this.listeners) listener(this.value); }
  sync() {
    if (this.disposed) return;
    if (this.reloadMessage) { this.publish({ status: 'restart', pending: !!this.active, routing: !!this.routing, requiresReload: true, message: this.reloadMessage }); return; }
    let key: string, context: ReaderContext;
    try { context = this.context(); key = contextKey(context); } catch (error) {
      this.key = ''; this.generation++; this.active?.cancel(); this.scan = undefined; this.recent = []; this.continueIntent = undefined;
      this.value = empty(); this.publish({ status: 'error', pending: !!this.active, routing: !!this.routing, message: error instanceof Error ? error.message : 'Reader context is invalid.' }); return;
    }
    if (key === this.key) return;
    // A title with no prepared selection has nothing stale to ask the reader to
    // refresh. Still advance ownership and discard caches on every change.
    // In-flight work, chapter neighbors, edition choices and scans must retain
    // the explicit restart requirement; catalogue changes are fenced separately.
    const quietTitle = !!this.key && !context.current && !this.active && !this.routing &&
      ['idle', 'ready'].includes(this.value.status) && !this.value.current && !this.value.next && !this.value.previous &&
      !this.value.groups.length && !this.value.editions.length && !this.value.groupAnchor && !this.value.candidate && !this.scan && !this.continueIntent;
    const hadKey = !!this.key; this.key = key; this.generation++; this.active?.cancel(); this.continueIntent = undefined;
    this.groupsCursor = this.editionCursor = null; this.groupsScope = this.editionScope = undefined; this.recent = []; this.scan = undefined;
    this.value = empty(); this.publish({ pending: !!this.active, routing: !!this.routing, status: quietTitle ? 'ready' : hadKey ? 'restart' : 'idle', message: this.routing ? 'The reader context changed after navigation was dispatched. Waiting for the router; its navigation cannot be cancelled here.' : hadKey && !quietTitle ? 'Your reading options changed. Refresh reading controls to continue.' : '' });
  }
  requireReload(message: string) { this.reloadMessage ??= message; this.generation++; this.active?.cancel(); this.scan = undefined; this.recent = []; this.continueIntent = undefined; this.value = empty(); this.publish({ status: 'restart', pending: !!this.active, routing: !!this.routing, requiresReload: true, message: this.reloadMessage }); }
  cancel() { if (this.reloadMessage) { this.sync(); return; } if (this.routing) { this.publish({ routing: true, message: 'Navigation is already owned by the router and cannot be cancelled here. Waiting for it to settle.' }); return; } this.generation++; this.active?.cancel(); this.scan = undefined; this.recent = []; this.continueIntent = undefined; this.publish({ pending: !!this.active, status: 'restart', groupAnchor: undefined, winner: undefined, editions: [], editionsMore: false, message: 'Reader action cancelled. Refresh or restart before continuing.' }); }
  dispose() { this.disposed = true; this.generation++; this.active?.cancel(); this.listeners.clear(); this.value = empty(); this.scan = undefined; this.recent = []; this.continueIntent = undefined; }
  dismissSelection(): boolean {
    if (this.disposed || this.active || this.reloadMessage) return false;
    this.generation++; this.continueIntent = undefined; this.editionCursor = null; this.editionScope = undefined; this.recent = [];
    this.publish({ groupAnchor: undefined, winner: undefined, editions: [], editionsMore: false, candidate: undefined, message: '' }); return true;
  }
  private async action(work: (context: ReaderContext, budget: ReaderBudget, commitVerifiedContext: (commit: () => void) => ReaderContext) => Promise<void>): Promise<void> {
    if (this.disposed) return; this.sync(); if (this.reloadMessage || !this.key || this.active) return;
    let context: ReaderContext; try { context = this.context(); } catch { this.sync(); return; }
    let key = this.key; const generation = ++this.generation;
    const stillCurrent = () => { try { return key === contextKey(this.context()); } catch { return false; } };
    const assert = () => { if (this.disposed || generation !== this.generation || !stillCurrent()) throw new Restart('Reader context changed. Refresh navigation or restart the scan.'); };
    const budget = this.active = new ReaderBudget(assert);
    this.publish({ pending: true, status: 'pending', message: '' });
    const commitVerifiedContext = (commit: () => void) => {
      budget.check(); commit(); context = this.context(); key = this.key = contextKey(context);
      this.groupsCursor = this.editionCursor = null; this.groupsScope = this.editionScope = undefined; this.scan = undefined; this.recent = []; this.continueIntent = undefined;
      this.value = empty(); this.publish({ status: 'pending', pending: true }); return context;
    };
    try { await work(context, budget, commitVerifiedContext); budget.check(); }
    catch (error) {
      if (!this.disposed && generation === this.generation && stillCurrent()) {
        if (error instanceof ReaderIdentityChanged) this.requireReload(error.message);
        else {
          const status = error instanceof Excluded ? 'excluded' : error instanceof Restart ? 'restart' : 'error';
          this.publish({ status, message: error instanceof Error ? error.message : 'Reader action failed', ...(status === 'excluded' || status === 'restart' ? { current: undefined, next: undefined, previous: undefined, groups: [], editions: [], candidate: undefined, groupsMore: false, editionsMore: false } : {}) });
          if (status === 'restart' || status === 'excluded') { this.scan = undefined; this.continueIntent = undefined; this.publish({ groupAnchor: undefined, winner: undefined, hasMultipleEditions: undefined }); }
        }
      }
    } finally {
      budget.close();
      if (this.active === budget) {
        this.active = undefined; if (this.routing === budget) this.routing = undefined;
        if (!this.disposed) {
          // A router may advance the navigation epoch and later reject/cancel.
          // Never restore a stale ready view after that ownership changed.
          if (generation !== this.generation || !stillCurrent()) { this.sync(); this.publish({ pending: false, routing: false }); }
          else this.publish({ pending: false, routing: false, ...(this.value.status === 'pending' ? { status: 'ready' as const, message: '' } : {}) });
        }
      }
    }
  }
  private async route(budget: ReaderBudget, dispatch: () => Promise<void>) {
    // Only selector/history preparation has the ten-second deadline. SvelteKit
    // owns goto after dispatch; do not race-release or claim to abort it.
    budget.handoffToRouter(); this.routing = budget;
    this.publish({ routing: true, message: 'Navigation dispatched. Waiting for the router; selection requests have finished.' });
    await dispatch();
  }
  private async request(context: ReaderContext, budget: ReaderBudget, operation: Operation, controls: Partial<Omit<Request, 'operation' | 'limit' | 'current'>> = {}): Promise<SelectorPage> {
    const result = await this.read(context.destination, { operation, limit: READER_LIMITS.page, ...(context.current ? { current: context.current.id } : {}), ...controls }, budget);
    budget.check();
    if (result.state === 'excluded-current') throw new Excluded('This chapter remains directly readable, but is outside the selected navigation filters.');
    if (result.state === 'stale-anchor') throw new Restart('A selected chapter or page anchor changed. Refresh or restart; no replacement target was chosen.');
    if (context.current && (!result.current || !matchesIntent(result.current, context.current))) throw new Restart('The currently displayed chapter changed. Reload it before navigating.');
    if (this.value.current && !sameRow(this.value.current, result.current)) throw new Restart('The current chapter witness changed. Refresh navigation before continuing.');
    return result;
  }
  prepareIdentity(verify: (budget: ReaderBudget) => Promise<() => void>, refreshCurrent: boolean) { return this.action(async (_, budget, commitVerifiedContext) => {
    const commit = await verify(budget); budget.check(); const context = commitVerifiedContext(commit);
    if (!refreshCurrent || !context.current) { this.publish({ status: 'ready' }); return; }
    const result = await this.request(context, budget, 'around');
    this.publish({ status: result.state === 'boundary' ? 'boundary' : 'ready', current: result.current, next: result.next, previous: result.previous });
  }); }
  refresh() { return this.action(async (context, budget) => {
    // Explicit refresh accepts a new backend witness only after matching the
    // directly displayed chapter's exact nullable labels, identity and version.
    this.publish({ current: undefined, next: undefined, previous: undefined });
    if (!context.current) { this.publish({ status: 'ready' }); return; }
    const result = await this.request(context, budget, 'around');
    this.publish({ status: result.state === 'boundary' ? 'boundary' : 'ready', current: result.current, next: result.next, previous: result.previous });
  }); }
  semantic(operation: 'first' | 'last' | 'next' | 'previous', lastPage = false, boundary?: () => Promise<void>) { return this.action(async (context, budget) => {
    const expectedCurrent = this.value.current;
    if (context.current && !expectedCurrent) throw new Restart('Refresh navigation before choosing another chapter.');
    const expected = operation === 'next' ? this.value.next : operation === 'previous' ? this.value.previous : undefined;
    let result = await this.request(context, budget, operation), target = result.data[0] ?? null;
    if (operation === 'next' || operation === 'previous') {
      if (expected === undefined || !sameRow(expected, target)) throw new Restart('The requested neighbor changed. Refresh navigation before choosing it.');
    } else {
      // Title Start and keyboard first/last have no previously presented target.
      // Capture a semantic result, then revalidate the same operation before goto.
      const rechecked = await this.request(context, budget, operation);
      if (!sameRow(result.current, rechecked.current) || !sameRow(target, rechecked.data[0] ?? null)) throw new Restart('The first or last chapter changed. Retry the selection.');
      result = rechecked; target = rechecked.data[0] ?? null;
    }
    budget.check();
    if (!target) { this.publish({ status: 'boundary', message: 'No chapter in that direction in the current live view.' }); if (boundary) { budget.check(); await this.route(budget, boundary); } return; }
    if (target.id === context.current?.id) { this.publish({ status: 'boundary', message: 'You are already at this chapter boundary.' }); return; }
    await this.route(budget, () => this.navigate(target, lastPage));
  }); }
  start() { return this.action(async (context, budget) => {
    this.continueIntent = undefined; this.publish({ groupAnchor: undefined, winner: undefined, editions: [], editionsMore: false, candidate: undefined });
    if (context.current) throw Error('Title Start must not inherit a currently read chapter.');
    const first = await this.request(context, budget, 'first');
    const checked = await this.request(context, budget, 'first');
    const target = checked.data[0];
    if (!sameRow(first.data[0] ?? null, target ?? null) || first.hasMultipleEditions !== checked.hasMultipleEditions) throw new Restart('The first chapter changed. Restart Start Reading.');
    if (!target) { this.publish({ status: 'boundary', message: 'No eligible chapter is visible in this live view.' }); return; }
    if (!checked.hasMultipleEditions) { budget.check(); await this.route(budget, () => this.navigate(target, false)); return; }
    const page = await this.request(context, budget, 'editions', { groupChapter: target.id });
    if (!sameRow(target, page.groupAnchor) || !sameRow(target, page.winner)) throw new Restart('The first group changed before edition selection. Restart Start Reading.');
    this.editionScope = page.scope; this.editionCursor = page.nextCursor; this.recent = [];
    this.observe(page.data, 'e:');
    this.publish({ status: 'ready', editions: page.data, editionsMore: !!page.nextCursor, groupAnchor: target, winner: page.winner, hasMultipleEditions: page.hasMultipleEditions, message: 'Choose a translation of the first chapter.' });
  }); }
  private async checkedContinue(context: ReaderContext, budget: ReaderBudget, expected?: NativeContinueResult) {
    const access = context.nativeContinue;
    if (context.current || !access?.available) throw Error(access?.reason ?? 'Native-account Continue is unavailable. No history fallback was used.');
    const result = await access.read(context.destination.filters, budget); budget.check();
    if (expected && !sameContinueSelection(expected, result)) throw new Restart('The native Continue selection changed. Retry Continue; no replacement group was selected.');
    return result;
  }
  continueNative() { return this.action(async (context, budget) => {
    this.continueIntent = undefined; this.editionCursor = null; this.editionScope = undefined; this.recent = [];
    this.publish({ groupAnchor: undefined, winner: undefined, editions: [], editionsMore: false, candidate: undefined });
    const first = await this.checkedContinue(context, budget), checked = await this.checkedContinue(context, budget, first);
    const winner = checked.winner;
    if (!winner) { this.publish({ status: 'boundary', message: 'No chapters match your current reading options.' }); return; }
    if (!checked.hasMultipleEditions) {
      const page = context.nativeContinue?.resume ? await context.nativeContinue.resume(winner, checked, budget) : 1;
      if (context.nativeContinue?.resume) await this.checkedContinue(context, budget, checked);
      budget.check(); await this.route(budget, () => this.navigate(winner, false, page)); return;
    }
    if (!budget.fits(1, READER_LIMITS.page + 2, READER_LIMITS.bytes)) throw Error('Native Continue chooser budget exceeded');
    const page = await this.request(context, budget, 'editions', { groupChapter: winner.id });
    if (!sameRow(winner, page.groupAnchor) || !sameRow(winner, page.winner) || page.hasMultipleEditions !== true) throw new Restart('The native Continue group changed before edition selection. Retry Continue.');
    const access = context.nativeContinue; if (!access) throw new Restart('Native Continue admission changed.');
    this.continueIntent = { key: access.key, result: checked }; this.editionScope = page.scope; this.editionCursor = page.nextCursor; this.observe(page.data, 'e:');
    this.publish({ status: 'ready', groupAnchor: winner, winner: page.winner, editions: page.data, editionsMore: !!page.nextCursor, hasMultipleEditions: true,
      message: checked.reason === 'all-completed-newest' ? 'You’re caught up. Choose a translation of the latest chapter to read again.' : 'Choose a translation to continue reading.' });
  }); }
  private async recheckContinueIntent(context: ReaderContext, budget: ReaderBudget) {
    const intent = this.continueIntent;
    if (!intent) return;
    if (context.nativeContinue?.key !== intent.key) throw new Restart('The native Continue account or context changed. Retry Continue.');
    await this.checkedContinue(context, budget, intent.result); budget.check();
  }
  private observe(rows: readonly Row[], prefix: string) {
    const keys = rows.map(row => prefix + (prefix === 'g:' ? JSON.stringify([row.volume, row.chapter]) : row.id));
    if (keys.some(key => this.recent.includes(key))) throw new Restart('A recently visited row reappeared after live catalogue drift. Restart the page.');
    this.recent = [...this.recent, ...keys].slice(-READER_LIMITS.recentKeys);
  }
  groups(more = false) { return this.action(async (context, budget) => {
    if (more && !this.groupsCursor) return;
    if (!more) { this.groupsCursor = null; this.groupsScope = undefined; this.recent = []; }
    const result = await this.request(context, budget, 'groups', { direction: 'newest', ...(this.groupsCursor ? { after: this.groupsCursor } : {}) });
    if (this.groupsScope && result.scope !== this.groupsScope) throw new Restart('The chapter page scope changed.');
    this.observe(result.data, 'g:'); this.groupsScope = result.scope; this.groupsCursor = result.nextCursor;
    this.publish({ status: result.state === 'boundary' ? 'boundary' : 'ready', groups: result.data, groupsMore: !!result.nextCursor, message: 'Live chapter page. New or moved rows behind this cursor require a restart.' });
  }); }
  chooseGroup(selected: Row) { return this.action(async (context, budget) => {
    const result = await this.request(context, budget, 'resolve-group', { groupChapter: selected.id });
    if (!sameRow(selected, result.groupAnchor) || !sameRow(selected, result.winner)) throw new Restart('The selected group representative or edition winner changed. Reload this chapter page.');
    if (!result.winner) throw new Restart('The selected chapter group is no longer available.');
    budget.check(); const winner = result.winner; await this.route(budget, () => this.navigate(winner, false));
  }); }
  editions(anchor?: Row, more = false) { return this.action(async (context, budget) => {
    const selected = anchor ?? this.value.groupAnchor ?? this.value.current;
    if (!selected) throw new Restart('Refresh navigation before opening editions.');
    if (this.continueIntent && !sameRow(selected, this.continueIntent.result.winner)) throw new Restart('The native Continue group anchor changed. Retry Continue.');
    await this.recheckContinueIntent(context, budget); budget.check();
    if (more && !this.editionCursor) return;
    if (!more) { this.editionCursor = null; this.editionScope = undefined; this.recent = []; }
    const result = await this.request(context, budget, 'editions', { groupChapter: selected.id, ...(this.editionCursor ? { after: this.editionCursor } : {}) });
    if (!sameRow(selected, result.groupAnchor) || this.editionScope && result.scope !== this.editionScope) throw new Restart('The selected edition group changed. Reopen editions.');
    if (this.continueIntent && (!sameRow(result.winner, this.continueIntent.result.winner) || result.hasMultipleEditions !== true)) throw new Restart('The native Continue edition winner changed. Retry Continue.');
    this.observe(result.data, 'e:'); this.editionScope = result.scope; this.editionCursor = result.nextCursor;
    this.publish({ status: result.state === 'boundary' ? 'boundary' : 'ready', editions: result.data, editionsMore: !!result.nextCursor, groupAnchor: selected, winner: result.winner, hasMultipleEditions: result.hasMultipleEditions });
  }); }
  chooseEdition(selected: Row) { return this.action(async (context, budget) => {
    const group = this.value.groupAnchor;
    if (!group || !sameGroup(group, selected)) throw new Restart('The selected edition belongs to a different displayed group.');
    if (this.continueIntent) {
      await this.recheckContinueIntent(context, budget); budget.check();
      const access = context.nativeContinue; if (!access?.available) throw new Restart('Native Continue admission changed.');
      const selectedFilters = (context.destination.profile === 'public-retained-v1' ? singletonAccountContinueFilters : singletonContinueFilters)(context.destination.filters, selected.id);
      const checked = await access.read(selectedFilters, budget); budget.check();
      // This different singleton scope proves only the selected edition's current
      // account/eligibility/row/group, never the original Continue semantics.
      if (!checked.winner || checked.hasMultipleEditions || !sameRow(selected, checked.winner) || !sameGroup(group, checked.winner)) throw new Restart('The selected Continue edition changed. Retry Continue.');
      const winner = checked.winner, page = access.resume ? await access.resume(winner, checked, budget) : 1;
      if (access.resume) {
        await this.recheckContinueIntent(context, budget);
        const final = await access.read(selectedFilters, budget);
        if (!sameContinueSelection(checked, final)) throw new Restart('The selected Continue edition changed while checking its saved page. Retry Continue.');
      }
      budget.check(); await this.route(budget, () => this.navigate(winner, false, page)); return;
    }
    const result = await this.request(context, budget, 'resolve-group', { groupChapter: selected.id });
    if (!sameRow(selected, result.groupAnchor) || !sameGroup(group, result.groupAnchor!)) throw new Restart('The selected edition changed. Reload editions.');
    budget.check(); await this.route(budget, () => this.navigate(selected, false));
  }); }
  continueScan(restart = false) { return this.action(async (context, budget) => {
    if (context.history.status !== 'complete-local' && context.history.status !== 'available-native') throw Error(context.history.reason ?? 'Completion history is not fully available. No empty history or device fallback was assumed.');
    if (restart || !this.scan) { this.scan = { groups: [], index: 0, end: false, completed: false }; this.recent = []; }
    const scan = this.scan; this.publish({ candidate: undefined });
    // Stop before issuing work that cannot fit the action envelope. Progress is
    // committed after each fully validated page, never after a failed request.
    const fits = (selectors: number, keys: number) => {
      const native = context.history.status === 'available-native' && keys > 0;
      return budget.fits(selectors + Number(native), selectors * (READER_LIMITS.page + 6) + keys, selectors * READER_LIMITS.bytes + (native ? NATIVE_COMPLETION_LIMITS.bytes : 0));
    };
    for (;;) {
      budget.check();
      if (!scan.anchor) {
        if (scan.index >= scan.groups.length) {
          if (scan.end) { this.publish({ status: 'observed-end', message: 'No unread candidate was observed in the scanned pages. This live scan cannot prove all-read or the oldest unread chapter. Restart to look for changes.' }); return; }
          if (!fits(1, 0)) break;
          const page = await this.request(context, budget, 'groups', { direction: 'oldest', ...(scan.groupAfter ? { after: scan.groupAfter } : {}) });
          if (scan.groupScope && scan.groupScope !== page.scope) throw new Restart('Continue scan scope changed. Restart the scan.');
          this.observe(page.data, 'g:'); scan.groupScope = page.scope; scan.groups = page.data; scan.index = 0; scan.groupAfter = page.nextCursor ?? undefined; scan.end = !page.nextCursor;
          if (!page.data.length) continue;
        }
        const anchor = scan.groups[scan.index];
        if (!fits(0, 1)) break;
        const completed = await context.history.completed([anchor.id], budget); budget.check();
        // A failed lookup must not leave a new anchor paired with the previous
        // group's completion. Commit the observed pair only after settlement.
        scan.anchor = anchor; scan.completed = completed; scan.editionAfter = undefined; scan.editionScope = undefined;
      }
      if (!scan.completed) {
        // Reserve the full selector plus selected-key lookup before dispatching
        // either. On pause, this uninspected anchor remains in the scan.
        if (!fits(1, READER_LIMITS.page)) break;
        const page = await this.request(context, budget, 'editions', { groupChapter: scan.anchor.id, ...(scan.editionAfter ? { after: scan.editionAfter } : {}) });
        if (!sameRow(scan.anchor, page.groupAnchor) || scan.editionScope && scan.editionScope !== page.scope) throw new Restart('An observed Continue group changed. Restart the scan.');
        const completed = await context.history.completed(page.data.map(row => row.id), budget); budget.check();
        this.observe(page.data, 'e:'); scan.editionScope = page.scope; scan.completed = completed;
        scan.editionAfter = page.nextCursor ?? undefined;
        if (!scan.completed && page.nextCursor) continue;
        if (!scan.completed) {
          this.publish({ status: 'candidate', candidate: scan.anchor, message: 'Observed unread candidate, not a proof of oldest unread. No completed edition was observed in this live scan. Inspect or open this candidate, continue scanning, or restart.' });
          scan.index++; scan.anchor = undefined; return;
        }
      }
      // Any observed completed edition marks only this observed group complete.
      scan.index++; scan.anchor = undefined;
    }
    this.publish({ status: 'paused', message: 'This bounded scan paused. Continue scanning or restart to look for changes behind the cursor.' });
  }); }
  openCandidate() { const candidate = this.value.candidate; return candidate ? this.chooseGroup(candidate) : Promise.resolve(); }
}
