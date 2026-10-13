import { HTTPError, type HTTPResponse } from '$lib/HTTP';
import { toast } from 'svelte-sonner';
// Shared truthful failure surface; never turns an error into a replay or success.
export function requireMutationAcknowledgement(response: HTTPResponse<unknown> | undefined, expectedStatus: number, requireBody = false) {
  if (response?.status === expectedStatus && (!requireBody || response.body !== undefined && response.body !== null)) return;
  const status = response?.status ?? 0;
  const detail = status === 401 ? 'Your account session expired. Sign in again and check the current state before submitting another change.'
    : status === 403 ? 'The current account does not have permission for this change. No successful update was acknowledged.'
    : status === 409 ? 'The saved state or authority changed. Refresh and check the current state before trying again.'
    : status === 429 ? 'The request was rate limited. Wait and check the current state before trying again.'
    : status >= 400 && status < 500 ? `The server rejected this change (HTTP ${status}). No successful update was acknowledged.`
    : 'No reliable update acknowledgement was received. The operation may have succeeded. Reload and check its current state before submitting again; it was not replayed.';
  throw new HTTPError(status, detail, status === 0 || status >= 500 || status === expectedStatus ? 'Request outcome unknown' : 'Update failed');
}
export function reportCurrentMutationFailure(title: string, error: unknown, isCurrent: () => boolean) {
  if (!isCurrent()) return;
  const detail = error instanceof HTTPError ? error.body
    : error instanceof Error ? `The update did not complete reliably: ${error.message}. Check the current state before another change.`
    : 'The update did not complete reliably. Check the current state before another change.';
  toast.error(title, { description: detail });
}

// Read failures must never masquerade as a successful empty feed.
export function requireReadAcknowledgement<T>(response: HTTPResponse<T> | undefined): T {
  if (response?.status === 200 && response.body !== undefined) return response.body;
  const status = response?.status ?? 0;
  const detail = status === 401 ? 'Your account session expired. Sign in again, then retry these updates.'
    : status === 403 ? 'These updates are not available to the current account.'
    : status === 409 ? 'The selected account or library changed. Retry the current updates.'
    : status === 429 ? 'The update feed was rate limited. Wait before retrying.'
    : status ? `The update feed could not be loaded (HTTP ${status}). No empty result was inferred. Retry when the service is available.`
    : 'The connection ended before the update feed could be loaded. Retry the current updates.';
  throw new HTTPError(status, detail, 'Updates unavailable');
}
export function readFailure(error: unknown) {
  return error instanceof HTTPError ? error : new HTTPError(0,
    error instanceof Error ? `The update feed could not be loaded: ${error.message}. Retry the current updates.` : 'The update feed could not be loaded. Retry the current updates.', 'Updates unavailable');
}
