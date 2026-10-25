import type { Row } from './contract';
export function label(row: Pick<Row, 'volume' | 'chapter'>): string {
  const volume = row.volume === null ? '' : 'Vol. ' + (row.volume === '' ? '(empty label)' : row.volume) + ' · ';
  return volume + (row.chapter === null ? 'Oneshot' : row.chapter === '' ? 'Chapter (empty label)' : 'Ch. ' + row.chapter);
}
// Semantic buttons intentionally do not expose a stale direct-navigation href.
export function plainAction(event: MouseEvent, action: () => void | Promise<void>) {
  if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
  event.preventDefault(); void action(); // Controller catches every operation and publishes its owned error.
}

export function continueHistoryExplanation(mode: 'device' | 'account', publicCatalogue: boolean, pending: boolean): string {
  if (pending) return 'A library selection is pending. Continue waits before using either history source.';
  if (mode === 'device') return 'Continue checks device completion locally. Device history is never sent to the server.';
  if (publicCatalogue) return 'Native-account completion is unavailable in this public catalogue. Choose device history explicitly in Library if you want to use it.';
  return 'Continue checks selected chapter IDs against the signed-in native cookie account’s server history. Device completion remains local and is not sent with these requests.';
}
