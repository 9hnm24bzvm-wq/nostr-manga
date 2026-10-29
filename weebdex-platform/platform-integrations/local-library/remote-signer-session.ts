// Optional portable signing only. Never used by native contribution/auth clients.
import { get, writable } from 'svelte/store';
import { Session } from '$lib/stores';
import { createIdentityLinkClient } from './native-identity-link';
import { capabilities, disconnectSigner, installRemoteSigner, signerIdentity } from './publication';
import { connectBunker, reviewBunker, type BunkerReview } from './remote-bunker';
type RemoteStatus = { connected: boolean; pending: boolean; authURL: string | null; message: string };
export const remoteSignerStatus = writable<RemoteStatus>({ connected: false, pending: false, authURL: null, message: '' });
let currentAbort: AbortController | undefined, generation = 0;
export function cancelRemoteSigner() { currentAbort?.abort(); if (get(signerIdentity).source === 'remote') disconnectSigner(); }
export async function prepareRemoteSigner(input: string, assertCurrent: () => void) {
  assertCurrent(); const policy = await capabilities(); assertCurrent();
  return reviewBunker(input, policy.connect_origins, window.location.origin);
}
export async function selectRemoteSigner(review: BunkerReview, approved: boolean, assertReview: () => void) {
  assertReview(); if (!approved) throw Error('Approve the exact signer and relay first.');
  cancelRemoteSigner(); disconnectSigner(); const epoch = ++generation, abort = currentAbort = new AbortController();
  const account = get(Session)?.identity?.id ?? null, selectionGeneration = get(signerIdentity).generation;
  let installed = false, owner = '', linkedOnce = false;
  let client: Awaited<ReturnType<typeof createIdentityLinkClient>> | undefined;
  const status = (patch: Partial<RemoteStatus>) => { if (generation === epoch) remoteSignerStatus.update(value => ({ ...value, ...patch })); };
  function check() {
    if (abort.signal.aborted || generation !== epoch || (get(Session)?.identity?.id ?? null) !== account) throw Error('Account or signer connection changed. Review it again.');
    if (!installed) { assertReview(); if (get(signerIdentity).generation !== selectionGeneration) throw Error('Selected signing identity changed. Review this connection again.'); }
  }
  const unsubscribe = Session.subscribe(value => { if ((value?.identity?.id ?? null) !== account) abort.abort(); });
  function dispose() { abort.abort(); unsubscribe(); client?.dispose(); if (generation === epoch) { currentAbort = undefined; status({ connected: false, pending: false, authURL: null, message: 'Remote signer disconnected. Reconnect explicitly when needed.' }); } }
  async function verifyRelayPolicy() { const policy = await capabilities(); check(); if (!policy.connect_origins.includes(new URL(review.relay).origin) || review.appOrigin !== window.location.origin) { dispose(); throw Error('Signer relay policy or site origin changed. Review the connection again.'); } }
  async function readConnection() { try { check(); const value = await client?.read(); check(); return value; } catch (error) { dispose(); throw error; } }
  const leaving = () => dispose(); window.addEventListener('pagehide', leaving, { once: true });
  abort.signal.addEventListener('abort', () => { window.removeEventListener('pagehide', leaving); unsubscribe(); client?.dispose(); if (generation === epoch) { currentAbort = undefined; status({ connected: false, pending: false, authURL: null, message: 'Remote signer connection cancelled. Late results will not be used.' }); } }, { once: true });
  try {
    status({ connected: false, pending: true, authURL: null, message: 'Connecting to your reviewed remote signer…' });
    await verifyRelayPolicy();
    if (account) client = await createIdentityLinkClient(account, check); check();
    const baseline = await readConnection(); check(); linkedOnce = !!baseline?.publicKey;
    const candidate = await connectBunker(review, {
      approved: true, signal: abort.signal, assertCurrent: check,
      onRequest(method) { status({ pending: method !== null, authURL: null, message: method ? 'Waiting for your remote signer. Approve only the public change you reviewed.' : '' }); },
      onAuthURL(url) { status({ authURL: url, pending: true, message: 'Your signer asks you to approve on its website. Check the domain before opening it.' }); },
      async beforeSign() {
        check(); await verifyRelayPolicy(); const before = await readConnection(); check();
        if (before && ((before.publicKey && before.publicKey !== owner) || linkedOnce && !before.publicKey)) { dispose(); throw Error('The connected identity changed. Reconnect from Account settings.'); }
        if (before?.publicKey) linkedOnce = true;
        return async () => { check(); const after = await readConnection(); check(); if (before && (before.revision !== after?.revision || before.publicKey !== after.publicKey)) { dispose(); throw Error('The account identity connection changed while signing. The late signature was discarded.'); } };
      },
    });
    owner = await candidate.getPublicKey(); check();
    const after = await readConnection(); check();
    if (baseline && (baseline.revision !== after?.revision || baseline.publicKey !== after.publicKey || baseline.publicKey && baseline.publicKey !== owner)) { candidate.clear(); throw Error('This signer does not match the current account connection. Nothing was linked or published.'); }
    candidate.onClose(dispose); installRemoteSigner(owner, candidate, check); installed = true;
    status({ connected: true, pending: false, authURL: null, message: candidate.relayPreferencesAvailable ? 'Remote identity selected for this session. Every public change still needs your review.' : 'Remote identity selected. The signer did not provide relay preferences; only your exact reviewed relay is used. Every public change still needs your review.' });
    return owner;
  } catch (error) { dispose(); throw error; }
}
