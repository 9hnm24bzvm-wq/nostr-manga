// Native browser logout only. No signer, local-only success, auth fallback or
// automatic mutation replay. Private flow tokens never enter a URL or storage.
export type LogoutResult = { state: 'signed-out' | 'not-confirmed'; message: string };
export type LogoutDependencies = {
  fetch: typeof fetch;
  origin: string;
  authBase: string;
  apiBase: string;
  account: string | null;
  assertCurrent(): void;
  signal?: AbortSignal;
  timeoutMs?: number;
};
const signedOut = (): LogoutResult => ({ state: 'signed-out', message: 'You are signed out in this browser.' });
const unconfirmed = (): LogoutResult => ({ state: 'not-confirmed', message: 'Sign out could not be confirmed. You may still be signed in. Check your status or try signing out again.' });
function transport(deps: LogoutDependencies) {
  const origin = new URL(deps.origin).origin;
  const endpoint = (base: string, suffix: string) => {
    const root = new URL(base, origin);
    if (root.origin !== origin || root.username || root.password || root.search || root.hash) throw Error('Native logout must stay on this site.');
    return root.href.replace(/\/$/, '') + suffix;
  };
  const auth = (suffix: string) => endpoint(deps.authBase || '/auth', suffix);
  const check = endpoint(deps.apiBase || '/api', '/auth/check');
  const timeout = Math.min(30000, Math.max(1, deps.timeoutMs ?? 20000));
  const signal = AbortSignal.any([AbortSignal.timeout(timeout), ...(deps.signal ? [deps.signal] : [])]);
  const deadline = performance.now() + timeout;
  const current = () => { deps.assertCurrent(); if (signal.aborted || performance.now() >= deadline) throw Error('Logout request expired.'); };
  async function request(url: string, method: 'GET' | 'POST' = 'GET', body?: unknown, account = deps.account) {
    current();
    const headers: Record<string, string> = { Accept: 'application/json' };
    if (account) headers['X-Weebdex-Expected-Account'] = account;
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    const response = await deps.fetch(url, { method, headers, ...(body === undefined ? {} : { body: JSON.stringify(body) }), credentials: 'same-origin', redirect: 'error', referrerPolicy: 'no-referrer', cache: 'no-store', signal });
    try { current(); } catch (error) { await response.body?.cancel().catch(() => {}); throw error; }
    return response;
  }
  async function object(response: Response) {
    if (response.status !== 200 || response.headers.get('content-type')?.split(';')[0].trim().toLowerCase() !== 'application/json') { await response.body?.cancel().catch(() => {}); throw Error('Native logout response unavailable.'); }
    const reader = response.body?.getReader(); if (!reader) throw Error('Missing logout response.');
    const parts: Uint8Array[] = []; let size = 0;
    try { for (;;) { const item = await reader.read(); current(); if (item.done) break; size += item.value.byteLength; if (size > 16384) throw Error('Logout response exceeded its budget.'); parts.push(item.value); } }
    catch (error) { await reader.cancel().catch(() => {}); throw error; } finally { reader.releaseLock(); }
    const bytes = new Uint8Array(size); let offset = 0; for (const part of parts) { bytes.set(part, offset); offset += part.length; }
    const value: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid logout response.');
    return value as Record<string, unknown>;
  }
  async function absent() { const response = await request(check); const result = response.status === 401; await response.body?.cancel().catch(() => {}); current(); return result; }
  return { request, object, absent, auth };
}
export async function checkNativeLogout(deps: LogoutDependencies): Promise<LogoutResult> {
  try { const io = transport(deps); return await io.absent() ? signedOut() : unconfirmed(); }
  catch { deps.assertCurrent(); return unconfirmed(); }
}
export async function performNativeLogout(deps: LogoutDependencies): Promise<LogoutResult> {
  try {
    const io = transport(deps), currentResponse = await io.request(io.auth('/sessions/whoami'));
    if (currentResponse.status === 401) { await currentResponse.body?.cancel().catch(() => {}); return await io.absent() ? signedOut() : unconfirmed(); }
    const current = await io.object(currentResponse), identity = current.identity;
    const session = current.id, account = identity && typeof identity === 'object' && !Array.isArray(identity) ? (identity as Record<string, unknown>).id : null;
    if (current.active !== true || typeof session !== 'string' || !session || session.length > 128 || typeof account !== 'string' || !account || account.length > 128 || deps.account && account !== deps.account) throw Error('The native session changed.');
    const flow = await io.object(await io.request(io.auth('/self-service/logout/browser'), 'GET', undefined, account));
    if (flow.account_id !== account || flow.session_id !== session || typeof flow.logout_token !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(flow.logout_token)) throw Error('The logout flow is not bound to the captured session.');
    deps.assertCurrent();
    // A browser POST carries Origin even under no-referrer. The server still
    // validates Origin, browser cookie, flow token and exact captured session.
    try { const response = await io.request(io.auth('/self-service/logout'), 'POST', { token: flow.logout_token, expected_session_id: session }, account); await response.body?.cancel().catch(() => {}); }
    catch { deps.assertCurrent(); /* An uncertain dispatch is never replayed. */ }
    return await io.absent() ? signedOut() : unconfirmed();
  } catch { deps.assertCurrent(); return unconfirmed(); }
}
