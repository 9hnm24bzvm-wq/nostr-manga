// Private report/draft DTOs are never restored from navigation history.
import API from '$lib/API';
import HTTP, { HTTPError, type HTTPResponse } from '$lib/HTTP';
import Session from '$lib/stores/Session';
import { PUBLIC_API_URL } from '$env/static/public';
import { derived, get } from 'svelte/store';
export const privateFeedIdentity = derived(Session, account => account ? JSON.stringify([account.identity.id, account.roles ?? [], account.permissions ?? [], account.identity.is_verified ?? false]) : 'anonymous');
let generation = 0, previous = '';
privateFeedIdentity.subscribe(key => { if (key !== previous) { previous = key; generation++; } });
export async function readPrivateFeed<T>(path: '/report' | '/manga/draft', query: URLSearchParams): Promise<HTTPResponse<T>> {
  const actor = get(Session)?.identity.id, selected = get(privateFeedIdentity), captured = generation;
  const assert = () => { if (!actor || get(Session)?.identity.id !== actor || get(privateFeedIdentity) !== selected || captured !== generation) throw new HTTPError(409, 'The account/session authority changed. Old private results were discarded; reload the current account.', 'Private feed context changed'); };
  const check = async () => { assert(); const result = await API.Auth.Check(); assert(); if (result?.status !== 200 || result.body?.identity?.id !== actor) throw new HTTPError(401, 'A fresh matching browser account was not confirmed. No old private feed was restored.', 'Private feed unavailable'); };
  await check();
  const encoded = [...query.entries()].map(([key, value]) => encodeURIComponent(key) + '=' + encodeURIComponent(value)).join('&');
  const request = new HTTP<T>(PUBLIC_API_URL.replace(/\/$/, '') + path + (encoded ? '?' + encoded : ''));
  request.setWithCredentials(true); request.setRequestHeader('X-WeebDex-Expected-Account', actor!);
  const result = await request.GET(); assert(); await check(); assert();
  if (!result) throw new HTTPError(502, 'No current private response was received');
  return result;
}
