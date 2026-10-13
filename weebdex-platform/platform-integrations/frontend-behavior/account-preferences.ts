// Apply a real current-account preference response only inside its admitted authority.
import Session from '$lib/stores/Session';
import { prefs } from '$lib/stores/prefs';
import { get } from 'svelte/store';
import { HTTPError } from '$lib/HTTP';
import { captureNativeAuthority, sameNativeAuthority, type NativeAuthority } from './mutation-authority';
import type { MutationIntent } from './authenticated-mutation';
export function captureAccountPreferences() {
  const authority = captureNativeAuthority(), account = get(Session);
  if (!authority.actor || account?.identity?.id !== authority.actor) throw new HTTPError(401, 'A matching normal browser account is required. No preferences were changed.');
  return { actor: authority.actor, authority, initial: structuredClone(get(prefs)) };
}
export function applyAccountPreferences(expectedActor: string, authority: NativeAuthority, actual: UserPreferences, intent?: MutationIntent) {
  intent?.assert();
  const account = get(Session);
  if (!sameNativeAuthority(authority) || account?.identity?.id !== expectedActor || authority.actor !== expectedActor) throw new HTTPError(409, 'Old preference results were discarded after browser account authority changed. Reload the original account before trying again.', 'Preference context changed');
  if (!actual || !Number.isInteger(actual.version) || actual.version < 1 || !Array.isArray(actual.groups) || !Array.isArray(actual.uploaders)) throw new HTTPError(502, 'No valid current preference response was received. Local preferences were kept.');
  const saved = structuredClone(actual);
  // No await or mutation of an active account's arrays occurs between authority
  // validation and these synchronous stores. An explicit own write completes this intent.
  prefs.set(saved); Session.set({ ...account, preferences: saved });
  return saved;
}
