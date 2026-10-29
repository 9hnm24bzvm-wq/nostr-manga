import { publicUuid } from '../public-api/validation';
// Match reader_selector::reader_filters canonical defaults and pair ordering.
// These are eligibility witnesses only; chapter ordering remains server-owned.
export function accountReaderFilters(filters: string): string {
  if (typeof filters !== 'string' || filters.length > 16384) throw Error('Account reader filter budget exceeded');
  const query = new URLSearchParams(filters), pairs: [string,string][] = [];
  for (const [raw, input] of query) {
    if (['limit','offset','includes','includes[]','manga','page','sort','order'].includes(raw) || raw.startsWith('order[')) throw Error('Account reader filters cannot change reader scope');
    const key = raw === 'chapter' ? 'chapter[]' : raw, value = ['externalUrl','excludeExternalUrl'].includes(key) ? input.toLowerCase() : input;
    pairs.push([key, value]);
  }
  if (!query.has('contentRating[]')) for (const value of ['safe','suggestive','erotica','pornographic']) pairs.push(['contentRating[]', value]);
  for (const [key,value] of [['includeEmptyPages','1'],['includeFuturePublishAt','1'],['includeFutureUpdates','1'],['includeExternalUrl','1'],['includeUnavailable','0']]) if (!query.has(key)) pairs.push([key,value]);
  pairs.sort((a,b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : a[1] < b[1] ? -1 : a[1] > b[1] ? 1 : 0);
  const out = new URLSearchParams(); let previous = '';
  for (const pair of pairs) { const key = JSON.stringify(pair); if (key !== previous) out.append(...pair); previous = key; }
  return out.toString().replace(/^\?/, '');
}
export function singletonAccountContinueFilters(filters: string, selected: string) {
  publicUuid(selected); const query = new URLSearchParams(filters), ids = query.getAll('ids[]');
  if (ids.length && !ids.includes(selected)) throw Error('Selected edition is outside the original ID filter');
  query.delete('ids[]'); query.append('ids[]', selected); return query.toString().replace(/^\?/, '');
}
