// Read-only, finite observations of the still-owned initial status hold. These
// are diagnostics, never a replacement for strict locator/assertion failures.
import { actorFollowHeader, actorFollowControl } from './browser-actor-follow-controls.mjs';

const counts = ['zero', 'one', 'many', 'unavailable'];
const controls = ['absent', 'enabled', 'disabled', 'multiple', 'unavailable'];
export const actorInitialObservationValues = Object.freeze(Object.fromEntries(Object.entries({
  kind: ['group', 'user'], route: ['current', 'other', 'unavailable'], header: counts,
  follow: controls, unfollow: controls, retry: controls, block: controls, unblock: controls,
  get: counts, post: counts, delete: counts, put: counts,
  request_actor: ['prerequisite-actor', 'other-actor', 'anonymous', 'unavailable'],
  held_status: ['native204', 'native404', 'other-native', 'unavailable'],
  delivery: ['pending', 'delivered', 'failed-delivery-after-real-response', 'browser-cancelled', 'unavailable'],
}).map(([key, values]) => [key, Object.freeze([...values])])));

const countClass = value => Number.isSafeInteger(value) && value >= 0 ? value === 0 ? 'zero' : value === 1 ? 'one' : 'many' : 'unavailable';
const safely = async read => { try { return await read(); } catch { return 'unavailable'; } };
export async function captureActorInitialObservation(page, kind, route, counts, gate, requestActor) {
  const row = {
    kind,
    route: await safely(() => new URL(page.url()).pathname === route ? 'current' : 'other'),
    header: await safely(async () => countClass(await actorFollowHeader(page).count())),
  };
  for (const [key, label] of [['follow', 'Follow'], ['unfollow', 'Unfollow'], ['retry', 'Retry follow status'], ['block', 'Block'], ['unblock', 'Unblock']]) {
    // evaluateAll is immediate and non-strict: missing/duplicate controls are
    // classified, never clicked or awaited into a successful diagnostic state.
    row[key] = await safely(() => actorFollowControl(page, label).evaluateAll(nodes => nodes.length === 0 ? 'absent' : nodes.length > 1 ? 'multiple' : nodes[0].tagName === 'BUTTON' && typeof nodes[0].disabled === 'boolean' ? nodes[0].disabled ? 'disabled' : 'enabled' : 'unavailable'));
  }
  for (const method of ['GET', 'POST', 'DELETE', 'PUT']) row[method.toLowerCase()] = countClass(counts[method]);
  row.request_actor = actorInitialObservationValues.request_actor.includes(requestActor) ? requestActor : 'unavailable';
  row.held_status = await safely(() => gate.heldNativeStatus === 204 ? 'native204' : gate.heldNativeStatus === 404 ? 'native404' : Number.isInteger(gate.heldNativeStatus) && gate.heldNativeStatus >= 100 && gate.heldNativeStatus <= 599 ? 'other-native' : 'unavailable');
  row.delivery = await safely(() => actorInitialObservationValues.delivery.includes(gate.disposition) ? gate.disposition : 'unavailable');
  return row;
}
