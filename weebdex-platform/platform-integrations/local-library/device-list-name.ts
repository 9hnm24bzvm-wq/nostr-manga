// Explicit saved-device edits. This module has no signer, relay or HTTP transport.
import { readDeviceMigrationState } from '../public-api/device-id-migration';
import { DeviceStore, normalizeDevice, sameScope, type InstanceScope } from './device-store';
import { request } from './protocol/indexeddb.js';
import { normalize } from './protocol/codec.js';
export interface DeviceListNameReview { readonly id: string; readonly name: string; readonly revision: number; readonly ownerId: string; readonly scope: Readonly<InstanceScope>; }
const reviews = new WeakMap<DeviceListNameReview, { device: DeviceStore; assertCurrent(): void }>();
export async function previewDeviceListName(device: DeviceStore, id: string, assertCurrent = () => {}): Promise<DeviceListNameReview> {
  if (!/^[0-9a-f]{32}$/.test(id)) throw Error('Choose an existing device list');
  assertCurrent(); const state = await readDeviceMigrationState(device.db); assertCurrent();
  if (state.migrationReason) throw Error(state.migrationReason);
  if (!state.ownerId || !sameScope(state.scope, device.scope)) throw Error('The saved-device owner or catalogue scope changed');
  const row = state.records.find(row => row.family === 'lists' && row.key === id && row.id === id);
  if (!row || typeof row.name !== 'string') throw Error('This list is no longer saved on this device');
  const review = Object.freeze({ id, name: row.name, revision: state.revision, ownerId: state.ownerId, scope: Object.freeze({ ...state.scope }) });
  reviews.set(review, { device, assertCurrent }); return review;
}
export async function renameDeviceList(device: DeviceStore, review: DeviceListNameReview, input: string, assertCurrent = () => {}) {
  const prepared = reviews.get(review);
  if (!prepared || prepared.device !== device) throw Error('Review the current device list before renaming it');
  const current = () => { prepared.assertCurrent(); assertCurrent(); };
  current();
  if (typeof input !== 'string' || !input.trim()) throw Error('Enter a list name');
  // The existing codec validates Unicode, control characters and byte/codepoint limits.
  const normalized = normalize('lists', { id: review.id, name: input.trim(), updated_at: 0 });
  if (normalized.name === review.name) {
    const state = await readDeviceMigrationState(device.db); current();
    if (state.revision !== review.revision || state.ownerId !== review.ownerId || !sameScope(state.scope, review.scope) || state.migrationReason || !state.records.some(row => row.family === 'lists' && row.key === review.id && row.id === review.id && row.name === review.name)) throw Error('The device library changed. Reopen Rename before saving.');
    reviews.delete(review); return { changed: false, revision: state.revision };
  }
  const revision = await device.mutate(async (rows, meta) => {
    current();
    const [owner, scope, old] = await Promise.all([request(meta.get('deviceOwnerId')), request(meta.get('instanceScope')), request(rows.get(['lists', review.id]))]);
    current();
    if (owner !== review.ownerId || !sameScope(scope, review.scope) || !old || old.id !== review.id || old.name !== review.name) throw Error('The saved list changed. Reopen Rename before saving.');
    rows.put(normalizeDevice({ ...old, name: normalized.name, updated_at: Math.max(Math.floor(Date.now() / 1000), old.updated_at + 1) }));
    current();
  }, review.revision);
  reviews.delete(review); return { changed: true, revision };
}
