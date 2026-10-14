// Actual Group/User route controls only. Importing starts no browser or fixture.
// Native response delivery is delayed/aborted; successful data is never invented.
import { actorFollowHeader, actorFollowControl } from './browser-actor-follow-controls.mjs';
export const actorFollowPhase = 'ui14-actor-follow';
export const actorFollowCaseChecks = Object.freeze(Object.fromEntries(Object.entries({
  'initial-readiness': ['prerequisite', 'initial-session', 'status-pending', 'status-settled'],
  'read-retry': ['prerequisite', 'delivery-failed', 'retry-pending', 'retry-settled'],
  'unblock-loading': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
  'unblock-error': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
  'pending-ack': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
  'pending-lost': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
  'pending-disposed': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
}).map(([name, checks]) => [name, Object.freeze(checks)])));
export const actorFollowDiagnosticChecks = Object.freeze({ [actorFollowPhase]: Object.freeze([
  'actor-follow-account', ...['group', 'user'].flatMap(kind => Object.entries(actorFollowCaseChecks).flatMap(([name, checks]) => {
    const id = `actor-follow-${kind}-${name}`;
    return [id, ...checks.map(check => id + '-' + check)];
  }))
]) });

// Observe browser gestures only. Harness prerequisite API calls are not page
// requests. No URL, header, body, account identity or error enters this receipt.
export function watchActorFollowRequests(page, origin, endpoint) {
  const counts = { GET: 0, POST: 0, DELETE: 0, PUT: 0 };
  const request = value => {
    const url = new URL(value.url()), method = value.method();
    if (url.origin !== origin) return;
    if (url.pathname === endpoint && ['GET', 'POST', 'DELETE'].includes(method)) counts[method]++;
    if (url.pathname === '/api/settings' && method === 'PUT') counts.PUT++;
  };
  page.on('request', request);
  return { counts, close: () => page.off('request', request) };
}

export async function runActorFollowJourneys(h) {
  const { phase, page, origin, ids, api, step, check, insist, open, signup, hold, requestPath, until, stable, realm, sameRealm } = h;
  insist(phase === actorFollowPhase, 'Unknown actor-follow phase');
  const run = (id, action) => step(id, () => check(id, () => action((suffix, operation) => check(id + '-' + suffix, operation))));
  await run('actor-follow-account', async () => {
    await signup();
    const actual = await api('/api/auth/check');
    insist(actual.status === 200 && actual.body?.identity?.is_verified && ids.group && ids.seeder && actual.body.identity.id !== ids.seeder, 'A real verified non-self account and native Group/User targets are required');
    return { actual_verified_account: true, native_seed_targets: true };
  });
  for (const [kind, id, collection, preference] of [['group', ids.group, 'groups', 'groups'], ['user', ids.seeder, 'users', 'uploaders']]) {
    const route = `/${kind}/${id}`, endpoint = `/api/user/follows/${collection}/${id}`;
    const header = () => actorFollowHeader(page);
    const button = name => actorFollowControl(page, name);
    const enabled = async name => { await button(name).waitFor({ state: 'visible' }); await until(() => button(name).isEnabled(), Boolean, 'The current entity control never became enabled'); };
    const publicReady = async () => { await header().locator('h1').waitFor({ state: 'visible' }); insist(await header().count() === 1 && new URL(page.url()).pathname === route, 'The actual entity header/route is not unique and current'); };
    const noWrites = counts => insist(counts.POST === 0 && counts.DELETE === 0 && counts.PUT === 0, 'A readiness/retry gesture wrote follows or account preferences');
    const absentBlock = async () => insist(await button('Block').count() === 0, 'Unknown/followed status exposed a new Block action');
    const prepare = async (followed = false, blocked = false) => {
      const actual = await api('/api/auth/check'), prefs = actual.body?.preferences;
      insist(actual.status === 200 && actual.body?.identity?.id && prefs && Array.isArray(prefs[preference]), 'Actual account preferences are unavailable');
      const next = prefs[preference].map(row => row.id).filter(value => value !== id); if (blocked) next.push(id);
      const saved = await api('/api/settings', { method: 'PUT', data: { version: prefs.version, [preference]: next } });
      insist(saved.status === 200 && saved.body?.[preference]?.some(row => row.id === id) === blocked, 'Genuine native block prerequisite did not persist');
      const changed = await api(endpoint, { method: followed ? 'POST' : 'DELETE' });
      insist(changed.status === 204 && (await api(endpoint)).status === (followed ? 204 : 404), 'Genuine native follow prerequisite did not persist');
      return { preferencesVersion: saved.body.version };
    };
    const close = async gates => {
      const results = await Promise.allSettled(gates.map(gate => gate.close()));
      const failed = results.find(result => result.status === 'rejected'); if (failed) throw failed.reason;
    };
    const clickDisabledAgain = async name => {
      insist(await button(name).isDisabled(), 'The unsettled operation left its control actionable');
      const box = await button(name).boundingBox(); insist(box && box.width > 0 && box.height > 0, 'The pending native control has no pointer target');
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { clickCount: 2 });
    };

    await run(`actor-follow-${kind}-initial-readiness`, async inspect => {
      await inspect('prerequisite', () => prepare(true)); const watch = watchActorFollowRequests(page, origin, endpoint), gates = [];
      try {
        const status = await inspect('initial-session', async () => {
          const auth = await hold(origin + '/api/auth/check', request => requestPath(request, '/api/auth/check') && !request.headers()['x-weebdex-expected-account']); gates.push(auth);
          const status = await hold(origin + endpoint, request => requestPath(request, endpoint)); gates.push(status);
          await open(route); await auth.wait(); await publicReady();
          await stable(async () => watch.counts.GET === 0 && watch.counts.POST === 0 && watch.counts.DELETE === 0 && watch.counts.PUT === 0 && await button('Follow').count() === 0 && await button('Unfollow').count() === 0 && await button('Block').count() === 0, 'The anonymous initial entity view issued personal work or exposed account actions', 350);
          await auth.release(); insist(auth.nativeStatus === 200 && auth.disposition === 'delivered', 'Initial Session delivery was not the genuine native200'); await status.wait(); return status;
        });
        await inspect('status-pending', async () => {
          await button('Follow').waitFor({ state: 'visible' }); insist(await button('Follow').isDisabled(), 'Follow became actionable before current status readiness'); await absentBlock(); noWrites(watch.counts);
        });
        await inspect('status-settled', async () => {
          await status.release(); insist(status.nativeStatus === 204 && status.disposition === 'delivered', 'Existing native follow was not actually delivered'); await enabled('Unfollow'); await absentBlock();
          await button('Unfollow').click(); await enabled('Follow');
          insist(watch.counts.GET === 1 && watch.counts.DELETE === 1 && watch.counts.POST === 0 && watch.counts.PUT === 0 && (await api(endpoint)).status === 404, 'Existing follow did not choose exactly one durable native DELETE');
          await enabled('Block');
        });
        return { delayed_genuine_auth200: true, public_route_before_session: true, initial_private_requests: 0, genuine_status204: true, current_unfollow_delete_once: true, durable_status404: true };
      } finally { watch.close(); await close(gates); }
    });

    await run(`actor-follow-${kind}-read-retry`, async inspect => {
      await inspect('prerequisite', () => prepare()); const watch = watchActorFollowRequests(page, origin, endpoint), gates = [];
      try {
        await inspect('delivery-failed', async () => {
          const failed = await hold(origin + endpoint, request => requestPath(request, endpoint)); gates.push(failed);
          await open(route); await failed.wait(); await failed.fail();
          insist(failed.nativeStatus === 404 && failed.disposition === 'failed-delivery-after-real-response', 'Read failure was not delivery loss after a genuine native404');
          await enabled('Retry follow status'); await absentBlock(); noWrites(watch.counts);
        });
        const boundary = watch.counts.GET, retried = await hold(origin + endpoint, request => requestPath(request, endpoint)); gates.push(retried);
        await inspect('retry-pending', async () => {
          await button('Retry follow status').dblclick(); await retried.wait();
          insist(watch.counts.GET === boundary + 1 && await button('Follow').isDisabled(), 'Repeated Retry did not coalesce into one current status read'); await absentBlock(); noWrites(watch.counts);
        });
        await inspect('retry-settled', async () => {
          await retried.release(); insist(retried.nativeStatus === 404 && retried.disposition === 'delivered', 'Retry did not deliver the actual native absent state'); await enabled('Follow'); await enabled('Block');
          await stable(async () => watch.counts.GET === boundary + 1 && watch.counts.POST === 0 && watch.counts.DELETE === 0 && watch.counts.PUT === 0, 'Read-only retry replayed work');
        });
        return { failure_kind: 'browser-delivery-abort-after-native404', retry_status_reads: 1, retry_mutations: 0, absent_status_restored: true };
      } finally { watch.close(); await close(gates); }
    });

    for (const failedDelivery of [false, true]) await run(`actor-follow-${kind}-unblock-${failedDelivery ? 'error' : 'loading'}`, async inspect => {
      const prepared = await inspect('prerequisite', () => prepare(false, true)); const watch = watchActorFollowRequests(page, origin, endpoint), gates = [];
      try {
        const status = await hold(origin + endpoint, request => requestPath(request, endpoint)); gates.push(status);
        await inspect('status-held', async () => {
          await open(route); await status.wait();
          if (failedDelivery) { const before = await page.getByText('Unknown Error', { exact: true }).count(); await status.fail(); insist(status.nativeStatus === 404 && status.disposition === 'failed-delivery-after-real-response', 'Blocked status read did not actually lose its native404 delivery'); await until(() => page.getByText('Unknown Error', { exact: true }).count(), count => count > before, 'The failed current follow read did not attach its new error feedback'); }
          await enabled('Unblock'); insist(await button('Follow').count() === 0 && await button('Retry follow status').count() === 0, 'An existing block exposed a follow control'); noWrites(watch.counts);
        });
        await inspect('unblock-committed', async () => {
          await button('Unblock').click();
          // Establish the actual settings effect while the status response is
          // still withheld: a deferred-until-follow-read Unblock must fail here.
          const actual = await until(() => api('/api/auth/check'), value => value.status === 200 && !value.body.preferences[preference].some(row => row.id === id) && watch.counts.PUT === 1, 'Legitimate Unblock waited for follow status instead of persisting its native settings PUT');
          insist(actual.body.preferences.version === prepared.preferencesVersion + 1 && watch.counts.POST === 0 && watch.counts.DELETE === 0, 'Legitimate Unblock did not persist exactly one current native settings version');
        });
        await inspect('follow-restored', async () => {
          if (!failedDelivery) await status.release();
          await enabled('Follow');
        });
        return { unknown_read: failedDelivery ? 'delivery-error' : 'loading', legitimate_unblock_put_once: true, actual_preferences_unblocked: true };
      } finally { watch.close(); await close(gates); }
    });

    for (const outcome of ['ack', 'lost', 'disposed']) await run(`actor-follow-${kind}-pending-${outcome}`, async inspect => {
      await inspect('prerequisite', () => prepare()); const watch = watchActorFollowRequests(page, origin, endpoint), gates = [];
      try {
        const pending = await inspect('write-committed', async () => {
          await open(route); await enabled('Follow'); await realm();
          const pending = await hold(origin + endpoint, request => requestPath(request, endpoint, 'POST')); gates.push(pending);
          await button('Follow').click(); await pending.wait();
          insist((await api(endpoint)).status === 204, 'The held follow write was not actually committed at the native backend');
          return pending;
        });
        await inspect('pending-controls', async () => {
          await clickDisabledAgain('Follow');
          insist(await button('Unfollow').count() === 0 && (await button('Block').count() === 0 || await button('Block').isDisabled()) && watch.counts.POST === 1 && watch.counts.DELETE === 0 && watch.counts.PUT === 0, 'Pending follow toggled optimistically or admitted another mutation');
        });
        await inspect('delivery-settled', async () => {
          if (outcome === 'disposed') {
            await page.getByRole('link', { name: 'Advanced Search', exact: true }).first().click(); await page.waitForURL(url => url.pathname === '/search'); await sameRealm();
            // A later genuine native deletion makes the new view observably
            // different from the old POST204; cancellation itself proves no rollback.
            insist((await api(endpoint, { method: 'DELETE' })).status === 204 && (await api(endpoint)).status === 404, 'The later native deletion did not establish a distinct reentry state');
            const beforeReentry = watch.counts.GET;
            await page.goBack({ waitUntil: 'domcontentloaded' }); await sameRealm(); await publicReady(); await enabled('Follow');
            insist(watch.counts.GET === beforeReentry + 1, 'Reentered entity did not establish exactly one fresh current status GET');
            await pending.release(); insist(pending.nativeStatus === 204 && ['delivered', 'browser-cancelled'].includes(pending.disposition), 'Disposed write lacked its genuine native204/cancellation disposition');
            await stable(async () => new URL(page.url()).pathname === route && await button('Follow').isEnabled() && await button('Unfollow').count() === 0 && await button('Retry follow status').count() === 0 && await page.getByText('Request outcome unknown', { exact: true }).count() === 0, 'Obsolete POST204 changed the reentered current absent view or attached stale feedback');
          } else if (outcome === 'lost') {
            const boundary = watch.counts.GET; await pending.fail();
            insist(pending.nativeStatus === 204 && pending.disposition === 'failed-delivery-after-real-response', 'Mutation uncertainty was not delivery loss after native204');
            await enabled('Retry follow status'); await absentBlock();
            await stable(async () => watch.counts.GET === boundary && watch.counts.POST === 1, 'An uncertain mutation was automatically reread or replayed');
            const retry = await hold(origin + endpoint, request => requestPath(request, endpoint)); gates.push(retry);
            await button('Retry follow status').dblclick(); await retry.wait(); insist(watch.counts.GET === boundary + 1, 'Uncertain-write Retry did not coalesce one status GET'); noExtraWrites();
            await retry.release(); insist(retry.nativeStatus === 204 && retry.disposition === 'delivered', 'Uncertain-write Retry did not discover the genuine committed follow'); await enabled('Unfollow');
          } else { await pending.release(); insist(pending.nativeStatus === 204 && pending.disposition === 'delivered', 'Current follow did not receive its actual native204'); await enabled('Unfollow'); }
          function noExtraWrites() { insist(watch.counts.POST === 1 && watch.counts.DELETE === 0 && watch.counts.PUT === 0, 'The original follow was replayed or another mutation was admitted'); }
          noExtraWrites();
          if (outcome === 'disposed') await enabled('Block'); else await absentBlock();
          insist((await api(endpoint)).status === (outcome === 'disposed' ? 404 : 204), 'The actual current native follow state changed unexpectedly');
          await stable(async () => watch.counts.POST === 1 && watch.counts.DELETE === 0 && watch.counts.PUT === 0, 'Post-settlement rendering replayed the follow write');
        });
        return { genuine_committed204: true, original_posts: 1, delivery: pending.disposition, read_only_reconciliation: outcome === 'lost', same_document_disposal_reentry: outcome === 'disposed', subsequent_native_delete: outcome === 'disposed', durable_current_status: outcome === 'disposed' ? 404 : 204 };
      } finally { watch.close(); await close(gates); }
    });
  }
}
