import { writable } from 'svelte/store';
import { PUBLIC_API_URL } from '$env/static/public';
import { HTTPError, type HTTPResponse } from '$lib/HTTP';
import type { Constants } from '$lib';
import { createSafeReadOwner } from './safe-read-owner';
import { requireReadAcknowledgement, readFailure } from './mutation-feedback';
type WidgetState = { feed?: MangaFeedResponse; isMounted: boolean; isLoading: boolean; error?: Constants.Error.Error };
// Public discovery owns its effective query, not password/library readiness.
// No persistent or module-scope DTO cache survives preference changes or remount.
export function createPublicMangaWidgetRead(query: () => URLSearchParams,
  request: (query: URLSearchParams) => Promise<HTTPResponse<MangaFeedResponse> | undefined>) {
  const state = writable<WidgetState>({ isMounted: false, isLoading: false });
  const reads = createSafeReadOwner();
  let observed = '', pending = false, draining = false;
  const signature = (q = query()) => JSON.stringify([
    new URL(PUBLIC_API_URL || '/api', window.location.origin + '/').href,
    window.location.href, [...q.entries()]
  ]);
  function queue() {
    if (!reads.isLive()) return;
    reads.invalidate(); pending = true;
    // The caller invokes this in $effect.pre: old explicit cards disappear
    // before the updated preference selection can render beside them.
    state.set({ isMounted: false, isLoading: draining });
    void Promise.resolve().then(drain);
  }
  async function drain() {
    if (draining || !reads.isLive()) return;
    draining = true; state.update(value => ({ ...value, isLoading: true }));
    try {
      while (pending && reads.isLive()) {
        pending = false;
        const q = query(), key = signature(q), intent = reads.capture(() => signature() === key);
        try {
          const response = await request(q);
          if (!intent.alive()) {
            if (intent.current()) state.update(value => ({ ...value, error: readFailure(new HTTPError(409, 'Navigation or filters interrupted this title feed. Retry the current view.')) }));
            continue;
          }
          const feed = requireReadAcknowledgement(response);
          if (!Array.isArray(feed.data)) throw new HTTPError(502, 'The server returned an invalid title feed. No empty result was inferred.');
          state.set({ feed, isMounted: true, isLoading: true });
        } catch (error) {
          if (intent.current()) state.update(value => ({ ...value, error: readFailure(error) }));
        } finally {
          if (intent.current()) state.update(value => ({ ...value, isMounted: true }));
        }
      }
    } finally {
      draining = false;
      if (reads.isLive()) state.update(value => ({ ...value, isLoading: false }));
    }
  }
  return { state, retry: queue, synchronize() {
    if (!reads.isLive()) return;
    const key = signature();
    if (key === observed) return;
    observed = key; queue();
  } };
}
