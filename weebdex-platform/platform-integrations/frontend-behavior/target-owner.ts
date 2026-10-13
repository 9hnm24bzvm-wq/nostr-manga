// One immutable target/browser authority for an editor's entire keyed lifetime.
import HTTP, { HTTPError, type HTTPResponse } from '$lib/HTTP';
import Session from '$lib/stores/Session';
import { privateFeedIdentity } from './private-feed';
import { PUBLIC_API_URL } from '$env/static/public';
import { get } from 'svelte/store';
import { onDestroy } from 'svelte';
export function targetOwner(target: () => string, actor: string) {
  const id = target(), authority = get(privateFeedIdentity); let disposed = false;
  onDestroy(() => { disposed = true; });
  const assert = () => { if (disposed || target() !== id || get(Session)?.identity.id !== actor || get(privateFeedIdentity) !== authority) throw new HTTPError(409, 'The editor target or browser authority changed. Any request already sent remains bound to its original target; reload its current state before retrying. No automatic replay occurred.', 'Previous editor context changed'); };
  async function request<T>(route: string, method: 'GET' | 'POST' | 'PUT' | 'DELETE', payload?: unknown): Promise<HTTPResponse<T> | undefined> {
    assert(); if (!route.startsWith('/') || route.startsWith('//') || route.includes('://')) throw Error('Invalid owned route');
    const http = new HTTP<T>(PUBLIC_API_URL.replace(/\/$/, '') + route, payload);
    http.setWithCredentials(true); http.setRequestHeader('X-WeebDex-Expected-Account', actor);
    const result = await http[method](); assert(); return result;
  }
  return { id, actor, assert, request, alive: () => { try { assert(); return true; } catch { return false; } } };
}
