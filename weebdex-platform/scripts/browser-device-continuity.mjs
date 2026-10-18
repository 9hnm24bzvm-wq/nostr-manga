// Owned same-context pages exercise real local controls. This helper itself
// never opens IndexedDB, sends a mutation, patches storage or fabricates rows.
import assert from 'node:assert/strict';
import {selectTitleLibraryStatus, titleLibraryTrigger} from './browser-select-controls.mjs';

export async function exerciseDeviceContinuity({page, context, origin, manga, readDevice, outboxCount, until}) {
  const target = new URL(origin);
  assert.ok(target.protocol === 'http:' && ['127.0.0.1', 'localhost'].includes(target.hostname) && target.origin === origin, 'Exact owned loopback origin required');
  assert.match(manga, /^[A-Za-z0-9]{10}$/, 'Native device continuity requires the native-profile fixture target');
  const pathname = '/title/' + manga;
  assert.equal(new URL(page.url()).origin + new URL(page.url()).pathname, origin + pathname, 'Current title must own the local continuity action');
  let sibling, offlineAttempted = false, primary, result;
  const cleanupErrors = [];
  let mutationRequests = 0;
  const observe = request => {
    if (['GET', 'HEAD', 'OPTIONS'].includes(request.method())) return;
    const url = new URL(request.url());
    // Ordinary anonymous title-view accounting is the only non-read exception.
    // Settings, status, ratings, protocol and future command routes all refuse.
    if (request.method() === 'POST' && url.origin === origin && url.pathname === `/api/analytics/manga/${manga}/views` && !url.search && !url.hash) return;
    mutationRequests++;
  };
  const durable = status => until(readDevice, snapshot => snapshot?.records.some(row => row.family === 'series' && row.id === manga && row.status === status), 'Exact device title status did not persist');
  context.on('request', observe);
  try {
    sibling = await context.newPage();
    const response = await sibling.goto(origin + pathname, {waitUntil: 'domcontentloaded'});
    assert.equal(response?.status(), 200, 'Sibling must load the actual same-origin title');
    await titleLibraryTrigger(sibling, 'Reading').waitFor({state: 'visible'});
    let siblingNavigations = 0;
    const navigation = frame => { if (frame === sibling.mainFrame()) siblingNavigations++; };
    sibling.on('framenavigated', navigation);
    try {
      await selectTitleLibraryStatus(page, 'On Hold');
      await durable('on_hold');
      await titleLibraryTrigger(sibling, 'On Hold').waitFor({state: 'visible'});
      await selectTitleLibraryStatus(sibling, 'Reading');
      await durable('reading');
      await titleLibraryTrigger(page, 'Reading').waitFor({state: 'visible'});
      offlineAttempted = true;
      await context.setOffline(true);
      await selectTitleLibraryStatus(page, 'On Hold');
      await durable('on_hold');
      await titleLibraryTrigger(sibling, 'On Hold').waitFor({state: 'visible'});
      assert.equal(siblingNavigations, 0, 'Cross-tab status must update without reloading the observer');
      const jobs = await outboxCount();
      assert.equal(jobs, 0, 'A device-only continuity action must not queue public delivery');
      assert.equal(mutationRequests, 0, 'A device-only continuity action must not call account or public mutation routes');
      result = {same_context_tabs: 2, observer_updated_without_reload: true, roundtrip_status_durable: true, offline_mounted_device_write_durable: true, public_outbox_jobs: jobs, authority_mutation_requests: mutationRequests};
    } finally { sibling.off('framenavigated', navigation); }
  } catch (error) { primary = error; }
  finally {
    if (offlineAttempted) try { await context.setOffline(false); }
    catch { cleanupErrors.push('Owned browser online restoration was not confirmed'); }
    if (sibling) try { await sibling.close(); }
    catch { cleanupErrors.push('Owned sibling page closure was not confirmed'); }
    if (!primary && cleanupErrors.length === 0) try {
      await selectTitleLibraryStatus(page, 'Reading');
      await durable('reading');
      await titleLibraryTrigger(page, 'Reading').waitFor({state: 'visible'});
      assert.equal(await outboxCount(), 0, 'Status restoration must not queue public delivery');
      assert.equal(mutationRequests, 0, 'Status restoration must stay device-only');
    } catch (error) { primary = error; }
    context.off('request', observe);
  }
  if (primary && cleanupErrors.length) throw new AggregateError([primary, ...cleanupErrors.map(message => Error(message))], 'Device continuity failed and cleanup was not confirmed');
  if (primary) throw primary;
  assert.deepEqual(cleanupErrors, [], 'Device continuity cleanup must complete before acceptance');
  return {...result, authority_mutation_requests: mutationRequests, online_restored: true, sibling_closed: true, original_status_restored: true};
}
