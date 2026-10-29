import { readerLibrarySource, assertLibraryAdmission } from '../local-library/library';
import { createAccountLibraryJsonReader, NATIVE_CONTINUE_LIMITS, type NativeCompletionDependencies } from '../public-api/owned-json';
import { publicUuid, record } from '../public-api/validation';
import { chapterRow, ReaderIdentityChanged } from './contract';
import { historySourceKey } from './history';
import type { ReaderBudget, ReaderEndpoint, ReaderIdentity } from './client';
import type { NativeContinueAccess } from './native-continue';
import type { NativeContinueResult } from './native-continue-contract';
import type { CompletionAccess } from './completion-contract';
import { accountReaderFilters } from './account-reader-contract';
export function createAccountReaderAccess(endpoint: () => ReaderEndpoint, injected: Partial<NativeCompletionDependencies> = {}) {
  const transport = createAccountLibraryJsonReader(endpoint, injected);
  return (mangaId: string, identity: ReaderIdentity | undefined): { continue: NativeContinueAccess; history: CompletionAccess } => {
    const source = readerLibrarySource(), approved = Object.freeze({ ...endpoint() }), sourceKey = historySourceKey(source), actor = source.admission.authority.actor;
    identity = identity && Object.freeze({ ...identity });
    const key = JSON.stringify([sourceKey, approved, identity]), available = !source.selectionPending && source.mode === 'account' && approved.profile === 'public-retained-v1' && !!actor && source.context.accountId === actor && !!identity && identity.origin === approved.origin;
    const reason = source.selectionPending ? 'Wait for the selected library to finish opening.' : 'Sign in and verify this catalogue before using account reading history.';
    let canonicalAccount: string | undefined;
    const current = () => { assertLibraryAdmission(source.admission); const now = endpoint(); if (historySourceKey(readerLibrarySource()) !== sourceKey || JSON.stringify(now) !== JSON.stringify(approved)) throw Error('The account or reader destination changed. Retry the action.'); };
    async function request(operation: 'me' | 'continue' | 'completion' | 'progress', budget: ReaderBudget, extra: { filters?: string; ids?: readonly string[]; chapterId?: string } = {}) {
      const check = () => { budget.check(); current(); }; check(); if (!available || !actor || !identity) throw Error(reason);
      if (!budget.fits(1, operation === 'completion' ? extra.ids?.length ?? 0 : 1, NATIVE_CONTINUE_LIMITS.bytes)) throw Error('Account reader action budget exceeded');
      budget.request(); const response = await transport({ operation, accountId: actor, ...(operation === 'me' ? {} : { mangaId }), ...extra }, { signal: budget.signal, assertCurrent: check, consume: budget.consume }); check();
      if (response.status === 401 || response.status === 403) throw Error('Sign in again to read your account history. No device history was substituted.');
      if (response.status === 409) throw new ReaderIdentityChanged('Account or catalogue changed or is busy. Reload before continuing.');
      if (response.status !== 200) throw Error('Account reading history is unavailable (HTTP ' + response.status + '). No unread state was assumed.');
      return record(response.body);
    }
    async function admission(budget: ReaderBudget) {
      if (!canonicalAccount) { const body = await request('me', budget), data = record(body.data), attributes = record(data.attributes), authority = record(body.nostr);
        if (body.result !== 'ok' || body.response !== 'entity' || data.type !== 'user' || authority.scope !== 'instance-account' || authority.identity !== 'native-session-account' || authority.authentication !== 'native-session' || authority.nativeSessionAuthority !== true || typeof attributes.username !== 'string' || !attributes.username || typeof attributes.isVerified !== 'boolean') throw Error('The ordinary account could not be verified.');
        canonicalAccount = publicUuid(data.id);
      } current(); budget.check(); return canonicalAccount;
    }
    function envelope(body: Record<string,unknown>, account: string) {
      if (body.result !== 'ok' || body.accountId !== account || body.authentication !== 'native-session' || body.scope !== 'instance-account' || !Number.isSafeInteger(body.revision) || Number(body.revision) < 0) throw new ReaderIdentityChanged('The account history response belongs to a different authority.');
    }
    function catalogue(value: unknown) { const scope = record(value); if (!identity || scope.catalogNamespace !== identity.catalogNamespace || scope.serviceKey !== identity.serviceKey) throw new ReaderIdentityChanged('The catalogue identity changed. Reload; no history fallback was used.'); }
    const continuation: NativeContinueAccess = { key, available, reason, async read(filters,budget): Promise<NativeContinueResult> {
      const account = await admission(budget), body = await request('continue', budget, { filters }); envelope(body,account); const data = record(body.data); catalogue(data.catalogIdentity);
      const effective = accountReaderFilters(filters);
      if (data.mangaId !== mangaId || data.orderingProfile !== 'public-retained-v1' || data.effectiveFilters !== effective || data.continueProof !== 'native-account-snapshot-v1' || data.crossRequestSnapshot !== false || typeof data.hasMultipleEditions !== 'boolean') throw Error('Account Continue witnesses did not match the selected title and filters.');
      const scope = JSON.stringify([key,account,mangaId,effective]);
      if (data.state === 'boundary') { if (data.reason !== 'empty' || data.winner !== null || data.hasMultipleEditions) throw Error('Invalid account Continue boundary'); return Object.freeze({scope,accountRevision:Number(body.revision),state:'boundary',reason:'empty',winner:null,hasMultipleEditions:false}); }
      if (data.state !== 'ready' || data.reason !== 'oldest-unread' && data.reason !== 'all-completed-newest') throw Error('Invalid account Continue state');
      const winner = chapterRow(data.winner,'public-retained-v1'); current(); budget.check(); budget.accept(1);
      return Object.freeze({ scope,accountRevision:Number(body.revision),state:'ready',reason:data.reason,winner,hasMultipleEditions:data.hasMultipleEditions });
    }, async resume(winner, selection, budget) {
      const account = await admission(budget);
      const observe = async () => {
        const body = await request('progress', budget, { chapterId: winner.id }); envelope(body,account); catalogue(body.catalogIdentity);
        if (body.revision !== selection.accountRevision) throw Error('Your account progress changed. Retry Continue or start from the first chapter.');
        const data = record(body.data), page = data.page, count = data.pageCount, version = data.version;
        if (data.chapterId !== winner.id || data.mangaId !== mangaId || typeof data.completed !== 'boolean' || !Number.isSafeInteger(count) || Number(count) < 0 || Number(count) > 2147483647 || !Number.isSafeInteger(version) || Number(version) < 0 || page === null && version !== 0 || page !== null && (Number(version) < 1 || !Number.isSafeInteger(page) || Number(page) < 0 || Number(page) >= Number(count))) throw Error('Your saved page could not be verified. Start from the first chapter instead, or retry Continue.');
        current(); budget.check(); return { page, count, version, completed:data.completed };
      };
      const first = await observe(), checked = await observe();
      if (JSON.stringify(first) !== JSON.stringify(checked)) throw Error('Your saved page changed while checking it. Retry Continue; no other page was selected.');
      return checked.completed || checked.page === null ? 1 : Number(checked.page) + 1;
    }};
    const projection: CompletionAccess['projection'] = async (ids,budget) => {
      current(); budget.check(); if (!available) throw Error(reason); if (ids.length > 50 || new Set(ids).size !== ids.length) throw Error('Choose at most 50 distinct account chapters'); for (const id of ids) publicUuid(id); if (!ids.length) return Object.freeze([]);
      const account = await admission(budget), body = await request('completion',budget,{ids}); envelope(body,account); catalogue(body.catalogIdentity);
      if (!Array.isArray(body.selection) || body.selection.length !== ids.length || body.selection.some((id,index) => id !== ids[index]) || body.page !== 1 || body.limit !== 100 || !Array.isArray(body.data) || body.total !== body.data.length || body.data.length > ids.length) throw Error('Account completion coverage is incomplete. No missing chapter was assumed unread.');
      const done = new Set<string>(); for (const input of body.data) { const row = record(input), id = publicUuid(row.chapterId); if (!ids.includes(id) || row.mangaId !== mangaId || done.has(id) || typeof row.readAt !== 'string' || !Number.isFinite(Date.parse(row.readAt))) throw Error('Account completion escaped its selected chapters'); done.add(id); }
      current(); budget.check(); budget.accept(ids.length); return Object.freeze(ids.map(id => Object.freeze({id,completed:done.has(id)})));
    };
    return { continue: continuation, history: { key, status: source.selectionPending ? 'pending' : available ? 'available-native' : 'unavailable', reason, projection, async completed(ids,budget) { return (await projection(ids,budget)).some(row => row.completed); } } };
  };
}
