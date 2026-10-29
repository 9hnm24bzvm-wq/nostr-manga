// Public-only NIP-19/NIP-05 lookup. No signer, secret-key handling or account authority.
import { bech32 } from '@scure/base';
import { schnorr } from '@noble/curves/secp256k1.js';
import { hex, unhex } from './protocol/codec.js';
import { bytesResponse } from './protocol/nostr.js';
import { capabilities, type Relay } from './publication';
import { readMetadata } from './protocol-identity';
export function publicIdentityKey(input: string): string {
  try {
    const value = input.trim(); if (value.length > 100) throw Error();
    let key: string;
    if (/^[0-9a-fA-F]{64}$/.test(value)) key = value.toLowerCase();
    else { const decoded = bech32.decode(value as `${string}1${string}`, 90); if (decoded.prefix !== 'npub') throw Error(); const bytes = bech32.fromWords(decoded.words); if (bytes.length !== 32) throw Error(); key = hex(bytes); }
    schnorr.utils.lift_x(BigInt('0x' + key)); return key;
  } catch { throw Error('Enter a public npub or public key. Do not paste a private recovery key.'); }
}
export function publicIdentityLabel(key: string) { return bech32.encode('npub', bech32.toWords(unhex(publicIdentityKey(key)))); }
export function nip05Address(input: string) {
  const value = input.trim().toLowerCase(), match = /^([a-z0-9_.-]{1,64})@([a-z0-9.-]{1,253})$/.exec(value);
  if (!match) throw Error('Enter a public address such as reader@example.com.');
  const host = match[2], url = new URL('https://' + host);
  if (url.hostname !== host || !host.includes('.') || host.endsWith('.') || host.split('.').some(part => !part || part.length > 63 || part.startsWith('-') || part.endsWith('-')) || /^[0-9.]+$/.test(host) || /(?:^|\.)(?:localhost|local|internal|home|lan)$/.test(host)) throw Error('Choose an HTTPS public-domain address, not a local network address.');
  return { address: value, name: match[1], origin: url.origin, url: url.origin + '/.well-known/nostr.json?name=' + encodeURIComponent(match[1]) };
}
export async function lookupPublicIdentity(input: string, relay: Relay, allowAddressLookup: boolean, assertCurrent = () => {}) {
  assertCurrent(); let key: string, address: string | undefined;
  if (input.includes('@')) {
    const target = nip05Address(input);
    if (!allowAddressLookup) throw Error('Approve contacting the address domain before looking it up.');
    const policy = await capabilities(); assertCurrent();
    // No backend URL fetch/proxy and no bypass of the deployment's CSP policy.
    if (!policy.connect_origins.includes(target.origin)) throw Error('This site does not allow contact with that address domain. Ask its operator to allow it, or use the person’s npub instead.');
    const response = await fetch(target.url, { method: 'GET', credentials: 'omit', referrerPolicy: 'no-referrer', redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(10000) }); assertCurrent();
    if (!response.ok || response.redirected || response.url && new URL(response.url).origin !== target.origin) throw Error('The public address could not be checked. No redirect or account action was followed.');
    const document = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await bytesResponse(response, 65536))); assertCurrent();
    if (!document || typeof document !== 'object' || !document.names || typeof document.names !== 'object' || !Object.hasOwn(document.names, target.name)) throw Error('That public address did not name an identity.');
    key = publicIdentityKey(document.names[target.name]);
    // NIP-05 wire keys are lowercase hexadecimal, never npub aliases.
    if (document.names[target.name] !== key) throw Error('The public address returned an invalid identity key.');
    address = target.address;
  } else key = publicIdentityKey(input);
  assertCurrent();
  const result = await readMetadata(relay, key, 0); assertCurrent();
  if (!result.event) throw Error('This public profile is still updating. Try again shortly.');
  const profile = JSON.parse(result.event.content);
  if (!profile || typeof profile !== 'object' || Array.isArray(profile)) throw Error('The public profile could not be read safely.');
  if (address && (typeof profile.nip05 !== 'string' || nip05Address(profile.nip05).address !== address)) throw Error('The profile does not confirm that public address. Use its npub and check with the person; nothing was followed or linked.');
  const field = (name: string, limit: number) => typeof profile[name] === 'string' ? profile[name].slice(0, limit) : '';
  return { publicKey: key, npub: publicIdentityLabel(key), displayName: field('display_name', 128) || field('name', 128) || 'Public identity', about: field('about', 4000), address: address ?? null, eventId: result.event.id };
}
