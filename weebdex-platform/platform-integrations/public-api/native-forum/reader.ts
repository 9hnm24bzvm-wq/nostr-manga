// Direct forum URLs retain persisted provenance. The byte/abort owner is shared
// with bounded catalogue selectors, not copied into a second transport.
import { record, PublicApiContractError } from '../validation';
import { nativeThreadId, nativeDiscussionTarget, type NativeDiscussionTarget } from './context';
import { createOwnedJsonReader, type OwnedJsonDependencies } from '../owned-json';
export interface NativeDiscussionContextDependencies extends OwnedJsonDependencies {}
export const NATIVE_DISCUSSION_CONTEXT_LIMITS = Object.freeze({ bytes: 16 * 1024, fragments: 4096, emptyFragments: 128, deadlineMs: 10000, cleanupMs: 250 });
export class NativeDiscussionContextUnsettled extends PublicApiContractError {
  readonly unsettled = true;
  constructor(status = 499) { super('Discussion target lookup cleanup is unconfirmed. New lookups are stopped until pending fetch, read and cancellation work settles. Browser login remains separate.', status); }
}
export function createNativeDiscussionTargetReader(injected: Partial<NativeDiscussionContextDependencies> = {}) {
  const read = createOwnedJsonReader(NATIVE_DISCUSSION_CONTEXT_LIMITS, injected, { unsettled: status => new NativeDiscussionContextUnsettled(status), failure: (message, status) => new PublicApiContractError(message, status) }, 404);
  return async (threadInput: string, signal?: AbortSignal, assertCurrent = () => {}): Promise<NativeDiscussionTarget | null> => {
    const threadId = nativeThreadId(threadInput);
    const response = await read('/api/v1/nostr/native-forum/threads/' + encodeURIComponent(threadId) + '/context', { credentials: 'omit', signal, assertCurrent });
    assertCurrent(); if (signal?.aborted) throw new PublicApiContractError('Discussion target lookup cancelled', 499);
    if (response.status === 404) return null;
    if (response.status !== 200) throw new PublicApiContractError('Signed discussion target lookup unavailable (HTTP ' + response.status + '). Browser login remains separate.', response.status);
    const body = record(response.body, 'native discussion context'), meta = record(body.nostr), data = record(body.data);
    if (body.result !== 'ok' || meta.source !== 'persisted-native-discussion-target') throw new PublicApiContractError('Discussion target lacks persisted native provenance');
    const target = nativeDiscussionTarget({ kind: data.kind, targetId: data.id, threadId: data.threadId }, threadId);
    assertCurrent(); return target;
  };
}
export const readNativeDiscussionTarget = createNativeDiscussionTargetReader();
