// One current query owns its debounce, cancellation, completion and busy state.
import { onDestroy } from 'svelte';
import { HTTPError } from '$lib/HTTP';
import { PUBLIC_API_URL } from '$env/static/public';
import { readThreadContext } from './thread-read';
export function createLatestSearch(setBusy: (busy: boolean) => void) {
  let generation = 0, timer = 0, disposed = false, controller: AbortController | undefined;
  const invalidate = () => { generation++; window.clearTimeout(timer); controller?.abort(); controller = undefined; setBusy(false); };
  onDestroy(() => { disposed = true; invalidate(); });
  return { invalidate, schedule<T>(delay: number, operation: (signal: AbortSignal) => Promise<T>, attach: (value: T) => void, fail: (error: unknown) => void) {
    invalidate(); if (disposed) return;
    const current = generation; controller = new AbortController(); const owned = controller;
    const alive = () => !disposed && generation === current && !owned.signal.aborted;
    timer = window.setTimeout(() => {
      if (!alive()) return; setBusy(true);
      void operation(owned.signal).then(value => { if (alive()) attach(value); }).catch(error => { if (alive()) fail(error); }).finally(() => { if (alive()) { controller = undefined; setBusy(false); } });
    }, delay);
  } };
}
export async function readPublicSearch<T>(path: string, query: URLSearchParams, signal: AbortSignal, personal = false) {
  const encoded = [...query.entries()].map(([key, value]) => encodeURIComponent(key) + '=' + encodeURIComponent(value)).join('&');
  const route = path + (encoded ? '?' + encoded : '');
  if (personal) return readThreadContext<T>(route, signal);
  const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + route, { credentials: 'omit', cache: 'no-store', redirect: 'error', signal: AbortSignal.any([signal, AbortSignal.timeout(30000)]) });
  const body = /json/i.test(response.headers.get('content-type') ?? '') ? await response.json() as T : undefined;
  if (signal.aborted) throw new HTTPError(409, 'An obsolete search result was discarded');
  return { status: response.status, body };
}
