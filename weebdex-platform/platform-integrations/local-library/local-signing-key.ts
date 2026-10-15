// Session-only browser key ownership. Never persist, log or send secret material.
// This module deliberately has no storage, transport, clipboard or UI dependency.
import { secp256k1, schnorr } from '@noble/curves/secp256k1.js';
import { bech32 } from '@scure/base';
import { get } from 'svelte/store';
import { hash, hex, unhex, integer } from './protocol/codec.js';
import { verifyEvent } from './protocol/nostr.js';
import { installLocalSigner, signerIdentity, type LocalSessionSigner } from './publication';

export type LocalKeyDraft = Readonly<{
  publicKey: string;
  npub: string;
  exportSecret(): string;
  activate(assertCurrent?: () => void): Promise<void>;
  discard(): void;
}>;
const text = new TextEncoder();
const invalidSecret = () => Error('Enter a valid nsec private key or 64 hexadecimal private-key characters. A public npub cannot sign.');

function randomBytes() {
  if (!globalThis.crypto?.getRandomValues) throw Error('Secure browser randomness is unavailable. No key was created.');
  const bytes = new Uint8Array(32);
  try { return globalThis.crypto.getRandomValues(bytes); }
  catch { bytes.fill(0); throw Error('Secure browser randomness failed. No key was retained.'); }
}
function parseSecret(input: string): Uint8Array {
  let bytes: Uint8Array | undefined;
  try {
    if (typeof input !== 'string' || input.length > 256) throw invalidSecret();
    const value = input.trim();
    if (/^[0-9a-fA-F]{64}$/.test(value)) bytes = unhex(value.toLowerCase());
    else {
      // NIP-19 uses ordinary Bech32, not Bech32m. The library checks checksum,
      // alphabet, mixed case and padding before we admit the exact nsec prefix.
      const decoded = bech32.decode(value as `${string}1${string}`, 90);
      try {
        if (decoded.prefix !== 'nsec') throw invalidSecret();
        bytes = bech32.fromWords(decoded.words);
      } finally { decoded.words.fill(0); }
    }
    if (bytes.length !== 32 || !secp256k1.utils.isValidSecretKey(bytes)) throw invalidSecret();
    return bytes;
  } catch {
    bytes?.fill(0);
    // Never echo an input value or a decoder error containing the private key.
    throw invalidSecret();
  }
}

export function encodePublicKey(publicKey: string): string {
  const bytes = unhex(publicKey);
  // Validate the actual BIP-340 x-only curve point, not just its byte length.
  schnorr.utils.lift_x(BigInt('0x' + publicKey));
  return bech32.encode('npub', bech32.toWords(bytes));
}

function eventToSign(template: unknown, owner: string) {
  if (!template || typeof template !== 'object' || Array.isArray(template)) throw Error('Invalid signing event');
  const allowed = new Set(['pubkey', 'created_at', 'kind', 'tags', 'content']);
  if (Object.keys(template).some(key => !allowed.has(key))) throw Error('Unexpected signing event field');
  const value = structuredClone(template) as Record<string, unknown>;
  if (value.pubkey !== undefined && value.pubkey !== owner) throw Error('Signing event belongs to another public key');
  const created_at = integer(value.created_at), kind = integer(value.kind, 65535);
  if (typeof value.content !== 'string' || !Array.isArray(value.tags)) throw Error('Invalid signing event');
  const tags: string[][] = [];
  for (let index = 0; index < value.tags.length; index++) {
    const tag = value.tags[index];
    if (!Array.isArray(tag)) throw Error('Invalid signing event tag');
    const copy: string[] = [];
    for (let entry = 0; entry < tag.length; entry++) {
      if (typeof tag[entry] !== 'string') throw Error('Invalid signing event tag');
      copy.push(tag[entry]);
    }
    tags.push(copy);
  }
  const event = { pubkey: owner, created_at, kind, tags, content: value.content };
  // Include the exact signature-envelope overhead before doing private-key work.
  if (text.encode(JSON.stringify({ ...event, id: '0'.repeat(64), sig: '0'.repeat(128) })).length > 16384) throw Error('Signing event exceeds the 16 KiB budget');
  return event;
}

function ownDraft(bytes: Uint8Array, assertDraftCurrent: () => void): LocalKeyDraft {
  let owned: Uint8Array | null = bytes;
  let activated = false;
  const selectedGeneration = get(signerIdentity).generation;
  const publicKey = hex(schnorr.getPublicKey(bytes)), npub = encodePublicKey(publicKey);
  const requireDraft = () => {
    assertDraftCurrent();
    if (!owned || activated) throw Error('This key draft is no longer available. Create or import it again.');
    return owned;
  };
  return Object.freeze({
    publicKey, npub,
    exportSecret() {
      const words = bech32.toWords(requireDraft());
      try { return bech32.encode('nsec', words); }
      finally { words.fill(0); }
    },
    async activate(assertCurrent = () => {}) {
      const key = requireDraft(); assertCurrent();
      if (get(signerIdentity).generation !== selectedGeneration) throw Error('The signing identity changed during setup. Review this key again before connecting.');
      let sessionKey: Uint8Array | null = key;
      const requireSession = () => {
        if (!sessionKey) throw Error('The local signing key is locked. Import it again to continue.');
        return sessionKey;
      };
      const session: LocalSessionSigner = {
        async getPublicKey() { requireSession(); return publicKey; },
        async signEvent(template: unknown) {
          const event = eventToSign(template, publicKey);
          const digest = hash(text.encode(JSON.stringify([0, publicKey, event.created_at, event.kind, event.tags, event.content])));
          const auxiliary = randomBytes();
          try {
            const signed = { ...event, id: digest, sig: hex(schnorr.sign(unhex(digest), requireSession(), auxiliary)) };
            if (!verifyEvent(signed)) throw Error('Local signature verification failed');
            return signed;
          } finally { auxiliary.fill(0); }
        },
        clear() { sessionKey?.fill(0); sessionKey = null; },
      };
      try {
        installLocalSigner(publicKey, session, () => { requireDraft(); assertCurrent(); });
        activated = true; owned = null;
      } catch (cause) {
        session.clear(); owned = null; throw cause;
      }
    },
    // Closing an unfinished draft never touches another selected identity.
    // After transfer the registry alone owns and clears the selected session.
    discard() { owned?.fill(0); owned = null; },
  });
}

export function generateLocalKeyDraft(assertCurrent = () => {}): LocalKeyDraft {
  assertCurrent();
  // Rejection sampling uses WebCrypto only; the vetted curve library performs
  // the strict 1 <= scalar < secp256k1 order validation.
  for (let attempt = 0; attempt < 128; attempt++) {
    const bytes = randomBytes();
    if (!secp256k1.utils.isValidSecretKey(bytes)) { bytes.fill(0); continue; }
    try { assertCurrent(); return ownDraft(bytes, assertCurrent); }
    catch (cause) { bytes.fill(0); throw cause; }
  }
  throw Error('Secure key generation failed. No key was retained.');
}

export function importLocalKeyDraft(input: string, assertCurrent = () => {}): LocalKeyDraft {
  assertCurrent();
  const bytes = parseSecret(input);
  try { assertCurrent(); return ownDraft(bytes, assertCurrent); }
  catch (cause) { bytes.fill(0); throw cause; }
}
