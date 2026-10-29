import { ReaderBudget } from './client';
import type { CompletionAccess } from './completion-contract';
export type BadgeContext = Readonly<{ key: string; target: string; ids: readonly string[] }>;
export type BadgeView = Readonly<{ key?: string; status: 'unknown' | 'pending' | 'ready' | 'error'; message: string; supportDetails?: string; values?: Readonly<Record<string, boolean>> }>;
// One in-memory mutation fence survives consumer remount. It owns NO transport
// or storage slot and never reconciles an uncertain write by reading a badge.
let mutationFence: { token: object; uncertain: boolean } | undefined;
const mutationListeners = new Set<() => void>();
const mutationMessage = () => mutationFence?.uncertain ? 'The update was not reliably acknowledged. Reload and check its outcome before changing read status again.' : 'Saving read status. Waiting for acknowledgement.';
const notifyMutation = () => { for (const listener of mutationListeners) listener(); };
// This projection is deliberately NOT the shared whole-title Continue history.
export class VisibleChapterBadges {
  private value: BadgeView = Object.freeze({ status: 'unknown', message: 'Read status has not been verified.' });
  private listeners = new Set<(value: BadgeView) => void>();
  private generation = 0; private key = ''; private disposed = false;
  private active?: ReaderBudget; private again = false;
  private mutationChanged = () => { this.invalidate(); this.sync(); };
  constructor(private readonly context: () => BadgeContext, private readonly prepare: (budget: ReaderBudget) => Promise<CompletionAccess>) { mutationListeners.add(this.mutationChanged); }
  get snapshot() { return this.value; }
  subscribe = (listener: (value: BadgeView) => void) => { this.listeners.add(listener); listener(this.value); return () => { this.listeners.delete(listener); }; };
  private publish(value: BadgeView) { this.value = Object.freeze(value); for (const listener of this.listeners) listener(this.value); }
  valueFor(view: BadgeView, id: string): boolean | undefined {
    try { return !mutationFence && view.status === 'ready' && view.key === this.context().key ? view.values?.[id] : undefined; } catch { return undefined; }
  }
  sync() {
    if (this.disposed) return;
    let key: string;
    try { key = this.context().key; } catch { key = ''; }
    if (key === this.key) return;
    this.key = key; this.generation++; this.active?.cancel();
    this.publish({ status: 'unknown', message: mutationFence ? mutationMessage() : 'Read status is being verified for this page.' });
    if (!mutationFence && key) { if (this.active) this.again = true; else void this.refresh(); }
  }
  invalidate(message = 'Read status has not been verified for this page.') { this.key = ''; this.generation++; this.active?.cancel(); this.publish({ status: 'unknown', message: mutationFence ? mutationMessage() : message }); }
  async refresh(): Promise<void> {
    if (this.disposed || mutationFence) return;
    if (this.active) { this.again = true; return; }
    let context: BadgeContext;
    try { context = this.context(); } catch { this.invalidate(); return; }
    const ids = Object.freeze([...context.ids]), key = this.key = context.key, generation = ++this.generation;
    const current = () => { if (this.disposed || mutationFence || generation !== this.generation || this.context().key !== key) throw Error('Your reading options changed. Read status needs to be checked again.'); };
    const budget = this.active = new ReaderBudget(current);
    this.publish({ status: 'pending', message: 'Checking read status for this page.' });
    try {
      budget.check();
      if (ids.length > 100 || new Set(ids).size !== ids.length || ids.some(id => typeof id !== 'string' || !id.length)) throw Error('Visible chapter coverage must contain at most 100 distinct accepted IDs.');
      if (!budget.fits(3, ids.length, 65536 + 2 * 16384)) throw Error('Visible chapter page exceeds the shared action budget.');
      const values: Record<string, boolean> = Object.create(null);
      if (ids.length) {
        const access = await this.prepare(budget); budget.check();
        if (access.status !== 'complete-local' && access.status !== 'available-native') throw Error(access.reason ?? 'Selected history is unavailable.');
        // Sequential bounded reads share ONE captured context and deadline.
        for (let offset = 0; offset < ids.length; offset += 50) {
          const batch = ids.slice(offset, offset + 50), rows = await access.projection(batch, budget); budget.check();
          if (rows.length !== batch.length || rows.some((row, index) => row.id !== batch[index] || typeof row.completed !== 'boolean')) throw Error('Visible chapter completion coverage is incomplete. No partial badges were published.');
          for (const row of rows) values[row.id] = row.completed;
        }
      }
      budget.check();
      if (Object.keys(values).length !== ids.length) throw Error('Visible chapter completion coverage is incomplete.');
      // The only coverage publication is after ALL exact batches and final fence.
      this.publish({ key, status: 'ready', message: '', values: Object.freeze(values) });
    } catch (error) {
      if (!this.disposed && generation === this.generation) {
        // Route/identity epochs can change during an awaited read before a store
        // notification reaches this owner. Reject the old result, then read the
        // newly admitted context after this budget settles. Never retry a failed
        // read of the same context, an invalid page, or a pending/uncertain write.
        let changed = false;
        try { const nextKey = this.context().key; changed = !mutationFence && !!nextKey && nextKey !== key; } catch { /* Page/catalogue admission must be repaired explicitly. */ }
        if (changed) {
          this.key = ''; this.again = true;
          this.publish({ status: 'unknown', message: 'Updating read status for your current reading options…' });
        } else { const detail=error instanceof Error?error.message:'Read status could not be loaded.'; const cleanup=/cleanup is unconfirmed|fetch, read and cancellation/i.test(detail); this.publish({ status:'error',message:cleanup?'Your reading status is temporarily unavailable. Try again in a moment. Your saved progress has not been changed.':detail,...(cleanup?{supportDetails:detail}:{}) }); }
      }
    } finally {
      budget.close(); if (this.active === budget) this.active = undefined;
      if (this.again && !this.disposed && !mutationFence) { this.again = false; this.sync(); if (!this.active && this.key) void this.refresh(); }
    }
  }
  beginMutation() {
    if (this.disposed || mutationFence) throw Error('A read-status mutation is already pending or uncertain. Reload and check its outcome.');
    if (this.value.status !== 'ready' || this.value.key !== this.context().key) throw Error('Read status is not currently verified.');
    const token = {}, target = this.context().target;
    mutationFence = { token, uncertain: false }; this.again = false; notifyMutation();
    const uncertain = () => { if (mutationFence?.token !== token) return; mutationFence.uncertain = true; notifyMutation(); };
    return {
      // Called only after the existing mutation owner validates acknowledgement.
      // Disposal or a changed destination cannot release the surviving fence.
      acknowledged: () => {
        if (mutationFence?.token !== token) return;
        try { if (this.disposed || this.context().target !== target) { uncertain(); return; } } catch { uncertain(); return; }
        mutationFence = undefined; notifyMutation();
      },
      uncertain
    };
  }
  dispose() { mutationListeners.delete(this.mutationChanged); this.disposed = true; this.generation++; this.active?.cancel(); this.again = false; this.listeners.clear(); this.value = Object.freeze({ status: 'unknown', message: '' }); }
}
