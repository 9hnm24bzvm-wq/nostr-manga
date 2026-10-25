import { HistoryBudget, HistoryBudgetStop, HistoryTransportUnsettled, historyRetainedSize, HISTORY_LIMITS } from './history-pagination';
import { TitleIndex, TitleIndexStopped, type TitleIndexEntry } from './title-index';
import { followedAtInstant, followedAtOrderKey, titleOrderKey, type TitleSortRow } from './title-order';
export { TitleIndexStopped } from './title-index';
export class TitleChanged extends TitleIndexStopped { constructor() { super('A title changed its ordering or visibility since indexing. Rebuild this library scan before continuing.'); } }
export class TitleCancelled extends Error { constructor() { super('Title loading was cancelled. Continue to resume.'); } }
export type TitleCandidate = Readonly<{ id: string; followedAt: string; status: string }>;
export type TitlePage<T> = { page: number; limit: number; data: T[]; total: number | null; scanned: number; candidates: number; indexed: boolean; ready: boolean; localOrder: boolean; started: string; notice?: string };
export type TitleHydrate<T> = (batch: readonly TitleCandidate[], budget: HistoryBudget) => Promise<T[]>;
export type TitleIndexStore = Pick<TitleIndex, 'append' | 'page' | 'dispose'>;
export class TitlePager<T extends TitleSortRow> {
  private offset = 0;
  private matches = 0;
  private disposed = false;
  private stopped?: Error;
  private active?: HistoryBudget;
  private batchLimit = 100;
  private comparisonKind?: string;
  private displayed: T[] = [];
  private disposal?: Promise<void>;
  readonly started = new Date().toISOString();
  constructor(private candidates: readonly TitleCandidate[], private index: TitleIndexStore, private hydrate: TitleHydrate<T>, private assertCurrent: () => void, readonly limit = 20, private sort = 'followedAt', private direction = -1) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 100 || ![1, -1].includes(direction) || candidates.length > 2000000) throw Error('Invalid bounded title view controls');
    if (sort === 'followedAt') {
      const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
      // Lightweight local metadata only. O(N log N) work is not a measured
      // large-library performance claim. Never apply this to catalogue keys.
      const keyed = candidates.map(candidate => ({ candidate, instant: followedAtInstant(candidate.followedAt) }));
      keyed.sort((a, b) => direction * (a.instant[0] - b.instant[0] || compare(a.instant[1], b.instant[1])) || compare(a.candidate.id, b.candidate.id));
      this.candidates = keyed.map(row => row.candidate);
    }
  }
  cancel() { this.active?.controller.abort(new TitleCancelled()); }
  releaseDisplayedPage() { this.displayed = []; }
  dispose(): Promise<void> {
    if (this.disposal) return this.disposal;
    this.disposed = true; this.cancel(); this.candidates = []; this.displayed = [];
    return this.disposal = this.index.dispose();
  }
  private check() { if (this.stopped) throw this.stopped; if (this.disposed) throw new TitleCancelled(); this.assertCurrent(); }
  private key(row: T, candidate: TitleCandidate) { return this.sort === 'followedAt' ? followedAtOrderKey(row.id, candidate.followedAt, this.direction) : titleOrderKey(row, this.direction); }
  private response(page: number, data: T[] = [], ready = false, notice?: string): TitlePage<T> {
    this.displayed = data;
    const indexed = this.offset === this.candidates.length;
    return { page, limit: this.limit, data, total: indexed ? this.matches : null, scanned: this.offset, candidates: this.candidates.length, indexed, ready, localOrder: this.sort === 'followedAt', started: this.started, ...(notice ? { notice } : {}) };
  }
  private async rows(batch: readonly TitleCandidate[], budget: HistoryBudget) {
    const rows = await this.hydrate(batch, budget); budget.check();
    if (rows.length > batch.length || historyRetainedSize([this.displayed, rows]) > HISTORY_LIMITS.cacheBytes) throw new TitleIndexStopped('This title batch exceeds the bounded display-data budget. Narrow preferences or rebuild with fewer titles. No saved record was changed.');
    const wanted = new Map(batch.map(row => [row.id, row])), found = new Set<string>();
    const prepared = rows.map(row => {
      const candidate = wanted.get(row.id);
      if (!candidate || found.has(row.id)) throw Error('Title lookup returned an unrequested or duplicate identity');
      found.add(row.id);
      if (this.sort !== 'followedAt') {
        if (!Object.prototype.hasOwnProperty.call(row, '_sort_value')) throw Error('Title comparison keys are missing');
        const kind = row._sort_decimal ? 'decimal' : row._sort_value == null ? undefined : typeof row._sort_value;
        if (kind && this.comparisonKind && kind !== this.comparisonKind) throw new TitleChanged();
        if (kind) this.comparisonKind = kind;
      }
      const data = { ...row, reading_status: candidate.status, followed_at: candidate.followedAt };
      return { data, entry: { id: row.id, order: this.key(row, candidate) } };
    });
    if (historyRetainedSize([this.displayed, rows, prepared]) > HISTORY_LIMITS.cacheBytes) throw new TitleIndexStopped('The title batch exceeds its retained-data budget. Rebuild with narrower preferences; no title was skipped.');
    return prepared;
  }
  async load(page = 1): Promise<TitlePage<T>> {
    this.check();
    if (!Number.isSafeInteger(page) || page < 1 || !Number.isSafeInteger(page * this.limit)) throw Error('Invalid title page');
    if (this.active) throw Error('Title indexing is already in progress');
    const budget = this.active = new HistoryBudget(() => this.check());
    try {
      const start = (page - 1) * this.limit, end = page * this.limit;
      const localOrder = this.sort === 'followedAt';
      if (this.offset < this.candidates.length && (!localOrder || this.matches < end)) {
        // Exactly one bounded candidate batch per explicit action. The complete
        // library DTO set is never fetched into or retained in memory.
        const batch = this.candidates.slice(this.offset, this.offset + this.batchLimit);
        const before = this.matches, rows = await this.rows(batch, budget); budget.check();
        await this.index.append(rows.map(row => row.entry));
        // A settled successful transaction is acknowledged before a deadline
        // check: retry must not insert an already committed batch a second time.
        this.offset += batch.length; this.matches += rows.length; budget.check();
        // A one-batch library is already globally considered. Reuse only that
        // current batch, avoiding a second hydration within the same action.
        if (this.offset === this.candidates.length && this.offset === batch.length || localOrder && start >= before && (this.matches >= end || this.offset === this.candidates.length)) {
          rows.sort((a, b) => indexedDB.cmp(a.entry.order, b.entry.order));
          return this.response(page, rows.slice(Math.max(0, start - before), Math.max(0, end - before)).map(row => row.data), true);
        }
        return this.response(page, [], false);
      }
      const selected = await this.index.page((page - 1) * this.limit, this.limit); budget.check();
      if (selected.length !== Math.min(this.limit, Math.max(0, this.matches - start))) throw new TitleChanged();
      const ids = new Set(selected.map(row => row.id));
      const batch = this.candidates.filter(row => ids.has(row.id));
      if (batch.length !== selected.length) throw new TitleChanged();
      const hydrated = selected.length ? await this.rows(batch, budget) : [];
      const byId = new Map(hydrated.map(row => [row.entry.id, row]));
      const data = selected.map(entry => {
        const row = byId.get(entry.id);
        if (!row || indexedDB.cmp(row.entry.order, entry.order) !== 0) throw new TitleChanged();
        return row.data;
      });
      budget.check(); return this.response(page, data, true);
    } catch (error) {
      if (error instanceof TitleIndexStopped || error instanceof HistoryTransportUnsettled) this.stopped = error;
      const cause = this.stopped ?? (budget.signal.aborted ? budget.signal.reason : error); budget.controller.abort(cause);
      if (cause instanceof HistoryBudgetStop && this.offset < this.candidates.length && this.batchLimit > 1) {
        this.batchLimit = Math.max(1, Math.floor(this.batchLimit / 2));
        return this.response(page, [], false, 'The read budget was reached. Continue with a smaller exact batch; no title was skipped.');
      }
      throw cause;
    } finally { budget.close(); this.active = undefined; }
  }
}
