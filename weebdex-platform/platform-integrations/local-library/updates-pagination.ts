import { HistoryBudget, HistoryBudgetStop, HistoryTransportUnsettled, HISTORY_LIMITS, historyRetainedSize } from './history-pagination';
import { TitleIndexStopped, type TitleIndexEntry } from './title-index';
import { titleOrderKey, type TitleSortRow } from './title-order';
import { deviceTimestampInstant } from './device-time';
export class UpdatesCancelled extends Error { constructor() { super('Update loading was cancelled. Continue to resume.'); } }
export class UpdatesChanged extends TitleIndexStopped { constructor() { super('An update changed ordering, identity or visibility during this scan. Rebuild the updates scan. No saved records were changed.'); } }
export type UpdateRow = TitleSortRow & { published_at: string; relationships: { manga: { id: string } } };
export type UpdateKey = TitleIndexEntry & { mangaId: string };
export type UpdatePage<T> = { page: number; limit: number; data: T[]; total: number | null; scanned: number; parents: number; chapters: number; ready: boolean; indexed: boolean; started: string; notice?: string };
export type UpdateSource<T> = {
  primaryOrderOnly?: boolean;
  scan(parent: string, offset: number, limit: number, budget: HistoryBudget): Promise<{ total: number; data: T[] }>;
  hydrate(keys: readonly UpdateKey[], budget: HistoryBudget): Promise<T[]>;
};
type Store = { append(entries: TitleIndexEntry[], observedIds?: string[]): Promise<void>; page(offset: number, limit: number): Promise<TitleIndexEntry[]>; dispose(): Promise<void> };
function newest(row: UpdateRow): IDBValidKey[] { const [second, fraction] = deviceTimestampInstant(row.published_at); return [second, fraction]; }
export class UpdatesPager<T extends UpdateRow> {
  private parent = 0; private offset = 0; private expected?: number; private matches = 0; private chapters = 0;
  private previous?: IDBValidKey[]; private winner?: { key: UpdateKey; published: IDBValidKey[] };
  private active?: HistoryBudget; private disposed = false; private stopped?: Error; private disposal?: Promise<void>;
  private batch = 100; private displayed: T[] = []; private kind?: string;
  readonly started = new Date().toISOString();
  constructor(private parents: readonly string[], private index: Store, private source: UpdateSource<T>, private assertCurrent: () => void, readonly limit = 100, private direction = -1, private latestOnly = false) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 100 || ![1,-1].includes(direction) || parents.length > 2000000 || new Set(parents).size !== parents.length) throw Error('Invalid bounded update controls');
  }
  releaseDisplayedPage() { this.displayed = []; }
  cancel() { this.active?.controller.abort(new UpdatesCancelled()); }
  dispose(): Promise<void> { if (this.disposal) return this.disposal; this.disposed = true; this.cancel(); this.parents = []; this.displayed = []; this.winner = undefined; return this.disposal = this.index.dispose(); }
  private check() { if (this.stopped) throw this.stopped; if (this.disposed) throw new UpdatesCancelled(); this.assertCurrent(); }
  private response(page: number, data: T[] = [], ready = false, notice?: string): UpdatePage<T> {
    this.displayed = data; const indexed = this.parent === this.parents.length;
    return { page, limit: this.limit, data, total: indexed ? this.matches : null, scanned: this.parent, parents: this.parents.length, chapters: this.chapters, ready, indexed, started: this.started, ...(notice ? { notice } : {}) };
  }
  private entry(row: T): UpdateKey {
    if (!row || !Object.prototype.hasOwnProperty.call(row, '_sort_value') || !row.relationships?.manga?.id) throw new UpdatesChanged();
    const kind = row._sort_decimal ? 'decimal' : row._sort_value == null ? undefined : typeof row._sort_value;
    if (kind && this.kind && kind !== this.kind) throw new UpdatesChanged(); if (kind) this.kind = kind;
    return { id: row.id, mangaId: row.relationships.manga.id, order: titleOrderKey(row, this.direction) };
  }
  async load(page = 1): Promise<UpdatePage<T>> {
    this.check(); if (!Number.isSafeInteger(page) || page < 1 || !Number.isSafeInteger(page * this.limit)) throw Error('Invalid update page');
    if (this.active) throw Error('Update loading is already in progress');
    const budget = this.active = new HistoryBudget(() => this.check());
    let pages = 0, scannedRows = 0;
    try {
      while (this.parent < this.parents.length && pages < 20 && scannedRows + this.batch <= 1000) {
        const parent = this.parents[this.parent], result = await this.source.scan(parent, this.offset, this.batch, budget); budget.check();
        if (!Number.isSafeInteger(result.total) || result.total < 0 || !Array.isArray(result.data) || result.data.length !== Math.min(this.batch, Math.max(0, result.total - this.offset)) || this.offset > result.total || this.expected !== undefined && result.total !== this.expected) throw new UpdatesChanged();
        if (historyRetainedSize([this.displayed, result]) > HISTORY_LIMITS.cacheBytes) throw new HistoryBudgetStop('Update representation budget');
        const entries: UpdateKey[] = []; let previous = this.previous, winner = this.winner;
        for (const row of result.data) {
          const key = this.entry(row);
          const scanOrder = this.source.primaryOrderOnly ? key.order.slice(0,2) : key.order;
          const comparison = previous ? indexedDB.cmp(previous, scanOrder) : -1;
          if (key.mangaId !== parent || (this.source.primaryOrderOnly ? comparison > 0 : comparison >= 0)) throw new UpdatesChanged();
          previous = scanOrder;
          if (this.latestOnly) { const published = newest(row), comparison = winner ? indexedDB.cmp(published, winner.published) : 1; if (!winner || comparison > 0 || comparison === 0 && row.id < winner.key.id) winner = { key: { ...key, selection: JSON.stringify(published) }, published }; }
          else entries.push(key);
        }
        const offset = this.offset + result.data.length, finished = offset === result.total;
        if (this.latestOnly && finished && winner) entries.push(winner.key);
        if (historyRetainedSize([this.displayed, result, entries, winner, previous]) > HISTORY_LIMITS.cacheBytes) throw new HistoryBudgetStop('Prepared update representation budget');
        budget.check(); await this.index.append(entries, this.latestOnly ? result.data.map(row => row.id) : []);
        // Only settled committed storage advances continuation. No DTO cache.
        this.matches += entries.length; this.chapters += result.data.length;
        if (finished) { this.parent++; this.offset = 0; this.expected = undefined; this.previous = undefined; this.winner = undefined; }
        else { this.offset = offset; this.expected = result.total; this.previous = previous; this.winner = winner; }
        pages++; scannedRows += result.data.length; budget.check();
      }
      if (pages || this.parent < this.parents.length) return this.response(page);
      const start = (page - 1) * this.limit, selected = await this.index.page(start, this.limit); budget.check();
      if (selected.length !== Math.min(this.limit, Math.max(0, this.matches - start)) || selected.some(row => !row.mangaId)) throw new UpdatesChanged();
      const keys = selected as UpdateKey[], rows = keys.length ? await this.source.hydrate(keys, budget) : []; budget.check();
      if (rows.length !== keys.length || historyRetainedSize([this.displayed, rows]) > HISTORY_LIMITS.cacheBytes) throw new UpdatesChanged();
      const byId = new Map(rows.map(row => [row.id, row])); if (byId.size !== rows.length) throw new UpdatesChanged();
      const data = keys.map(key => { const row = byId.get(key.id); if (!row || row.relationships.manga.id !== key.mangaId || indexedDB.cmp(this.entry(row).order, key.order) !== 0 || this.latestOnly && key.selection !== JSON.stringify(newest(row))) throw new UpdatesChanged(); return row; });
      budget.check(); return this.response(page, data, true);
    } catch (error) {
      if (error instanceof TitleIndexStopped || error instanceof HistoryTransportUnsettled) this.stopped = error;
      const cause = this.stopped ?? (budget.signal.aborted ? budget.signal.reason : error); budget.controller.abort(cause);
      if (cause instanceof HistoryBudgetStop && pages) return this.response(page, [], false, 'This bounded action reached its read budget. Continue from the acknowledged position; no parent or chapter was skipped.');
      if (cause instanceof HistoryBudgetStop && this.parent < this.parents.length && this.batch > 1) {
        // Divisors retain exact native page offsets after a smaller retry.
        this.batch = [50,25,5,1].find(value => value < this.batch)!;
        return this.response(page, [], false, 'Read budget reached. Continue with a smaller exact page; no parent or chapter was skipped.');
      }
      throw cause;
    } finally { budget.close(); this.active = undefined; }
  }
}
