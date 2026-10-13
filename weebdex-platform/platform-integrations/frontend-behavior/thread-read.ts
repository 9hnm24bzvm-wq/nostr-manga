// Discussion/announcement personal projections are current-authority reads, never history DTOs.
import { HTTPError } from '$lib/HTTP';
import { PUBLIC_API_URL } from '$env/static/public';
import { captureNativeAuthority, sameNativeReadAuthority, nativeAuthority } from './mutation-authority';
export async function readThreadContext<T>(path: string, signal?: AbortSignal) {
  const authority = captureNativeAuthority(), controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), 30000);
  const cancel = () => controller.abort();
  signal?.addEventListener('abort', cancel, { once: true });
  if (signal?.aborted) cancel();
  const assert = () => { if (controller.signal.aborted || !sameNativeReadAuthority(authority)) throw new HTTPError(409, 'An old personal projection was discarded after its reader or browser authority changed.', 'Read context changed'); };
  const stop = nativeAuthority.subscribe(() => { if (!sameNativeReadAuthority(authority)) controller.abort(); });
  const read = async (route: string) => {
    assert();
    const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + route, { credentials: authority.actor ? 'include' : 'omit', cache: 'no-store', redirect: 'error', signal: controller.signal, headers: authority.actor ? { Accept: 'application/json', 'X-WeebDex-Expected-Account': authority.actor } : { Accept: 'application/json' } });
    const body = response.status === 204 ? undefined : /json/i.test(response.headers.get('content-type') ?? '') ? await response.json() : undefined;
    assert(); return { status: response.status, body };
  };
  const check = async () => { if (!authority.actor) return; const result = await read('/auth/check'); if (result.status !== 200 || result.body?.identity?.id !== authority.actor) throw new HTTPError(401, 'A fresh matching account could not be confirmed. Old private results were not restored.', 'Read context changed'); };
  try { await check(); const result = await read(path); await check(); assert(); return result as { status: number; body?: T }; }
  finally { window.clearTimeout(timer); stop(); signal?.removeEventListener('abort', cancel); }
}
