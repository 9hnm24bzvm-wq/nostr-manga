// Dependency-free authority registry: HTTP must not import Session through its API cycle.
export type NativeAuthority = Readonly<{ actor?: string; generation: number; epoch: number }>;
let current: NativeAuthority = Object.freeze({ generation: 0, epoch: 0 });
const listeners = new Set<(value: NativeAuthority) => void>();
let bound = false, authorityKey = "anonymous";
export function bindNativeMutationAuthority(store: { subscribe(callback: (value: any) => void): () => void }) {
  if (bound) throw Error('Native mutation authority already bound');
  bound = true;
  store.subscribe(value => {
    const id = value?.identity?.id, actor = typeof id === 'string' && id.length ? id : undefined;
    const key = JSON.stringify([actor ?? null, [...(value?.roles ?? [])].sort(), [...(value?.permissions ?? [])].sort(), !!value?.identity?.is_verified]);
    // Data refresh revisions still fence stale writes, but only actual authority
    // changes advance lifecycle epoch. History/follow refresh must not remount Reader.
    const epoch = current.epoch + (key === authorityKey ? 0 : 1); authorityKey = key;
    current = Object.freeze({ actor, generation: current.generation + 1, epoch });
    for (const callback of [...listeners]) callback(current);
  });
}
export const nativeAuthority = { subscribe(callback: (value: NativeAuthority) => void) { listeners.add(callback); callback(current); return () => { listeners.delete(callback); }; } };
export const captureNativeAuthority = () => current;
export const sameNativeAuthority = (captured: NativeAuthority) => captured === current;
// Personal reads/lifetimes depend on authority, not unrelated follow/history data refreshes.
export const sameNativeReadAuthority = (captured: NativeAuthority) => captured.actor === current.actor && captured.epoch === current.epoch;
export function isNativeAPIRequest(value: string, apiBase: string) {
  const base = new URL(apiBase, window.location.href), target = new URL(value, window.location.href);
  const prefix = base.pathname.replace(/\/$/, '');
  return base.origin === window.location.origin && target.origin === base.origin && (target.pathname === prefix || target.pathname.startsWith(prefix + '/'));
}
