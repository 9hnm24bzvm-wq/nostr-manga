// Follow status belongs to one current Session revision and route target.
import { onDestroy } from 'svelte';
import { writable } from 'svelte/store';
import Session from '$lib/stores/Session';
import type Mutex from '$lib/Mutex';
import type { HTTPResponse } from '$lib/HTTP';
import * as Errors from '$lib/constants/Error';
import type { createMutationOwner, MutationIntent } from './authenticated-mutation';

interface FollowState {
  followed: boolean;
  ready: boolean;
  loading: boolean;
  updating: boolean;
  error: boolean;
}
interface FollowOptions {
  owner: ReturnType<typeof createMutationOwner>;
  mutex: Mutex;
  target(): string;
  lifetime(): string;
  canMutate(): boolean;
  read(target: string): Promise<HTTPResponse<unknown> | undefined>;
  write(target: string, followed: boolean, intent: MutationIntent): Promise<HTTPResponse<unknown> | undefined>;
  onError(error: unknown): void;
}
const empty = (): FollowState => ({ followed: false, ready: false, loading: false, updating: false, error: false });

export function createActorFollowState(options: FollowOptions) {
  let state = empty(), actor: string | undefined, selected: string | undefined;
  let generation = 0, disposed = false;
  let pendingWrite: { actor: string; target: string } | undefined;
  const store = writable(state);
  const publish = (next: FollowState) => { state = next; store.set(next); };
  const key = () => JSON.stringify([options.target(), options.lifetime()]);
  const current = (revision: number, targetKey: string) => !disposed && generation === revision && selected === targetKey && key() === targetKey;
  const reset = () => { generation++; publish(empty()); };

  const refresh = async () => {
    if (disposed || !actor || selected === undefined || selected !== key() || state.updating) return;
    // A read ahead of an unsettled write cannot establish actionable state for
    // that same actor/target, even after a Session refresh or route round trip.
    if (pendingWrite?.actor === actor && pendingWrite.target === options.target()) { publish({ ...empty(), loading: true }); return; }
    if (state.loading) return;
    const revision = ++generation, targetKey = selected, target = options.target();
    let intent: MutationIntent | undefined;
    publish({ ...empty(), loading: true });
    try {
      intent = options.owner.capture();
      intent.assert();
      const response = await options.read(target);
      if (!current(revision, targetKey) || !intent.alive()) return;
      // A failed, forbidden or missing response must never become "not followed".
      if (response?.status !== 204 && response?.status !== 404) throw Errors.Unknown;
      publish({ ...empty(), ready: true, followed: response.status === 204 });
    } catch (error) {
      if (!current(revision, targetKey) || (intent && !intent.alive())) return;
      publish({ ...empty(), error: true });
      options.onError(error);
    } finally { intent?.release(); }
  };

  const selectTarget = () => {
    if (disposed) return;
    const next = key();
    if (selected === next) return;
    options.owner.invalidate();
    selected = next;
    reset();
    void refresh();
  };
  const unsubscribe = Session.subscribe(session => {
    actor = session?.identity?.id;
    reset();
    // The initial target is selected by the component's pre-effect. Later
    // Session completions reset synchronously, before controls can be used.
    if (selected !== undefined) void refresh();
  });

  const activate = () => {
    selectTarget();
    if (disposed || !actor || !options.canMutate() || options.mutex.isLocked() || state.loading || state.updating) return;
    if (!state.ready) return refresh(); // Explicit Retry only reads current status.
    const revision = generation, targetKey = selected!, target = options.target(), followed = !state.followed;
    const intent = options.owner.capture(), mutation = { actor, target };
    let acknowledged = false;
    return options.mutex.exec(async () => {
      pendingWrite = mutation;
      publish({ ...state, updating: true });
      try {
        intent.assert();
        if (!current(revision, targetKey)) return;
        const response = await options.write(target, followed, intent);
        acknowledged = response?.status === 204;
        if (!current(revision, targetKey) || !intent.alive()) return;
        if (response?.status !== 204) throw response?.status === 429 ? Errors.TooManyRequests : response?.status === 409 ? Errors.UpdateConflict : Errors.Unknown;
        publish({ ...empty(), ready: true, followed });
      } catch (error) {
        if (!current(revision, targetKey) || !intent.alive()) return;
        // An interrupted write may have succeeded. Reconcile by read, never replay.
        publish({ ...empty(), error: true });
        options.onError(error);
      } finally {
        if (current(revision, targetKey) && state.updating) publish({ ...state, updating: false });
        intent.release();
        if (pendingWrite === mutation) pendingWrite = undefined;
        if (!disposed && generation !== revision && actor === mutation.actor && options.target() === target && selected === key()) {
          reset();
          if (acknowledged) void refresh();
          else {
            // Cancellation is not proof that the server rolled back. Leave a
            // visible unknown state for explicit read-only retry; no replay or
            // strict server-side ordering is inferred from transport settlement.
            publish({ ...empty(), error: true });
          }
        }
      }
    });
  };
  const knownAbsent = () => !!actor && selected !== undefined && current(generation, selected) && state.ready && !state.followed && options.canMutate();
  onDestroy(() => { disposed = true; generation++; unsubscribe(); });
  return { subscribe: store.subscribe, selectTarget, refresh, activate, knownAbsent };
}
