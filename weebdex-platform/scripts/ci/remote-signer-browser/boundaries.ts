// Explicit finite seams: native account read + publication selection only.
// Remote coordinator, transport, cryptography and all UI primitives stay real.
import { writable, get } from 'svelte/store';
export const Session = writable({ identity: { id: 'SyntheticAccountA' } });
export const signerIdentity = writable<{ owner: string | null; generation: number; source: 'remote' | null }>({ owner: null, generation: 0, source: null });
let selected: { clear(): void; onClose(fn: () => void): void } | undefined;
async function state() {
  const response = await fetch('/fixture/state', { credentials: 'omit', cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(2000) });
  if (!response.ok) throw Error('Synthetic native boundary unavailable.');
  return await response.json();
}
export function startSyntheticSession() {
  let alive = true, reading = false;
  async function refresh() { if (!alive || reading) return; reading = true; try { const value = await state(); if (alive && get(Session).identity.id !== value.account) Session.set({ identity: { id: value.account } }); } finally { reading = false; } }
  const timer = setInterval(() => { void refresh().catch(() => {}); }, 40);
  return () => { alive = false; clearInterval(timer); disconnectSigner(); };
}
export async function capabilities() { return { connect_origins: [(await state()).relayOrigin] }; }
export async function createIdentityLinkClient(account: string, guard: () => void) {
  let disposed = false;
  return { async read() { guard(); if (disposed) throw Error('Synthetic native scope closed.'); const value = await state(); guard(); if (disposed || value.account !== account) throw Error('Synthetic account changed.'); return { accountId: account, publicKey: value.linkKey, revision: value.revision }; }, dispose() { disposed = true; } };
}
export function disconnectSigner() { const old = selected; selected = undefined; old?.clear(); signerIdentity.update(value => ({ owner: null, generation: value.generation + 1, source: null })); }
export function installRemoteSigner(owner: string, candidate: NonNullable<typeof selected>, guard: () => void) {
  guard(); disconnectSigner(); selected = candidate;
  signerIdentity.update(value => ({ owner, generation: value.generation + 1, source: 'remote' }));
  candidate.onClose(() => { if (selected === candidate) disconnectSigner(); });
}
export async function readMetadata(): Promise<never> { throw Error('Public profile networking is absent from this focus fixture.'); }
