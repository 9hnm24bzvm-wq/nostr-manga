import type { DeviceRecord } from './device-store';

// Presentation reads only the already-owned local snapshot. No catalogue scan,
// metadata request or storage transaction is needed to distinguish empty states.
export function libraryDiscovery(records: readonly DeviceRecord[], list: string | null, acceptsId: (id: unknown) => boolean) {
  const saved = new Set<string>();
  let listName: string | undefined;
  for (const row of records) {
    if (row.family === 'series' && row.id && acceptsId(row.id)) saved.add(row.id);
    if (list && row.family === 'lists' && row.id === list) listName = row.name;
  }
  let listSaved = 0;
  if (list) for (const row of records) if (row.family === 'members' && row.list === list && row.series && saved.has(row.series)) listSaved++;
  return { saved: saved.size, listSaved, listName };
}

export function unfilteredLibraryHref(list: string | null): string {
  return list && /^[0-9a-f]{32}$/.test(list) ? '/library?list=' + list : '/library';
}
