// Deliberate, memory-only NIP-46 direct bunker session. No user key generation,
// persistence, automatic reconnect, arbitrary URL proxy or native auth fallback.
import { schnorr } from '@noble/curves/secp256k1.js';
import { v2 as nip44 } from 'nostr-tools/nip44';
import { hash, hex, unhex } from './protocol/codec.js';
import { verifyEvent, now } from './protocol/nostr.js';
import type { EventTemplate, SignedEvent } from './protocol/types.d.ts';
const encoder = new TextEncoder();
export const REMOTE_SIGNING_KINDS = Object.freeze([0, 3, 5, 7, 1111, 10002, 11999, 27235]);
export type BunkerReview = Readonly<{ bunkerKey: string; relay: string; appOrigin: string; permissions: string; encryption: 'NIP-44 v2'; hasSecret: boolean }>;
const credentials = new WeakMap<BunkerReview, string>();
class RemoteRequestRefused extends Error { constructor() { super('The remote signer refused this request. Check its approval screen; nothing was retried.'); this.name = 'RemoteRequestRefused'; } }
function key(input: unknown): string {
  if (typeof input !== 'string' || !/^[0-9a-f]{64}$/.test(input)) throw Error('The signer returned an invalid public identity.');
  schnorr.utils.lift_x(BigInt('0x' + input)); return input;
}
function publicHost(host: string) {
  return host.includes('.') && !host.includes(':') && !/^[0-9.]+$/.test(host) && !host.endsWith('.') && !/(?:^|\.)(?:localhost|local|internal|home|lan)$/.test(host) && host.split('.').every(part => /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(part));
}
function relayURL(input: string, allowedOrigins: readonly string[], appOrigin: string) {
  const url = new URL(input), app = new URL(appOrigin), local = ['localhost', '127.0.0.1', '[::1]'];
  const loopback = local.includes(app.hostname) && local.includes(url.hostname) && url.protocol === 'ws:';
  if (url.username || url.password || url.hash || url.search || input.length > 1024 || (!loopback && (url.protocol !== 'wss:' || !publicHost(url.hostname)))) throw Error('Use a public WSS relay without credentials, query or fragment.');
  if (!allowedOrigins.includes(url.origin)) throw Error('This site does not allow that signer relay. Ask its operator to allow the exact relay origin.');
  return url.href;
}
export function reviewBunker(input: string, allowedOrigins: readonly string[], appOrigin: string): BunkerReview {
  try {
    if (input.length > 4096 || input !== input.trim()) throw Error();
    const url = new URL(input), relays = url.searchParams.getAll('relay'), secrets = url.searchParams.getAll('secret');
    if (url.protocol !== 'bunker:' || url.username || url.password || url.port || url.hash || url.pathname && url.pathname !== '/' || relays.length !== 1 || secrets.length > 1 || [...url.searchParams.keys()].some(name => !['relay', 'secret'].includes(name))) throw Error();
    const secret = secrets[0] ?? ''; if (encoder.encode(secret).length > 512 || /[\u0000-\u001f\u007f]/.test(secret)) throw Error();
    const review = Object.freeze({ bunkerKey: key(url.hostname), relay: relayURL(relays[0], allowedOrigins, appOrigin), appOrigin: new URL(appOrigin).origin, permissions: REMOTE_SIGNING_KINDS.map(kind => 'sign_event:' + kind).join(','), encryption: 'NIP-44 v2' as const, hasSecret: !!secret });
    credentials.set(review, secret); return review;
  } catch (error) {
    if (error instanceof Error && error.message.startsWith('This site does not allow')) throw error;
    throw Error('Use a bunker connection link from a signer you trust with exactly one relay. Do not paste a private recovery key.');
  }
}
export function forgetBunkerReview(review: BunkerReview) { credentials.delete(review); }
function authURL(input: unknown): string {
  if (typeof input !== 'string' || input.length > 2048) throw Error('Invalid signer approval address.');
  const url = new URL(input);
  if (url.protocol !== 'https:' || url.username || url.password || !publicHost(url.hostname)) throw Error('The signer approval address is not a public HTTPS site.');
  return url.href;
}
function envelope(event: unknown, bunkerKey: string, clientKey: string): event is SignedEvent {
  try {
    const e = event as SignedEvent;
    if (!e || typeof e !== 'object' || Array.isArray(e) || encoder.encode(JSON.stringify(e)).length > 60000 || e.pubkey !== bunkerKey || e.kind !== 24133 || !Number.isSafeInteger(e.created_at) || Math.abs(e.created_at - now()) > 60 || typeof e.content !== 'string' || e.content.length > 50000 || JSON.stringify(e.tags) !== JSON.stringify([['p', clientKey]])) return false;
    const id = hash(encoder.encode(JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content])));
    return e.id === id && schnorr.verify(unhex(e.sig, 64), unhex(id), unhex(bunkerKey));
  } catch { return false; }
}
export type RemoteBunkerOptions = { approved: boolean; signal: AbortSignal; assertCurrent(): void; beforeSign?(): Promise<(() => Promise<void>) | void>; onRequest?(method: string | null): void; onAuthURL?(url: string): void; socketFactory?(url: string): WebSocket; requestTimeoutMs?: number };
export async function connectBunker(review: BunkerReview, options: RemoteBunkerOptions) {
  options.assertCurrent();
  if (!options.approved || options.signal.aborted || !credentials.has(review)) throw Error('Review and approve this signer connection first.');
  let token = credentials.get(review)!; credentials.delete(review);
  const secret = schnorr.utils.randomSecretKey(), clientKey = hex(schnorr.getPublicKey(secret));
  const conversation = nip44.utils.getConversationKey(secret, review.bunkerKey);
  let socket: WebSocket | undefined, closed = false, owner = '', messages = 0, signing = false;
  const closedListeners = new Set<() => void>();
  const subscription = crypto.randomUUID(), timeoutMs = Math.min(60000, Math.max(50, options.requestTimeoutMs ?? 60000));
  let pending: { id: string; eventId: string; deadline: number; resolve(result: string): void; reject(error: Error): void; timer: ReturnType<typeof setTimeout> } | undefined;
  function close(message = 'Remote signer disconnected. Reconnect explicitly before another public action.') {
    if (closed) return; closed = true; secret.fill(0); conversation.fill(0); token = '';
    options.onRequest?.(null); for (const listener of closedListeners) { try { listener(); } catch { /* cleanup stays bounded */ } } closedListeners.clear();
    if (pending) { clearTimeout(pending.timer); pending.reject(Error(message)); pending = undefined; }
    options.signal.removeEventListener('abort', abort);
    if (socket) { try { if (socket.readyState === 1) socket.send(JSON.stringify(['CLOSE', subscription])); } catch { /* no retry */ } socket.close(); }
  }
  const abort = () => close('Signer connection cancelled. Late replies will not be used.');
  function check() { if (closed || options.signal.aborted) throw Error('Remote signer is disconnected.'); try { options.assertCurrent(); } catch (error) { close('Account or identity changed. Connect again from current Account settings.'); throw error; } }
  options.signal.addEventListener('abort', abort, { once: true });
  function complete(error: Error | undefined, result = '') { const request = pending; if (!request) return; pending = undefined; clearTimeout(request.timer); options.onRequest?.(null); if (error) request.reject(error); else request.resolve(result); }
  function receive(message: MessageEvent) {
    try {
      check(); if (typeof message.data !== 'string' || message.data.length > 65536 || ++messages > 256) { close('Signer relay exceeded its response budget. Reconnect explicitly.'); return; }
      const frame = JSON.parse(message.data);
      if (!Array.isArray(frame)) return;
      if (frame[0] === 'CLOSED' && frame[1] === subscription) { close('Signer relay closed the connection. Reconnect explicitly.'); return; }
      if (frame[0] === 'AUTH') { close('This relay requires a separate relay login, which this signer connection does not support.'); return; }
      if (frame[0] === 'OK' && pending && frame[1] === pending.eventId && frame[2] === false) { complete(Error('Signer relay refused the request. Nothing was retried.')); return; }
      if (pending && performance.now() >= pending.deadline) { close('Signer request timed out. Late replies will not be used.'); return; }
      if (frame[0] !== 'EVENT' || frame.length !== 3 || frame[1] !== subscription || !pending || !envelope(frame[2], review.bunkerKey, clientKey)) return;
      // Validate signed sender and recipient BEFORE authenticated decryption.
      const payload = JSON.parse(nip44.decrypt(frame[2].content, conversation));
      if (!payload || typeof payload !== 'object' || Array.isArray(payload) || payload.id !== pending.id) return;
      if (payload.result === 'auth_url') { const url = authURL(payload.error); if (!options.onAuthURL) throw Error('Open the signer app to approve, then reconnect explicitly.'); options.onAuthURL(url); return; }
      if (payload.error !== undefined && payload.error !== null && payload.error !== '') { if (typeof payload.error !== 'string' || payload.error.length > 1024) throw Error('Invalid signer refusal.'); complete(new RemoteRequestRefused()); return; }
      if (typeof payload.result !== 'string' || payload.result.length > 40000) throw Error('The signer returned an invalid response.');
      complete(undefined, payload.result);
    } catch { if (!closed) close('The remote signer response could not be verified. No late result will be used.'); }
  }
  async function rpc(method: 'connect' | 'get_public_key' | 'sign_event' | 'switch_relays', params: string[]) {
    check(); if (pending) throw Error('A signer approval is already pending.'); messages = 0; options.onRequest?.(method);
    const deadline = performance.now() + timeoutMs, id = crypto.randomUUID(), content = nip44.encrypt(JSON.stringify({ id, method, params }), conversation);
    const event: SignedEvent = { pubkey: clientKey, created_at: now(), kind: 24133, tags: [['p', review.bunkerKey]], content, id: '', sig: '' };
    event.id = hash(encoder.encode(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]))); event.sig = hex(schnorr.sign(unhex(event.id), secret));
    const result = new Promise<string>((resolve, reject) => { pending = { id, eventId: event.id, deadline, resolve, reject, timer: setTimeout(() => close('Signer request timed out. Late approval will not be used; reconnect explicitly.'), timeoutMs) }; });
    try { check(); if (performance.now() >= deadline) throw Error('Signer request timed out before transport.'); if (socket?.readyState !== 1) throw Error('Signer relay is disconnected.'); socket.send(JSON.stringify(['EVENT', event])); } catch { close(); return await result; }
    return result;
  }
  try {
    socket = (options.socketFactory ?? (url => new WebSocket(url)))(review.relay);
    socket.addEventListener('message', receive); socket.addEventListener('close', () => close()); socket.addEventListener('error', () => close('Signer relay could not be reached. No automatic retry was made.'));
    const connectionDeadline = performance.now() + Math.min(timeoutMs, 10000);
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => { finish(); reject(Error('Signer relay connection timed out.')); }, Math.min(timeoutMs, 10000));
      const opened = () => { finish(); resolve(); }, failed = () => { finish(); reject(Error('Signer relay connection failed or was cancelled.')); };
      function finish() { clearTimeout(timer); socket?.removeEventListener('open', opened); socket?.removeEventListener('close', failed); socket?.removeEventListener('error', failed); options.signal.removeEventListener('abort', failed); }
      socket!.addEventListener('open', opened, { once: true }); socket!.addEventListener('close', failed, { once: true }); socket!.addEventListener('error', failed, { once: true }); options.signal.addEventListener('abort', failed, { once: true });
    });
    check(); if (performance.now() >= connectionDeadline) throw Error('Signer relay connection timed out.'); socket.send(JSON.stringify(['REQ', subscription, { kinds: [24133], authors: [review.bunkerKey], '#p': [clientKey], since: now() - 5, limit: 64 }]));
    const connected = await rpc('connect', [review.bunkerKey, token, review.permissions, JSON.stringify({ name: 'WeebDex', url: review.appOrigin })]);
    if (connected !== 'ack' && (!token || connected !== token)) throw Error('The remote signer did not confirm this connection.');
    token = ''; owner = key(await rpc('get_public_key', []));
    // NIP-46 asks clients to consult relay preferences. No unreviewed switch,
    // fallback or network contact is permitted; null or same relay is safe.
    let relayPreferencesAvailable = true;
    try {
      const next: unknown = JSON.parse(await rpc('switch_relays', []));
      if (next !== null && (!Array.isArray(next) || next.length !== 1 || typeof next[0] !== 'string' || relayURL(next[0], [new URL(review.relay).origin], review.appOrigin) !== review.relay)) throw Error('The signer requested a different relay. Review a new connection link; no other relay was contacted.');
    } catch (error) {
      // Only an authenticated, correlated, bounded STRING error for this optional
      // method retains the already reviewed relay. Never catch network/timeouts,
      // malformed replies or authentication failures as compatible success.
      if (!(error instanceof RemoteRequestRefused)) throw error;
      check(); relayPreferencesAvailable = false;
    }
    check();
    return {
      relayPreferencesAvailable,
      async getPublicKey() { check(); return owner; },
      async signEvent(input: unknown): Promise<SignedEvent> {
        check(); if (signing) throw Error('A signer approval is already pending.'); signing = true;
        const operationDeadline = performance.now() + timeoutMs;
        const phase = () => { check(); if (performance.now() >= operationDeadline) { close('Signer operation timed out. Late results will not be used.'); throw Error('Signer operation timed out. Reconnect explicitly.'); } };
        let deadline: ReturnType<typeof setTimeout> | undefined;
        const operation = async () => { const finish = await options.beforeSign?.(); phase(); if (finish !== undefined && typeof finish !== 'function') { close(); throw Error('The account confirmation callback is invalid.'); }
        const e = structuredClone(input) as EventTemplate;
        if (!e || typeof e !== 'object' || Array.isArray(e) || Object.keys(e).some(k => !['pubkey', 'created_at', 'kind', 'content', 'tags'].includes(k)) || e.pubkey !== undefined && e.pubkey !== owner || !REMOTE_SIGNING_KINDS.includes(e.kind) || !Number.isSafeInteger(e.created_at) || e.created_at < 0 || typeof e.content !== 'string' || !Array.isArray(e.tags) || e.tags.some(tag => !Array.isArray(tag) || tag.some(value => typeof value !== 'string')) || encoder.encode(JSON.stringify(e)).length > 16000) throw Error('This exact event is outside the reviewed signer permissions.');
        const template = { kind: e.kind, content: e.content, tags: e.tags, created_at: e.created_at };
        phase(); const signed = JSON.parse(await rpc('sign_event', [JSON.stringify(template)])); phase();
        if (!verifyEvent(signed) || signed.pubkey !== owner || signed.kind !== e.kind || signed.created_at !== e.created_at || signed.content !== e.content || JSON.stringify(signed.tags) !== JSON.stringify(e.tags)) { close(); throw Error('The signer changed the requested event or returned an invalid signature.'); }
        try { await finish?.(); } catch (error) { close(); throw error; } phase(); return signed;
        };
        const overdue = new Promise<never>((_, reject) => { deadline = setTimeout(() => { close('Signer operation timed out. Late results will not be used.'); reject(Error('Signer operation timed out. Reconnect explicitly.')); }, timeoutMs); });
        try { return await Promise.race([operation(), overdue]); } finally { clearTimeout(deadline); signing = false; }
      },
      onClose(listener: () => void) { if (closed) listener(); else closedListeners.add(listener); return () => closedListeners.delete(listener); },
      clear: close,
    };
  } catch (error) { close(); throw error; }
}
