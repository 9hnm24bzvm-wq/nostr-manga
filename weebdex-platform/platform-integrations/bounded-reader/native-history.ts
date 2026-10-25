import { readerLibrarySource, assertLibraryAdmission } from '../local-library/library';
import { createNativeCompletionJsonReader, type NativeCompletionDependencies, type NativeCompletionInput } from '../public-api/owned-json';
import { ReaderIdentityChanged } from './contract';
import { nativeCompletionRows, type CompletionAccess } from './completion-contract';
import { historySourceKey } from './history';
import type { ReaderBudget, ReaderEndpoint, ReaderIdentity } from './client';
export function createNativeHistoryAccess(endpoint: () => ReaderEndpoint, injected: Partial<NativeCompletionDependencies> = {}) {
  const read = createNativeCompletionJsonReader(endpoint, injected);
  return (mangaId: string, identity: ReaderIdentity | undefined): CompletionAccess => {
    const source = readerLibrarySource(), approved = Object.freeze({ ...endpoint() }), sourceKey = historySourceKey(source), actor = source.admission.authority.actor;
    identity = identity && Object.freeze({ ...identity });
    const key = JSON.stringify([sourceKey, approved, identity]);
    const status = source.selectionPending ? 'pending' : approved.profile !== 'native-aggregate-v1' || source.mode !== 'account' || !actor || source.context.accountId !== actor ? 'unavailable' : !identity || identity.origin !== approved.origin ? 'pending' : 'available-native';
    const current = () => {
      assertLibraryAdmission(source.admission);
      const now = endpoint();
      if (historySourceKey(readerLibrarySource()) !== sourceKey || now.origin !== approved.origin || now.base !== approved.base || now.profile !== approved.profile) throw Error('The native history destination or selected library changed. Restart the scan.');
    };
    const projection = async (ids: readonly string[], budget: ReaderBudget) => {
        budget.check(); current();
        if (status !== 'available-native' || !identity || !actor) throw Error('Native account history is unavailable. No device or empty-history fallback was used.');
        if (ids.length > 50 || new Set(ids).size !== ids.length) throw Error('Native history key-read budget exceeded');
        if (!ids.length) return Object.freeze([]);
        const request: NativeCompletionInput = Object.freeze({ mangaId, ids: Object.freeze([...ids]), accountId: actor, accountEpoch: source.admission.authority.epoch, catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey });
        const check = () => { budget.check(); current(); };
        budget.request();
        const response = await read(request, { signal: budget.signal, assertCurrent: check, consume: budget.consume });
        check();
        if (response.status === 401 || response.status === 403) throw Error('The native cookie account is not authorized. Sign in again before scanning; no fallback was used.');
        if (response.status === 409) throw new ReaderIdentityChanged('Native account, catalogue or requested chapter admission changed. Reload before scanning; no incomplete coverage was accepted.');
        if (response.status !== 200) throw Error('Native completion unavailable (HTTP ' + response.status + '). No empty-history assumption was made.');
        const observed = nativeCompletionRows(response.body, request); check(); budget.accept(ids.length); return observed;
    };
    return { key, status, reason: source.selectionPending ? 'A library selection is pending. Wait before scanning either history source.' : approved.profile !== 'native-aggregate-v1' ? 'This public catalogue has no native-account completion source. Select device history explicitly in Library.' : !actor ? 'Sign into the native cookie account before scanning account history.' : 'Verify the current catalogue before scanning native-account history.',
      projection,
      async completed(ids, budget) { const rows = await projection(ids, budget); budget.check(); current(); return rows.some(row => row.completed); }
    };
  };
}
