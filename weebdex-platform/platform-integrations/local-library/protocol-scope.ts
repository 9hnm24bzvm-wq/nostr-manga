import type { LocalLibrary } from './protocol/store.js';
// Additive key-scoped canonical stores. The old unscoped database is never
// imported, renamed, cleared or attributed automatically.
export type ProtocolSelection = { origin: string; owner: string | null; generation: number };
export function canonicalProtocolDatabase(selection: ProtocolSelection, requireOwner = true): string {
  if (new URL(selection.origin).origin !== selection.origin || !['http:', 'https:'].includes(new URL(selection.origin).protocol)) throw Error('Invalid canonical library origin');
  if (selection.owner === null) { if (requireOwner) throw Error('Connect the intended signing identity before opening its canonical library'); return 'weebdex-protocol-settings-v2:' + encodeURIComponent(selection.origin); }
  if (!/^[0-9a-f]{64}$/.test(selection.owner)) throw Error('Invalid canonical library owner');
  return 'weebdex-protocol-library-key-v2:' + encodeURIComponent(selection.origin) + ':' + selection.owner;
}
export function createScopedProtocolAccess<T extends object>(options: { selection(): ProtocolSelection; assertCurrent(selected: ProtocolSelection): void; open(name: string): Promise<T> }) {
  const opened = new Map<string, Promise<T>>();
  return async (requireOwner = true) => {
    const selected = Object.freeze({ ...options.selection() }), name = canonicalProtocolDatabase(selected, requireOwner);
    const check = () => options.assertCurrent(selected); check();
    if (!opened.has(name)) { const pending = options.open(name); opened.set(name, pending); void pending.catch(() => { if (opened.get(name) === pending) opened.delete(name); }); }
    const store = await opened.get(name)!; check();
    const protocol = new Proxy(store, { get(target, property) { check(); const value = Reflect.get(target, property, target); if (typeof value !== 'function') return value; return (...args: unknown[]) => { check(); const result = value.apply(target, args); if (result && typeof result.then === 'function') return Promise.resolve(result).then(value => { check(); return value; }); check(); return result; }; } });
    return { protocol, check, owner: selected.owner, generation: selected.generation, name, completionStore: (id: string) => { check(); if (!selected.owner) throw Error('A key-owned job is required'); return ownerBoundJobCompletion(store as unknown as LocalLibrary, selected.owner, id); } };
  };
}

// Only the original delivery loop's post-response cancellation read and exact
// receipt save use this handle. It cannot start a signature/network request or
// access another job/key. The ordinary scope Proxy stays fully fenced.
export function ownerBoundJobCompletion(store: Pick<LocalLibrary, 'job' | 'saveJob'>, owner: string, id: string): Pick<LocalLibrary, 'job' | 'saveJob'> {
  if (!/^[0-9a-f]{64}$/.test(owner) || !/^[0-9a-f]{64}$/.test(id)) throw Error('Invalid job completion identity');
  const exact = (job: Awaited<ReturnType<LocalLibrary['job']>>) => { if (job && (job.id !== id || job.event.id !== id || job.event.pubkey !== owner)) throw Error('Completion belongs to a different immutable job or key'); return job; };
  return { job: async requested => { if (requested !== id) throw Error('Completion is bound to one exact job'); return exact(await store.job(id)); }, saveJob: async job => { exact(job); await store.saveJob(job); } };
}
