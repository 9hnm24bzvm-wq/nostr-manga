import { readerLibrarySource, assertLibraryAdmission } from '../local-library/library';
import { createNativeContinueJsonReader, NATIVE_CONTINUE_LIMITS, type NativeCompletionDependencies, type NativeContinueInput } from '../public-api/owned-json';
import { ReaderIdentityChanged } from './contract';
import { historySourceKey } from './history';
import { nativeContinueScope, nativeContinueResult, type NativeContinueResult } from './native-continue-contract';
import type { ReaderBudget, ReaderEndpoint, ReaderIdentity } from './client';
export type NativeContinueAccess = Readonly<{ key: string; available: boolean; reason: string; read(filters: string, budget: ReaderBudget): Promise<NativeContinueResult> }>;
export function createNativeContinueAccess(endpoint: () => ReaderEndpoint, injected: Partial<NativeCompletionDependencies> = {}) {
  const read = createNativeContinueJsonReader(endpoint, injected);
  return (mangaId: string, identity: ReaderIdentity | undefined): NativeContinueAccess => {
    const source = readerLibrarySource(), approved = Object.freeze({ ...endpoint() }), sourceKey = historySourceKey(source), actor = source.admission.authority.actor;
    identity = identity && Object.freeze({ ...identity });
    const key = JSON.stringify([sourceKey, approved, identity]);
    const available = !source.selectionPending && source.mode === 'account' && approved.profile === 'native-aggregate-v1' && !!actor && source.context.accountId === actor && !!identity && identity.origin === approved.origin;
    const reason = source.selectionPending ? 'A library selection is pending.' : approved.profile !== 'native-aggregate-v1' ? 'Native-account Continue is unavailable in this public catalogue.' : 'Verify this catalogue and sign into the selected native cookie account before continuing.';
    const current = () => {
      assertLibraryAdmission(source.admission); const now = endpoint();
      if (historySourceKey(readerLibrarySource()) !== sourceKey || now.origin !== approved.origin || now.base !== approved.base || now.profile !== approved.profile) throw Error('The native Continue destination or selected library changed. Retry the action.');
    };
    return { key, available, reason, async read(filters, budget) {
      budget.check(); current(); if (!available || !identity || !actor) throw Error(reason);
      if (!budget.fits(1, 1, NATIVE_CONTINUE_LIMITS.bytes)) throw Error('Native Continue action budget exceeded');
      const input: NativeContinueInput = Object.freeze({ mangaId, accountId: actor, accountEpoch: source.admission.authority.epoch,
        catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey, requestId: 'reader_' + crypto.randomUUID(), filters });
      const check = () => { budget.check(); current(); };
      // Hash input is bounded. Cancellation leaves the shared digest slot owned
      // until actual WebCrypto settlement; it cannot accumulate remounted work.
      const scope = await nativeContinueScope(input, check, budget.signal); check(); budget.request();
      const response = await read(input, { signal: budget.signal, assertCurrent: check, consume: budget.consume }); check();
      if (response.status === 401 || response.status === 403) throw Error('The native cookie account is not authorized. Sign in again; no device fallback was used.');
      if (response.status === 409) throw new ReaderIdentityChanged('Native account or catalogue admission changed or is busy. Reload before continuing; no replacement target was selected.');
      if (response.status !== 200) throw Error('Native Continue unavailable (HTTP ' + response.status + '). No empty history or all-read result was assumed.');
      const result = nativeContinueResult(response.body, input, scope); check(); budget.accept(Number(!!result.winner)); return result;
    } };
  };
}
