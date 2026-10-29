// Ordinary site actions use exactly one browser account. This transport never
// imports a signer, accepts a key, or falls back to another authentication mode.
import { publicUuid, record, PublicApiContractError } from './validation';
export type NativeAccountIssue = 'signin' | 'verify' | 'permission' | 'changed' | 'unavailable';
export class NativeAccountError extends PublicApiContractError {
  constructor(readonly issue: NativeAccountIssue, message: string, status: number) { super(message, status); this.name = 'NativeAccountError'; }
}
export class NativeAccountRequestError extends Error {
  constructor(message: string, readonly outcome: 'unsent' | 'uncertain') { super(message); this.name = 'NativeAccountRequestError'; }
}
export function nativeAccountIssue(cause: unknown): NativeAccountIssue | undefined { return cause instanceof NativeAccountError ? cause.issue : undefined; }
export function nativeAccountError(status: number, body?: unknown): NativeAccountError {
  const envelope = body && typeof body === 'object' && !Array.isArray(body) ? body as Record<string, unknown> : {};
  const first = Array.isArray(envelope.errors) && envelope.errors.length === 1 ? envelope.errors[0] as Record<string, unknown> : undefined;
  const detail = first?.status === status ? first.detail : envelope.error;
  if (status === 401) return new NativeAccountError('signin', 'Sign in to your account to continue.', status);
  if (status === 403 && detail === 'Verify your email before performing this action') return new NativeAccountError('verify', 'Verify your email to finish setting up your account, then return here.', status);
  if (status === 403) return new NativeAccountError('permission', 'Your account is not allowed to do this. Contact the site team if you think this is a mistake.', status);
  if (status === 409) return new NativeAccountError('changed', 'Your account or this item changed. Reload it before trying again.', status);
  return new NativeAccountError('unavailable', 'The site could not complete this request. Please try again in a moment.', status);
}
export interface NativeAccountDependencies {
  fetch: typeof fetch;
  origin(): string;
  fences: { get(key: string): string | null; set(key: string, value: string): void; delete(key: string): void };
}
const defaults: NativeAccountDependencies = { fetch: (...args) => fetch(...args), origin: () => window.location.origin, fences: { get: key => sessionStorage.getItem(key), set: (key, value) => sessionStorage.setItem(key, value), delete: key => sessionStorage.removeItem(key) } };
export async function createNativeAccountScope(nativeAccount: string, assertCurrent: () => void, injected: Partial<NativeAccountDependencies> = {}, options: { requireVerified?: boolean } = {}) {
  const deps = { ...defaults, ...injected }, origin = new URL(deps.origin()).origin;
  const abort = new AbortController(); let disposed = false;
  const dispose = () => { disposed = true; abort.abort(); };
  function check() { assertCurrent(); if (disposed || abort.signal.aborted || new URL(deps.origin()).origin !== origin) throw new NativeAccountError('changed', 'This account view changed or closed. Open it again to continue.', 409); }
  if (!nativeAccount || !/^[A-Za-z0-9_:-]{1,128}$/.test(nativeAccount)) throw nativeAccountError(401);
  async function send(path: string, method: 'GET' | 'POST' | 'PUT' | 'DELETE' = 'GET', body?: unknown) {
    let dispatched = false, received: Response | undefined;
    try {
      check();
      if (!path.startsWith('/') || path.startsWith('//') || /[#\\\u0000-\u0020]/.test(path)) throw Error('Invalid account path');
      const destination = origin + '/api/v1/account' + path, parsed = new URL(destination);
      if (parsed.href !== destination || parsed.origin !== origin || !parsed.pathname.startsWith('/api/v1/account/')) throw Error('Account request escaped its scope');
      const bytes = body === undefined ? undefined : new TextEncoder().encode(JSON.stringify(body));
      if (method === 'GET' && bytes || (bytes?.byteLength ?? 0) > 32768) throw Error('Invalid account request body');
      const signal = AbortSignal.any([abort.signal, AbortSignal.timeout(30000)]);
      const headers: Record<string, string> = { Accept: 'application/json', 'X-Weebdex-Expected-Account': nativeAccount };
      if (bytes) headers['Content-Type'] = 'application/json';
      check(); dispatched = true;
      const response = received = await deps.fetch(destination, { method, headers, body: bytes, credentials: 'same-origin', redirect: 'error', cache: 'no-store', signal });
      check();
      if (response.headers.get('content-type')?.split(';')[0].trim().toLowerCase() !== 'application/json') throw Error('Invalid account response');
      const reader = response.body?.getReader(); if (!reader) throw Error('Missing account response');
      const chunks: Uint8Array[] = []; let length = 0;
      try { for (;;) { const row = await reader.read(); check(); if (row.done) break; length += row.value.byteLength; if (length > 1024 * 1024) throw Error('Account response exceeded budget'); chunks.push(row.value); } }
      catch (error) { await reader.cancel().catch(() => {}); throw error; } finally { reader.releaseLock(); }
      const data = new Uint8Array(length); let offset = 0; for (const row of chunks) { data.set(row, offset); offset += row.byteLength; }
      const result = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(data)); check();
      return { status: response.status, body: result, headers: Object.fromEntries(response.headers.entries()) };
    } catch (error) {
      try { await received?.body?.cancel(); } catch { /* The reader may already have closed the body. */ }
      if (error instanceof NativeAccountError) throw error;
      throw new NativeAccountRequestError(dispatched && method !== 'GET' ? 'The result is not confirmed. Reload before trying again; nothing will be sent twice automatically.' : 'Your account could not be checked. Please try again.', dispatched ? 'uncertain' : 'unsent');
    }
  }
  try {
    const response = await send('/me'); check();
    if (response.status !== 200) throw nativeAccountError(response.status, response.body);
    const body = record(response.body), row = record(body.data), attributes = record(row.attributes), authority = record(body.nostr);
    if (body.result !== 'ok' || body.response !== 'entity' || row.type !== 'user' || authority.scope !== 'instance-account' || authority.identity !== 'native-session-account' || authority.authentication !== 'native-session' || authority.nativeSessionAuthority !== true || typeof attributes.username !== 'string' || !attributes.username || typeof attributes.isVerified !== 'boolean' || !Array.isArray(attributes.permissions) || attributes.permissions.some(value => typeof value !== 'string')) throw Error('Invalid current account');
    const account = Object.freeze({ id: publicUuid(row.id), name: attributes.username, verified: attributes.isVerified, permissions: Object.freeze([...attributes.permissions]) as readonly string[] });
    if (options.requireVerified !== false && !account.verified) throw new NativeAccountError('verify', 'Verify your email to finish setting up your account, then return here.', 403);
    return { account, check, send, dispose, fences: deps.fences, key: (operation: string) => `weebdex.account-${operation}.v1:${origin}:${nativeAccount}:${account.id}` };
  } catch (error) { dispose(); throw error; }
}
