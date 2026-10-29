// Deliberate dual-proof association only. Ordinary native clients never call a signer.
import { createNativeAccountScope, type NativeAccountDependencies } from '../public-api/native-account-scope';
import { record, publicUuid } from '../public-api/validation';
import { selectedSigner } from './publication';
import { signExact, now } from './protocol/nostr.js';
import { hash } from './protocol/codec.js';
export type IdentityLink = { accountId: string; publicKey: string | null; revision: number };
export type IdentityLinkReview = { accountId: string; publicKey: string; action: 'link' | 'unlink'; revision: number; challengeId: string; payload: string; proofUrl: string; proofPayloadHash: string; expiresAt: number; signerGeneration?: number };
export async function createIdentityLinkClient(nativeAccount: string, assertCurrent: () => void, injected: Partial<NativeAccountDependencies> = {}) {
  const scope = await createNativeAccountScope(nativeAccount, assertCurrent, injected);
  const accountId = scope.account.id, fence = scope.key('portable-identity-link'); let sending = false;
  const check = scope.check;
  function receipt(input: unknown): IdentityLink {
    const value = record(input);
    if (value.result !== 'ok' || value.authentication !== 'native-session' || value.scope !== 'private-account-association' || value.accountId !== accountId || value.publiclyListed !== false || value.grantsAuthority !== false || value.changesPublisherAuthority !== false || !(value.publicKey === null || typeof value.publicKey === 'string' && /^[0-9a-f]{64}$/.test(value.publicKey)) || !Number.isSafeInteger(value.revision) || Number(value.revision) < 0) throw Error('The account connection could not be verified. Reload before continuing.');
    return { accountId, publicKey: value.publicKey as string | null, revision: Number(value.revision) };
  }
  async function send(path: string, payload?: unknown) {
    const response = await scope.send('/identity-link' + path, payload === undefined ? 'GET' : 'POST', payload); check();
    if (response.status !== 200) {
      const errors = record(response.body).errors, first = Array.isArray(errors) && errors.length === 1 ? record(errors[0]) : {};
      const detail = first.status === response.status && typeof first.detail === 'string' && first.detail.length <= 600 ? first.detail : 'The identity connection could not be completed. Reload Account settings.';
      throw Error(detail);
    }
    return response.body;
  }
  async function read() {
    const result = receipt(await send('')); check();
    const pending = scope.fences.get(fence);
    // Equal revision is not a barrier: a previously dispatched confirm may still commit.
    if (pending) { try { if (result.revision > JSON.parse(pending).revision) scope.fences.delete(fence); } catch { /* malformed fence requires explicit new prepare */ } }
    return result;
  }
  async function prepare(action: 'link' | 'unlink', current: IdentityLink): Promise<IdentityLinkReview> {
    check(); if (sending || current.accountId !== accountId) throw Error('Account changed or a connection is still being confirmed.');
    const selected = action === 'link' ? selectedSigner() : undefined;
    const result = record(await send('/prepare', { expectedAccountId: accountId, expectedRevision: current.revision, action, ...(selected ? { publicKey: selected.owner } : {}) })); check(); selected?.check();
    const expectedKey = selected?.owner ?? current.publicKey, origin = new URL(injected.origin?.() ?? window.location.origin).origin;
    if (result.result !== 'ok' || publicUuid(result.accountId) !== accountId || result.publicKey !== expectedKey || result.revision !== current.revision || result.action !== action || typeof result.challengeId !== 'string' || !/^[0-9a-f]{64}$/.test(result.challengeId) || typeof result.payload !== 'string' || result.payload.length > 4096 || result.proofUrl !== origin + '/api/v1/account/identity-link/confirm' || result.proofPayloadHash !== hash(new TextEncoder().encode(result.payload)) || !Number.isSafeInteger(result.expiresAt) || Number(result.expiresAt) <= now() || Number(result.expiresAt) > now() + 310 || result.privateProofOnly !== true || result.grantsAuthority !== false) throw Error('The connection review was not bound to this account, identity and site. Nothing was signed.');
    const body = record(JSON.parse(result.payload));
    if (body.purpose !== 'weebdex-private-identity-link-v1' || body.challenge !== result.challengeId || body.accountId !== accountId || body.publicKey !== expectedKey || body.action !== action || body.revision !== current.revision || body.origin !== origin || body.expiresAt !== result.expiresAt) throw Error('The connection review changed. Nothing was signed.');
    // Successful prepare is a server-side serialized barrier invalidating older reviews.
    scope.fences.delete(fence);
    return Object.freeze({ ...result, ...(selected ? { signerGeneration: selected.generation } : {}) }) as IdentityLinkReview;
  }
  async function confirm(review: IdentityLinkReview, approved: boolean, assertIntent = () => {}) {
    const active = () => { check(); assertIntent(); };
    active(); if (!approved || sending || scope.fences.get(fence) || review.accountId !== accountId || review.expiresAt <= now()) throw Error('Review and approve the current connection first. If its result is uncertain, check it or begin a new review.');
    sending = true;
    try {
      let proof;
      if (review.action === 'link') {
        const selected = selectedSigner();
        if (selected.owner !== review.publicKey || selected.generation !== review.signerGeneration) throw Error('Selected public identity changed. Review again.');
        proof = await signExact({ signEvent: async (template: unknown) => { active(); return selected.signer.signEvent(template); } }, selected.owner, { kind: 27235, created_at: now(), tags: [['u', review.proofUrl], ['method', 'POST'], ['payload', review.proofPayloadHash]], content: '' }); active(); selected.check();
      }
      active(); scope.fences.set(fence, JSON.stringify({ revision: review.revision, action: review.action, publicKey: review.publicKey }));
      const result = receipt(await send('/confirm', { expectedAccountId: accountId, challengeId: review.challengeId, confirmation: review.action === 'link' ? 'connect-private-identity' : 'disconnect-private-identity', ...(proof ? { proof } : {}) })); active();
      if (result.revision !== review.revision + 1 || result.publicKey !== (review.action === 'link' ? review.publicKey : null)) throw Error('The confirmed connection differs from your review. Check the current connection before continuing.');
      scope.fences.delete(fence); return result;
    } finally { sending = false; }
  }
  return { read, prepare, confirm, check, dispose: scope.dispose, uncertain: () => !!scope.fences.get(fence) };
}
