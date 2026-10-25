import { get } from 'svelte/store';
import { PUBLIC_API_URL, PUBLIC_MANGADEX_CATALOGUE } from '$env/static/public';
import { encodeParams, assertCatalogueScope, assertCurrentCatalogueScopeValue, currentDeviceSnapshot, captureLibraryView, assertLibraryView, captureLibraryContext, assertLibraryContext, localLibraryState, isCurrentCatalogueId } from './library';
import { captureNativeAuthority, sameNativeReadAuthority } from '$platform/mutation-authority';
import { UpdatesPager, type UpdateKey, type UpdateRow } from './updates-pagination';
import { TitleIndex } from './title-index';
import type { HistoryBudget } from './history-pagination';
import { historyJson } from './history-transport';
import { publicUpdatesSource } from '../public-api/updates-catalogue';
export async function openDeviceUpdatesFeed(input: URLSearchParams, latestOnly = false, assertOwner: () => void = () => {}): Promise<UpdatesPager<Chapter & UpdateRow>> {
  assertOwner(); const authority = captureNativeAuthority(); await assertCatalogueScope(); assertOwner();
  const view = captureLibraryView(), context = captureLibraryContext(); if (context.mode !== 'device') throw Error('Device updates cannot read an account library');
  const check = () => { assertOwner(); assertLibraryView(view); assertLibraryContext(context); if (!sameNativeReadAuthority(authority)) throw Error('Update account authority changed. Reopen this view.'); };
  const query = new URLSearchParams(input);
  for (const field of ['sort','order','limit','page']) if (query.getAll(field).length > 1) throw Error('Duplicate device update control: ' + field);
  if ([...query.keys()].some(key => ['id','ids','manga','offset'].includes(key.replace(/\[\]$/, '')) || key.startsWith('order['))) throw Error('Update scope and continuation are owned by the device library');
  const limit = Number(query.get('limit') ?? 100), direction = query.get('order') ?? 'desc'; if (!['asc','desc'].includes(direction)) throw Error('Invalid update ordering');
  const snapshot = await currentDeviceSnapshot(); check(); assertCurrentCatalogueScopeValue(snapshot.scope);
  if (snapshot.migration_reason || snapshot.revision !== get(localLibraryState).revision || snapshot.records.length > 2000000) throw Error('The device update snapshot changed or exceeded its record budget. Reopen this view.');
  const parents = snapshot.records.filter(row => row.family === 'series' && row.notification && isCurrentCatalogueId(row.id)).map(row => row.id!);
  for (const field of ['page','limit','tagx','txmod']) query.delete(field); // Retained personal-update preferences deliberately omit tag exclusions.
  const scope = { ...snapshot.scope }, source = PUBLIC_MANGADEX_CATALOGUE === '1' ? publicUpdatesSource(query) : nativeUpdatesSource(query);
  const index = await TitleIndex.create(check);
  const owned = () => { check(); assertCurrentCatalogueScopeValue(scope); };
  try { owned(); return new UpdatesPager(parents,index,{
    primaryOrderOnly: source.primaryOrderOnly,
    async scan(parent,offset,size,budget) { owned(); const rows = await source.scan(parent,offset,size,budget); owned(); return rows; },
    async hydrate(keys,budget) { owned(); const rows = await source.hydrate(keys,budget); owned(); return rows; }
  },owned,limit,direction === 'asc' ? 1 : -1,latestOnly); } catch (error) { await index.dispose(); throw error; }
}
function nativeUpdatesSource(input: URLSearchParams) {
  async function read(query: URLSearchParams, offset: number, limit: number, budget: HistoryBudget) {
    if (offset % limit) throw Error('Native update continuation offset changed');
    query.set('page',String(offset / limit + 1)); query.set('limit',String(limit));
    const response = await historyJson(PUBLIC_API_URL.replace(/\/$/,'') + '/chapter/feed?' + encodeParams(query),budget,true); budget.check();
    if (response.status < 200 || response.status >= 300) throw Error('Update catalogue lookup failed (' + response.status + ')');
    const body = response.body as { total: number; data: (Chapter & UpdateRow)[]; page: number; limit: number };
    if (!Number.isSafeInteger(body.total) || body.total < 0 || !Array.isArray(body.data) || body.page !== offset / limit + 1 || body.limit !== limit || body.data.length !== Math.min(limit,Math.max(0,body.total-offset))) throw Error('Invalid native update continuation page');
    return body;
  }
  return {
    primaryOrderOnly: false,
    scan(parent: string,offset: number,limit: number,budget: HistoryBudget) { const query = new URLSearchParams(input); query.set('manga',parent); return read(query,offset,limit,budget); },
    async hydrate(keys: readonly UpdateKey[],budget: HistoryBudget) { const query = new URLSearchParams(input); for (const key of keys) query.append('id',key.id); const response = await read(query,0,100,budget); if (response.total > keys.length) throw Error('Native update hydration exceeded its exact scope'); return response.data; }
  };
}
