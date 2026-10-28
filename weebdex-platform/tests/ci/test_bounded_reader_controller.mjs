// SOURCE-AUTHORED, UNEXECUTED. Actual controller, query, transport and DTO.
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture, row, id, deferred, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
for (const publicMode of [false, true]) {
  test((publicMode ? 'public' : 'native') + ': real query, profile, cookie mode and semantic neighbor wiring', async () => {
    const f = await fixture(publicMode); await f.controller.refresh(); await f.controller.semantic('previous', true);
    assert.equal(f.calls.length, 2); assert.equal(f.calls[0].url.pathname, (publicMode ? '/api/v1' : '/api') + '/manga/' + f.destination.mangaId + '/reader');
    assert.equal(f.calls[0].url.searchParams.get(publicMode ? 'translatedLanguage[]' : 'tlang'), 'en'); assert.equal(f.calls[0].init.credentials, publicMode ? 'omit' : 'same-origin');
    assert.equal(f.calls[1].url.searchParams.get('operation'), 'previous'); assert.equal(f.navigated[0].target.id, id(publicMode, 1)); assert.equal(f.navigated[0].lastPage, true);
    assert(f.calls.every(call => !call.url.searchParams.has('offset') && !call.url.searchParams.has('history'))); assert(f.allocations.every(size => size === 256 * 1024)); f.controller.dispose(); assert.equal(f.timers.size, 0);
  });
  test((publicMode ? 'public' : 'native') + ': nullable labels and server full-set winner are not locally resorted', async () => {
    const f = await fixture(publicMode), lateWinner = row(publicMode, 10001, '2.0'), selected = row(publicMode, 4, '2.0');
    await f.controller.refresh(); f.responder = url => f.envelope(url, { data: [selected], groupAnchor: selected, winner: lateWinner, hasMultipleEditions: true });
    await f.controller.editions(selected); assert.equal(f.controller.snapshot.winner.id, lateWinner.id); assert.equal(f.controller.snapshot.editions.length, 1); assert.equal(f.calls.length, 2);
    f.controller.dispose();
  });
}
test('same ID with moved exact nullable label, changed witness or neighbor resets before navigation', async () => {
  for (const field of ['chapter', 'witness', 'id']) {
    const f = await fixture(); await f.controller.refresh(); const next = row(false, 3, '3', { [field]: field === 'chapter' ? null : field === 'witness' ? 'f'.repeat(64) : 'other' });
    f.responder = url => f.envelope(url, { data: [next] }); await f.controller.semantic('next'); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.status, 'restart'); f.controller.dispose();
  }
});
test('first/last revalidate twice and do not use aggregate indices', async () => {
  const f = await fixture(); f.current = undefined; await f.controller.semantic('first'); assert.equal(f.calls.length, 2); assert(f.calls.every(call => call.url.searchParams.get('operation') === 'first')); assert.equal(f.navigated[0].target.id, 'chapter1'); f.controller.dispose();
  const g = await fixture(); g.current = undefined; let count = 0; g.responder = url => g.envelope(url, { data: [row(false, ++count)] }); await g.controller.semantic('first'); assert.equal(g.navigated.length, 0); assert.equal(g.controller.snapshot.status, 'restart'); g.controller.dispose();
});
for (const change of ['account', 'history', 'query', 'service', 'dispose']) test(change + ' change fences late response publication', async () => {
  const f = await fixture(), gate = deferred(); f.responder = async url => { const value = f.envelope(url); await gate.promise; return value; }; const pending = f.controller.refresh(); await turns();
  if (change === 'dispose') f.controller.dispose(); else if (change === 'query') { f.destination.filters = 'tlang=ja'; f.controller.sync(); } else if (change === 'service') { f.destination.origin = 'https://other.test'; f.controller.sync(); } else f.changeEpoch();
  gate.resolve(); await pending; assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.current, undefined); f.controller.dispose();
});
test('excluded current disables navigation while direct chapter ownership is untouched', async () => {
  const f = await fixture(); f.responder = url => f.envelope(url, { state: 'excluded-current', data: [], current: undefined }); await f.controller.refresh(); await f.controller.semantic('next'); assert.equal(f.navigated.length, 0); assert.equal(f.current.id, 'chapter2'); f.controller.dispose();
});
for (const status of ['pending', 'incomplete', 'unavailable']) test(status + ' history is never treated as empty or sent to the server', async () => {
  const f = await fixture(); f.current = undefined; f.historyStatus = status; await f.controller.continueScan(); assert.equal(f.calls.length, 0); assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.navigated.length, 0); f.controller.dispose();
});
test('observed exhaustion under behind-cursor drift never automatically navigates newest or claims all-read', async () => {
  const f = await fixture(); f.current = undefined; const a = row(false, 1), b = row(false, 2); f.completed = new Set([a.id, b.id]);
  f.responder = url => f.envelope(url, { data: url.searchParams.has('after') ? [b] : [a], nextCursor: url.searchParams.has('after') ? null : 'cursor1' });
  await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'observed-end'); assert.match(f.controller.snapshot.message, /cannot prove all-read/); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 2);
  // A newly inserted older group behind cursor is intentionally not inferred.
  const older = row(false, 0); f.responder = url => f.envelope(url, { data: [older], nextCursor: null, ...(url.searchParams.get('operation') === 'editions' ? { groupAnchor: older, winner: older, hasMultipleEditions: false } : {}) }); await f.controller.continueScan(true); assert.equal(f.controller.snapshot.status, 'candidate'); assert.equal(f.controller.snapshot.candidate.id, older.id); f.controller.dispose();
});
test('10K+ groups traverse only per-action pages and retain a bounded window', async () => {
  const f = await fixture(); f.current = undefined; let offset = 0;
  f.responder = url => { const data = Array.from({ length: Math.min(50, 10001 - offset) }, (_, i) => row(false, offset + i + 1)); offset += data.length; return f.envelope(url, { data, nextCursor: offset < 10001 ? 'cursor' + offset : null }); };
  for (let page = 0; page < 201; page++) { await f.controller.groups(page > 0); assert(f.controller.snapshot.groups.length <= 50); assert.equal(f.controller.snapshot.editions.length, 0); }
  assert.equal(offset, 10001); assert.equal(f.calls.length, 201); assert.equal(f.controller.snapshot.groupsMore, false); f.controller.dispose();
});
test('repeated actions own one request and cancellation rejection is observed', async () => {
  const f = await fixture(), gate = deferred(), unhandled = []; const observe = error => unhandled.push(error); process.on('unhandledRejection', observe);
  try { f.responder = () => gate.promise; const first = f.controller.refresh(); await turns(); await f.controller.refresh(); assert.equal(f.calls.length, 1); f.controller.cancel(); gate.reject(Error('late fetch rejection')); await first; await turns(); assert.deepEqual(unhandled, []); } finally { process.off('unhandledRejection', observe); f.controller.dispose(); }
});

test('title Start opens the bounded edition chooser instead of navigating when multiple editions exist', async () => {
  const f = await fixture(); f.current = undefined; const first = row(false, 1), other = row(false, 10001, '1');
  f.responder = url => f.envelope(url, { data: url.searchParams.get('operation') === 'editions' ? [other] : [first], hasMultipleEditions: true, ...(url.searchParams.get('operation') === 'editions' ? { groupAnchor: first, winner: first, nextCursor: 'nextEditionPage' } : {}) });
  await f.controller.start(); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 3); assert.equal(f.controller.snapshot.editions[0].id, other.id); assert.equal(f.controller.snapshot.winner.id, first.id); assert.equal(f.controller.snapshot.editionsMore, true); f.controller.dispose();
});
test('title Start with one edition navigates only after semantic revalidation', async () => {
  const f = await fixture(); f.current = undefined; await f.controller.start(); assert.equal(f.calls.length, 2); assert.equal(f.navigated[0].target.id, 'chapter1'); f.controller.dispose();
});

test('native colon-valid IDs, positive versions and scalar/byte schema bounds use the actual wire validator', async () => {
  const f = await fixture(); f.current = { ...f.current, id: 'native:chapter' }; await f.controller.refresh(); assert.equal(f.controller.snapshot.current.id, 'native:chapter'); f.controller.dispose();
  for (const overrides of [{ version: 0 }, { chapter: 'x'.repeat(33) }, { language: 'é'.repeat(25) }]) {
    const g = await fixture(); g.responder = url => g.envelope(url, { data: [row(false, 2, '2', overrides)] }); await g.controller.refresh(); assert.equal(g.controller.snapshot.status, 'error'); g.controller.dispose();
  }
  const h = await fixture(); h.current = row(false, 2, '😀'.repeat(32)); await h.controller.refresh(); assert.equal(h.controller.snapshot.current.chapter, '😀'.repeat(32)); h.controller.dispose();
});

for (const publicMode of [false, true]) for (const state of ['ready', 'boundary', 'excluded-current', 'stale-anchor']) test((publicMode ? 'public' : 'native') + ': ' + state + ' rejects changed actual response identity before rows, boundary or history', async () => {
  const f = await fixture(publicMode); f.current = undefined;
  f.responder = url => f.envelope(url, { catalogIdentity: { ...f.catalogIdentity, serviceKey: 'b'.repeat(64) }, state, restartRequired: state === 'stale-anchor', data: state === 'ready' ? [row(publicMode, 1)] : [], nextCursor: null });
  await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'restart'); assert.equal(f.controller.snapshot.requiresReload, true); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 1); assert.equal(f.controller.snapshot.groups.length, 0); f.controller.dispose();
});
test('a changed response namespace during navigation cannot turn into a chapter or title boundary', async () => {
  const f = await fixture(); await f.controller.refresh(); let boundary = 0;
  f.responder = url => f.envelope(url, { catalogIdentity: { ...f.catalogIdentity, catalogNamespace: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb' }, state: 'boundary', data: [] });
  await f.controller.semantic('next', false, async () => { boundary++; }); assert.equal(boundary, 0); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.requiresReload, true); f.controller.dispose();
});
test('missing or malformed identity is not accepted from an older selector server', async () => {
  for (const identity of [undefined, {}, { catalogNamespace: 'a'.repeat(32), serviceKey: null }, { catalogNamespace: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serviceKey: 'BAD' }]) {
    const f = await fixture(); f.responder = url => f.envelope(url, { catalogIdentity: identity }); await f.controller.refresh(); assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.controller.snapshot.current, undefined); f.controller.dispose();
  }
});
for (const publicMode of [false, true]) test((publicMode ? 'public' : 'native configured API') + ': capability and around share actual bounded transport and one action budget', async () => {
  const endpoint = { origin: publicMode ? 'https://reader.test' : 'https://native-api.test', base: publicMode ? '/api/v1' : '/configured/api', profile: publicMode ? 'public-retained-v1' : 'native-aggregate-v1' }, f = await fixture(publicMode, endpoint); let ownedBudget;
  f.historyStatus = 'unavailable'; // Ordinary native/public navigation needs no IndexedDB.
  f.responder = url => url.pathname.endsWith('/protocol/capabilities') ? { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: f.catalogIdentity.serviceKey } : f.envelope(url);
  await f.controller.prepareIdentity(async budget => { ownedBudget = budget; const identity = await f.pinned.verifyIdentity(budget); return () => { f.scopeEpoch = JSON.stringify(identity); }; }, true);
  assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.calls.length, 2); assert.equal(ownedBudget.requests, 2); assert(ownedBudget.bytes < 65536 + 256 * 1024);
  assert.equal(f.calls[0].url.href, endpoint.origin + (publicMode ? '/api' : endpoint.base) + '/protocol/capabilities'); assert.equal(f.calls[0].init.credentials, 'omit');
  assert.equal(f.calls[1].url.pathname, endpoint.base + '/manga/' + f.destination.mangaId + '/reader'); assert.equal(f.calls[1].init.credentials, publicMode ? 'omit' : 'same-origin'); f.controller.dispose();
});
test('actual source endpoint resolution preserves empty native root, configured origin and fixed public route', async () => {
  const f = await fixture(), { readerEndpoint, identityChanged } = await f.module('platform-integrations/bounded-reader/endpoint.ts'), page = 'https://reader.test/chapter/id/1';
  const native = readerEndpoint(false, 'https://native-api.test/custom/api/', page), publicEndpoint = readerEndpoint(true, 'https://native-api.test/custom/api/', page);
  assert.deepEqual(JSON.parse(JSON.stringify(native)), { origin: 'https://native-api.test', base: '/custom/api', profile: 'native-aggregate-v1' }); assert.equal(readerEndpoint(false, '', page).base, ''); assert.equal(publicEndpoint.base, '/api/v1'); assert.equal(publicEndpoint.origin, 'https://reader.test');
  const verified = { origin: publicEndpoint.origin, ...f.catalogIdentity }, changed = { url: 'https://reader.test/api/protocol/capabilities', scope: { ...verified, catalogNamespace: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb' } };
  assert.equal(identityChanged(publicEndpoint, verified, changed), true); assert.equal(identityChanged(publicEndpoint, verified, { ...changed, url: 'https://reader.test/other/protocol/capabilities' }), false); assert.equal(identityChanged(publicEndpoint, verified, undefined), false); f.controller.dispose();
});
test('actual retained public comparator goldens arrive unchanged through real current groups controller', async () => {
  const f = await fixture(true), { compareReaderChapters } = await f.module('platform-integrations/public-api/reader-order.ts'); f.current = undefined;
  for (const labels of [['2a', '1', null, 'special', '02', '2', '2.0', ''], ['9007199254740992', '9007199254740993', '2.0000000000000000000000000001', '2.0000000000000000000000000002'], ['𐀀', '\ue000', '😀', '\uffff'], ['2\n', '1']]) {
    const expected = labels.map((label, i) => row(true, i + 1, label)).sort(compareReaderChapters); f.responder = url => f.envelope(url, { data: expected }); await f.controller.groups();
    assert.deepEqual(Array.from(f.controller.snapshot.groups, row => row.chapter), expected.map(row => row.chapter)); assert.equal(f.calls.at(-1).url.searchParams.get('direction'), 'newest');
  } f.controller.dispose();
});
test('native numeric-key promotion and public UUID insertion goldens remain distinct actual edition pages', async () => {
  for (const publicMode of [false, true]) {
    const f = await fixture(publicMode); f.current = undefined;
    const keys = publicMode ? ['ffffffff-ffff-8fff-8fff-ffffffffffff', '00000000-0000-8000-8000-000000000001'] : Object.keys(JSON.parse('{"02":0,"10":0,"2":0,"4294967294":0,"4294967295":0}'));
    const editions = keys.map((key, i) => row(publicMode, i + 1, '1', { id: key })), anchor = editions[0]; f.responder = url => f.envelope(url, { data: editions, groupAnchor: anchor, winner: anchor, hasMultipleEditions: true });
    await f.controller.editions(anchor); assert.deepEqual(Array.from(f.controller.snapshot.editions, row => row.id), keys); assert.equal(f.controller.snapshot.winner.id, keys[0]); f.controller.dispose();
  }
});

for (const failureStage of ['representative', 'edition']) test('failed ' + failureStage + ' history after completed group retries without skipping or drift fabrication', async () => {
  const f = await fixture(); f.current = undefined; const a = row(false, 1), b = row(false, 2), edition = row(false, 4, '2'); let failed = false; const readKeys = [];
  f.completionRead = ids => { readKeys.push([...ids]); if (ids.includes(a.id)) return true; if (!failed && ids.includes(failureStage === 'representative' ? b.id : edition.id)) { failed = true; throw Error('Transient history failure'); } return false; };
  f.responder = url => f.envelope(url, url.searchParams.get('operation') === 'groups' ? { data: [a, b] } : { data: [edition], groupAnchor: b, winner: b, hasMultipleEditions: true });
  await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.controller.snapshot.candidate, undefined); assert.equal(f.navigated.length, 0);
  await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'candidate'); assert.equal(f.controller.snapshot.candidate.id, b.id); assert.equal(readKeys.filter(ids => ids.includes(failureStage === 'representative' ? b.id : edition.id)).length, 2); f.controller.dispose();
});
for (const publicMode of [false, true]) test((publicMode ? 'public' : 'native') + ': actual title query overrides preferences before exact profile translation', async () => {
  const f = await fixture(publicMode), { titleReaderFilters } = await f.module('platform-integrations/bounded-reader/title-query.ts'); f.current = undefined;
  const group1 = publicMode ? id(true, 41) : 'group:one', group2 = publicMode ? id(true, 42) : 'group:two';
  const query = titleReaderFilters(q => { q.append('tlang', 'en'); q.append('group', 'preferred'); if (!publicMode) q.append('tlangx', 'de'); }, new URLSearchParams([['tlang', 'ja'], ['tlang', 'fr'], ['group', group1], ['group', group2], ['offset', '999999']]));
  f.destination.filters = f.client.readerFilters(f.destination.profile, query); await f.controller.start();
  const sent = f.calls[0].url.searchParams; assert.deepEqual(sent.getAll(publicMode ? 'translatedLanguage[]' : 'tlang'), ['ja', 'fr']); assert.deepEqual(sent.getAll(publicMode ? 'groups[]' : 'group'), [group1, group2]); assert.equal(sent.has('offset'), false); f.controller.dispose();
});

test('cancel during next-group history discards the scan before late completion and retry', async () => {
  const f = await fixture(); f.current = undefined; const a = row(false, 1), b = row(false, 2), gate = deferred(); let held = true; const keys = [];
  f.completionRead = ids => { keys.push([...ids]); return ids.includes(a.id) ? true : held ? gate.promise : false; };
  f.responder = url => f.envelope(url, url.searchParams.get('operation') === 'groups' ? { data: [a, b] } : { data: [b], groupAnchor: b, winner: b, hasMultipleEditions: false });
  const work = f.controller.continueScan(); await turns(); f.controller.cancel(); held = false; gate.resolve(true); await work;
  assert.equal(f.controller.snapshot.candidate, undefined); await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'candidate'); assert.equal(f.controller.snapshot.candidate.id, b.id); assert.equal(keys.filter(ids => ids.includes(a.id)).length, 2); assert.equal(f.navigated.length, 0); f.controller.dispose();
});

test('unsupported public exclusion fails closed instead of silently dropping the preference', async () => { const f = await fixture(true); assert.throws(() => f.client.readerFilters('public-retained-v1', new URLSearchParams({ tlangx: 'de' })), /not dropped/); assert.equal(f.calls.length, 0); f.controller.dispose(); });

for (const settlement of ['resolved', 'rejected', 'changed-epoch']) test('held router navigation stays owned and cannot claim cancellation: ' + settlement, async () => {
  const f = await fixture(); await f.controller.refresh(); const gate = deferred(); f.navigateHook = () => gate.promise;
  const action = f.controller.semantic('next'); await turns(); assert.equal(f.controller.snapshot.routing, true); assert.equal(f.controller.snapshot.pending, true);
  f.controller.cancel(); assert.match(f.controller.snapshot.message, /cannot be cancelled here/); await f.controller.semantic('next'); assert.equal(f.navigated.length, 1); assert.equal(f.calls.length, 2);
  f.expire(10000); assert.equal(f.controller.snapshot.pending, true); assert.equal(f.timers.size, 0); // Preparation timer ended at handoff, not a fake router timeout.
  if (settlement === 'changed-epoch') f.changeEpoch(); if (settlement === 'rejected') gate.reject(Error('Router rejected navigation')); else gate.resolve(); await action;
  assert.equal(f.controller.snapshot.pending, false); assert.equal(f.controller.snapshot.routing, false); assert.equal(f.controller.snapshot.status, settlement === 'resolved' ? 'ready' : settlement === 'rejected' ? 'error' : 'restart');
  if (settlement === 'changed-epoch') assert.equal(f.controller.snapshot.current, undefined); assert.equal(f.navigated.length, 1); f.controller.dispose();
});

test('native Continue pauses before an unfunded lookup and resumes the same uninspected group', async () => {
  const f = await fixture(), inspected = []; f.current = undefined; f.historyStatus = 'available-native';
  const groups = Array.from({ length: 12 }, (_, index) => row(false, index + 1));
  f.responder = url => ({ ...f.envelope(url), data: groups, nextCursor: null });
  f.completionRead = (ids, budget) => { budget.request(); budget.consume(16 * 1024); inspected.push(...ids); return true; };
  await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'paused'); assert.deepEqual(inspected, groups.slice(0, 7).map(group => group.id));
  await f.controller.continueScan(); assert.deepEqual(inspected, groups.map(group => group.id)); assert.equal(f.controller.snapshot.status, 'observed-end'); assert.equal(f.navigated.length, 0); f.controller.dispose();
});
test('native Continue reserves selector plus completion and keeps an inspected unread anchor on pause', async () => {
  const f = await fixture(), inspected = [], groups = Array.from({ length: 7 }, (_, index) => row(false, index + 1)); f.current = undefined; f.historyStatus = 'available-native';
  f.responder = url => url.searchParams.get('operation') === 'groups' ? { ...f.envelope(url), data: groups, nextCursor: null } : { ...f.envelope(url), data: [groups[6]], groupAnchor: groups[6], winner: groups[6], hasMultipleEditions: false, nextCursor: null };
  f.completionRead = (ids, budget) => { budget.request(); budget.consume(16 * 1024); inspected.push([...ids]); return ids[0] !== groups[6].id; };
  await f.controller.continueScan(); assert.equal(f.controller.snapshot.status, 'paused'); assert.equal(f.calls.length, 1); assert.equal(inspected.length, 7);
  await f.controller.continueScan(); assert.equal(f.calls.length, 2); assert.equal(f.calls[1].url.searchParams.get('groupChapter'), groups[6].id); assert.equal(inspected.length, 8); assert.equal(f.controller.snapshot.candidate.id, groups[6].id); assert.equal(f.navigated.length, 0); f.controller.dispose();
});

// Empty title controls do not contain stale chapter choices. Ownership must
// still change, and genuine prepared/in-flight selections must remain fenced.
test('idle title context changes stay quiet; prepared title choices still require restart', async () => {
  const f = await fixture(); f.current = undefined; await f.controller.refresh();
  f.changeEpoch(); assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.controller.snapshot.message, '');
  assert.equal(f.calls.length, 0);
  await f.controller.groups(); f.changeEpoch(); assert.equal(f.controller.snapshot.status, 'restart');
  assert.equal(f.controller.snapshot.groups.length, 0); f.controller.dispose();
});
test('pending title selection invalidates rather than becoming quietly ready', async () => {
  const f = await fixture(), gate = deferred(); f.current = undefined;
  f.responder = async url => { const value = f.envelope(url); await gate.promise; return value; };
  const action = f.controller.start(); await turns(); f.changeEpoch();
  assert.equal(f.controller.snapshot.status, 'restart'); gate.resolve(); await action;
  assert.equal(f.navigated.length, 0); f.controller.dispose();
});
