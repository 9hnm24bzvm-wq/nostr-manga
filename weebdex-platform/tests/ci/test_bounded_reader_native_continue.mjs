// Source-authored, unexecuted. Actual native Continue adapter/controller/DTO and
// transport; server selection and authority observations are synthetic. No SQL,
// cookie, full-history badge, browser, or exact device >10K claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { nativeContinueFixture } from '../fixtures/bounded-reader/native-continue-fixture.mjs';
import { fixture, row, deferred, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
const native = call => call.url.pathname.endsWith('/continue');
const close = f => { f.controller.dispose(); assert.equal(f.timers.size, 0); };
test('actual native automatic Continue reads no device/badge history and sends only exact native query witnesses', async () => {
  const f = await nativeContinueFixture(); f.filters = 'group[]=g:2&group=g:1&group=g:2&tlang=en&tlang=en';
  await f.controller.continueNative(); assert.equal(f.navigated.length, 1); assert.equal(f.navigated[0].target.id, 'chapter1'); assert.equal(f.navigated[0].lastPage, false);
  assert.equal(f.calls.length, 2); const requests = new Set(); for (const call of f.calls) {
    assert.equal(call.url.origin + call.url.pathname, 'https://native-api.test/custom/api/user/history/manga/manga%3Aone/continue');
    assert.deepEqual(call.url.searchParams.getAll('group'), ['g:1', 'g:2']); assert.deepEqual(call.url.searchParams.getAll('tlang'), ['en']);
    assert.deepEqual([...new Set(call.url.searchParams.keys())].sort(), ['accountEpoch', 'catalogNamespace', 'group', 'requestId', 'serviceKey', 'tlang']);
    assert.equal(call.options.credentials, 'include'); assert.equal(call.options.redirect, 'error'); assert.equal(call.options.headers['X-WeebDex-Expected-Account'], 'actor:one');
    assert.deepEqual(Object.keys(call.options.headers).sort(), ['Accept', 'X-WeebDex-Expected-Account']); assert.equal(call.options.body, undefined);
    assert.match(call.url.searchParams.get('requestId'), /^[A-Za-z0-9_-]{1,64}$/); requests.add(call.url.searchParams.get('requestId'));
    assert(!JSON.stringify([call.url.href, call.options.headers]).includes('NEVER_UPLOAD_LOCAL_HISTORY'));
  }
  assert.equal(requests.size, 2); close(f);
});
for (const reason of ['oldest-unread', 'all-completed-newest']) test('actual controller directly opens compact server-selected ' + reason + ' winner beyond 10K', async () => {
  const f = await nativeContinueFixture(); f.selection = { winner: row(false, 10001, null, { volume: null }), reason, hasMultipleEditions: false };
  await f.controller.continueNative(); assert.equal(f.calls.length, 2); assert.equal(f.navigated[0].target.id, 'chapter10001'); assert.equal(f.navigated[0].target.chapter, null);
  assert(f.calls.every(native)); assert(f.calls.every(call => !call.url.searchParams.has('limit') && !call.url.searchParams.has('offset'))); close(f);
});
test('empty native Continue is an explicit boundary, with no newest/first fallback', async () => {
  const f = await nativeContinueFixture(); f.selection = { winner: null, reason: 'empty', hasMultipleEditions: false };
  await f.controller.continueNative(); assert.equal(f.controller.snapshot.status, 'boundary'); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 2); close(f);
});
for (const fault of ['reason', 'winner', 'raw-label', 'multiplicity', 'empty']) test('changed original Continue ' + fault + ' cannot silently choose a replacement', async () => {
  const f = await nativeContinueFixture(); f.responder = call => { const body = f.envelope(call); if (f.calls.length === 2) {
    if (fault === 'reason') body.reason = 'all-completed-newest'; else if (fault === 'multiplicity') body.hasMultipleEditions = true;
    else if (fault === 'empty') return f.envelope(call, { winner: null }); else { const winner = fault === 'winner' ? row(false, 2) : row(false, 1, '01'); body.winner = winner; body.target = { volume: winner.volume, chapter: winner.chapter, anchorChapterId: winner.id }; }
  } return body; };
  await f.controller.continueNative(); assert.equal(f.controller.snapshot.status, 'restart'); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 2); close(f);
});
for (const fault of ['namespace', 'service', 'actor', 'epoch', 'request-id', 'scope', 'profile', 'parent', 'proof', 'cross-request', 'target', 'extra-field', 'version-zero', 'id-final-newline', 'group-id-final-newline']) test('strict native Continue DTO rejects ' + fault, async () => {
  const f = await nativeContinueFixture(); f.responder = call => { const body = f.envelope(call);
    if (fault === 'namespace') body.catalogIdentity.catalogNamespace = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; else if (fault === 'service') body.catalogIdentity.serviceKey = 'a'.repeat(64);
    else if (fault === 'actor') body.account.id = 'actor:other'; else if (fault === 'epoch') body.account.epoch++; else if (fault === 'request-id') body.requestId = 'other'; else if (fault === 'scope') body.scope = '0'.repeat(64);
    else if (fault === 'profile') body.orderingProfile = 'public-retained-v1'; else if (fault === 'parent') body.mangaId = 'other'; else if (fault === 'proof') body.continueProof = 'observed-only';
    else if (fault === 'cross-request') body.crossRequestSnapshot = true; else if (fault === 'target') body.target.chapter = '01'; else if (fault === 'extra-field') body.privateHistory = []; else if (fault === 'id-final-newline') { body.winner.id += '\n'; body.target.anchorChapterId = body.winner.id; } else if (fault === 'group-id-final-newline') body.winner.groupIds.push('group:one\n'); else body.winner.version = 0; return body; };
  await f.controller.continueNative(); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 1); assert(['error', 'restart'].includes(f.controller.snapshot.status)); close(f);
});
for (const fault of ['namespace', 'actor', 'conflict']) test('native Continue ' + fault + ' latches reload through Start, retry, cancel and context changes', async () => {
  const f = await nativeContinueFixture(); f.responder = call => { if (fault === 'conflict') return new Response('', { status: 409 }); const body = f.envelope(call); if (fault === 'namespace') body.catalogIdentity.catalogNamespace = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; else body.account.id = 'actor:other'; return body; };
  await f.controller.continueNative(); assert.equal(f.controller.snapshot.requiresReload, true); f.responder = undefined;
  await f.controller.start(); await f.controller.continueNative(); f.controller.cancel(); f.source.selectionEpoch += 2; f.invalidate(); await f.controller.start(); await f.controller.continueNative();
  assert.equal(f.calls.length, 1); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.requiresReload, true); if (fault === 'conflict') assert.match(f.controller.snapshot.message, /changed or is busy/); close(f);
});
for (const change of ['account', 'epoch', 'selection', 'revision', 'context', 'mode', 'pending', 'endpoint', 'action']) test('native Continue late ' + change + ' change cannot publish or navigate', async () => {
  const f = await nativeContinueFixture(), gate = deferred(); f.responder = async call => { const body = f.envelope(call); await gate.promise; return body; };
  const work = f.controller.continueNative(); await f.until(() => f.calls.length === 1);
  if (change === 'account') f.authority.actor = 'actor:other'; else if (change === 'epoch') f.authority.epoch++; else if (change === 'selection') f.source.selectionEpoch += 2;
  else if (change === 'revision') f.source.accountRevision++; else if (change === 'context') f.source.context = { ...f.source.context, generation: 3 }; else if (change === 'mode') f.source.mode = 'device';
  else if (change === 'pending') f.source.selectionPending = true; else if (change === 'endpoint') f.endpoint = { ...f.endpoint, base: '/changed' }; else f.invalidate();
  gate.resolve(); await work; assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 1); assert.equal(f.controller.snapshot.winner, undefined); close(f);
});
for (const unavailable of ['public', 'device', 'pending', 'wrong-actor']) test(unavailable + ' mode never dispatches native Continue cookies', async () => {
  const f = await nativeContinueFixture(); if (unavailable === 'public') f.endpoint = { origin: 'https://reader.test', base: '/api/v1', profile: 'public-retained-v1' }; else if (unavailable === 'wrong-actor') f.source.context.accountId = 'actor:other'; else if (unavailable === 'pending') f.source.selectionPending = true; else f.source.mode = 'device';
  await f.controller.continueNative(); assert.equal(f.calls.length, 0); assert.equal(f.navigated.length, 0); close(f);
});
for (const status of [401, 403, 429, 500]) test('HTTP ' + status + ' is not empty/all-read and never falls back to device history', async () => {
  const f = await nativeContinueFixture(); f.responder = () => new Response('', { status }); await f.controller.continueNative(); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 1); assert.equal(f.controller.snapshot.status, 'error'); close(f);
});
test('unobserved cookie account swap is rejected before a target is accepted', async () => {
  const f = await nativeContinueFixture(); f.cookieActor = 'actor:other'; await f.controller.continueNative(); assert.equal(f.controller.snapshot.requiresReload, true); assert.equal(f.calls.length, 1); assert.equal(f.navigated.length, 0); close(f);
});
async function chooser() {
  const f = await nativeContinueFixture(); f.selection = { winner: row(false, 10001, null, { volume: null }), reason: 'all-completed-newest', hasMultipleEditions: true };
  const edition = row(false, 2, null, { volume: null }); f.responder = call => native(call) ? f.envelope(call) : f.selector(call, { data: [edition], nextCursor: 'page-two' });
  await f.controller.continueNative(); assert.equal(f.calls.length, 3); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.editions[0].id, edition.id); assert.equal(f.controller.snapshot.winner.id, 'chapter10001'); return { f, edition };
}
test('native multi-edition chooser keeps full-set winner independent of its bounded page and rechecks before paging', async () => {
  const { f } = await chooser(); f.responder = call => native(call) ? f.envelope(call) : f.selector(call, { data: [row(false, 3, null, { volume: null })] });
  await f.controller.editions(undefined, true); assert.equal(f.calls.length, 5); assert(native(f.calls[3])); assert.equal(f.calls[4].url.searchParams.get('after'), 'page-two'); assert.equal(f.controller.snapshot.editions[0].id, 'chapter3'); assert.equal(f.controller.snapshot.winner.id, 'chapter10001'); close(f);
});
for (const drift of ['before-page', 'page-winner', 'before-choice']) test('native chooser refuses ' + drift + ' drift without another-group fallback', async () => {
  const { f, edition } = await chooser(); f.responder = call => native(call) ? f.envelope(call, drift === 'page-winner' ? {} : { winner: row(false, 4) }) : f.selector(call, { winner: edition, data: [row(false, 3, null, { volume: null })] });
  if (drift === 'before-choice') await f.controller.chooseEdition(edition); else await f.controller.editions(undefined, true);
  assert.equal(f.controller.snapshot.status, 'restart'); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.groupAnchor, undefined); assert.equal(f.calls.length, drift === 'page-winner' ? 5 : 4); close(f);
});
test('native edition handoff rechecks original semantics then exact singleton eligibility, without widening ID or exclusion filters', async () => {
  const f = await nativeContinueFixture(), chosen = row(false, 2); f.filters = 'id=chapter1&id[]=chapter2&tlang=en&tlangx=ja&group=g:1&groupx=g:2&uploaderx=u:9';
  f.selection = { winner: row(false, 1, '2'), reason: 'oldest-unread', hasMultipleEditions: true };
  f.responder = call => native(call) ? f.envelope(call) : f.selector(call, { data: [chosen] }); await f.controller.continueNative();
  f.responder = call => call.url.searchParams.getAll('id').length === 1 ? f.envelope(call, { winner: chosen, hasMultipleEditions: false, reason: 'all-completed-newest' }) : f.envelope(call);
  await f.controller.chooseEdition(chosen); assert.equal(f.navigated.length, 1); assert.equal(f.navigated[0].target.id, chosen.id); assert.equal(f.calls.length, 5);
  assert.deepEqual(f.calls[3].url.searchParams.getAll('id'), ['chapter1', 'chapter2']); assert.deepEqual(f.calls[4].url.searchParams.getAll('id'), ['chapter2']);
  for (const key of ['tlang', 'tlangx', 'group', 'groupx', 'uploaderx']) assert.deepEqual(f.calls[4].url.searchParams.getAll(key), f.calls[3].url.searchParams.getAll(key));
  assert.notEqual(f.canonicalScope(f.calls[3]), f.canonicalScope(f.calls[4])); close(f);
});
test('selected edition outside the original OR ID set cannot be added as another accepted ID', async () => {
  const { f, edition } = await chooser(); f.filters = 'id=chapter10001'; f.controller.sync(); f.responder = call => native(call) ? f.envelope(call) : f.selector(call, { data: [edition] }); await f.controller.continueNative(); const count = f.calls.length;
  await f.controller.chooseEdition(edition); assert.equal(f.calls.length, count + 1); assert.equal(f.navigated.length, 0); assert.match(f.controller.snapshot.message, /outside the original ID filter/); close(f);
});
for (const fault of ['boundary', 'wrong-edition', 'changed-witness', 'changed-group', 'multiple']) test('singleton Continue ' + fault + ' cannot authorize chooser navigation', async () => {
  const { f, edition } = await chooser(); f.responder = call => { if (!call.url.searchParams.has('id')) return f.envelope(call); return f.envelope(call, { winner: fault === 'boundary' ? null : fault === 'wrong-edition' ? row(false, 9, null, { volume: null }) : { ...edition, ...(fault === 'changed-witness' ? { witness: 'f'.repeat(64) } : fault === 'changed-group' ? { chapter: '' } : {}) }, hasMultipleEditions: fault === 'multiple' }); };
  await f.controller.chooseEdition(edition); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 5); assert(['restart', 'error'].includes(f.controller.snapshot.status)); close(f);
});
test('32 KiB Continue envelope accepts a 20 KiB body without widening the 16 KiB completion limit', async () => {
  const f = await nativeContinueFixture(); f.responder = call => new Response(JSON.stringify(f.envelope(call)).padEnd(20 * 1024, ' '), { headers: { 'content-type': 'application/json' } });
  await f.controller.continueNative(); assert.equal(f.navigated.length, 1); const transport = await f.module('platform-integrations/public-api/owned-json.ts');
  assert.equal(transport.NATIVE_CONTINUE_LIMITS.bytes, 32768); assert.equal(transport.NATIVE_COMPLETION_LIMITS.bytes, 16384);
  const read = transport.createNativeCompletionJsonReader(() => f.endpoint); await assert.rejects(read({ mangaId: 'manga:one', ids: ['chapter1'], accountId: f.authority.actor, accountEpoch: 7, ...f.identity }, { assertCurrent() {} }), /budget|size|large|byte|limit/i); close(f);
});
test('Continue byte overflow refuses rather than parsing a response beyond 32 KiB', async () => {
  const f = await nativeContinueFixture(); f.responder = call => new Response(JSON.stringify(f.envelope(call)).padEnd(32769, ' '), { headers: { 'content-type': 'application/json' } });
  await f.controller.continueNative(); assert.equal(f.calls.length, 1); assert.equal(f.navigated.length, 0); assert.equal(f.controller.snapshot.status, 'error'); close(f);
});
test('cancelled ignored fetch retains the native slot across Continue remount and completion until actual settlement', async () => {
  const f = await nativeContinueFixture(), gate = deferred(); f.responder = async call => { const value = f.envelope(call); await gate.promise; return value; };
  const work = f.controller.continueNative(); await f.until(() => f.calls.length === 1); f.controller.cancel(); await turns(); f.expire(250); await work;
  const b = f.budget(); await assert.rejects(f.remountAccess().read('tlang=en', b), /cleanup|settle/i); b.close();
  const transport = await f.module('platform-integrations/public-api/owned-json.ts'); await assert.rejects(transport.createNativeCompletionJsonReader(() => f.endpoint)({ mangaId: 'manga:one', ids: ['chapter1'], accountId: f.authority.actor, accountEpoch: 7, ...f.identity }, { assertCurrent() {} }), /cleanup|settle/i);
  assert.equal(f.calls.length, 1); gate.resolve(); await turns(); f.responder = undefined; const fresh = f.budget(); await f.remountAccess().read('tlang=en', fresh); fresh.close(); assert.equal(f.calls.length, 2); assert.equal(f.navigated.length, 0); close(f);
});
test('uncancellable scope digest has finite caller cancellation and shared ownership through remount without unhandled rejection', async () => {
  const f = await nativeContinueFixture(), gate = deferred(), unhandled = []; const report = value => unhandled.push(value); process.on('unhandledRejection', report);
  try {
    f.digestHook = () => gate.promise; const b = f.budget(), work = f.access().read('tlang=en', b), rejected = assert.rejects(work, /cancel/i); await f.until(() => f.digests === 1); b.cancel(); await rejected; b.close();
    const retry = f.budget(); await assert.rejects(f.remountAccess().read('tlang=en', retry), /not settled/); retry.close(); assert.equal(f.digests, 1); assert.equal(f.calls.length, 0);
    gate.reject(Error('Late native digest rejection')); await turns(); await new Promise(resolve => setImmediate(resolve)); assert.equal(unhandled.length, 0); f.digestHook = undefined;
    const fresh = f.budget(); await f.remountAccess().read('tlang=en', fresh); fresh.close(); assert.equal(f.calls.length, 1); close(f);
  } finally { process.removeListener('unhandledRejection', report); }
});
test('canonical scope matches the actual admitted query sets and excludes only epoch/request correlation', async () => {
  const f = await nativeContinueFixture(), contract = await f.module('platform-integrations/bounded-reader/native-continue-contract.ts');
  const input = { mangaId: 'manga:one', accountId: 'actor:one', accountEpoch: 7, catalogNamespace: f.identity.catalogNamespace, serviceKey: null, requestId: 'one', filters: 'group[]=g:2&tlang=en&group=g:1&group=g:2' };
  const expected = createHash('sha256').update(JSON.stringify([{ catalogNamespace: input.catalogNamespace, serviceKey: null }, 'native-aggregate-v1', 'native-continue', 'manga:one', 'actor:one', { group: ['g:1', 'g:2'], tlang: ['en'] }])).digest('hex');
  assert.equal(await contract.nativeContinueScope(input, () => {}), expected); assert.equal(await contract.nativeContinueScope({ ...input, accountEpoch: 8, requestId: 'two' }, () => {}), expected);
  assert.notEqual(await contract.nativeContinueScope({ ...input, accountId: 'actor:two' }, () => {}), expected); close(f);
});
for (const outcome of ['held', 'rejected', 'changed-context']) test('native navigation ' + outcome + ' keeps router ownership and cannot report false cancellation', async () => {
  const f = await nativeContinueFixture(), gate = deferred(); f.navigateHook = () => gate.promise; const work = f.controller.continueNative(); await f.until(() => f.navigated.length === 1);
  assert.equal(f.controller.snapshot.routing, true); assert.equal(f.timers.size, 0); f.controller.cancel(); await f.controller.continueNative(); assert.equal(f.calls.length, 2); assert.equal(f.navigated.length, 1);
  if (outcome === 'changed-context') f.invalidate(); if (outcome === 'rejected') gate.reject(Error('Router rejected')); else gate.resolve(); await work;
  assert.equal(f.controller.snapshot.pending, false); if (outcome === 'changed-context') assert.equal(f.controller.snapshot.status, 'restart'); close(f);
});

test('native Continue reserves the whole response envelope before hashing or dispatch', async () => {
  const f = await nativeContinueFixture(), client = await f.module('platform-integrations/bounded-reader/client.ts'), b = f.budget(); b.bytes = client.READER_LIMITS.actionBytes - 32767;
  await assert.rejects(f.access().read('tlang=en', b), /budget/); assert.equal(f.digests, 0); assert.equal(f.calls.length, 0); b.close(); close(f);
});
test('native Continue disposal during an owned read cannot publish or navigate after its late response', async () => {
  const f = await nativeContinueFixture(), gate = deferred(); f.responder = async call => { const body = f.envelope(call); await gate.promise; return body; };
  const work = f.controller.continueNative(); await f.until(() => f.calls.length === 1); f.controller.dispose(); gate.resolve(); await work; assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 1); assert.equal(f.controller.snapshot.winner, undefined); assert.equal(f.timers.size, 0);
});
test('native Continue preparation deadline cancels a hung digest but retains its shared slot until actual settlement', async () => {
  const f = await nativeContinueFixture(), gate = deferred(); f.digestHook = () => gate.promise; const work = f.controller.continueNative(); await f.until(() => f.digests === 1); f.expire(10000); await work;
  assert.equal(f.controller.snapshot.pending, false); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 0); const b = f.budget(); await assert.rejects(f.remountAccess().read('tlang=en', b), /not settled/); b.close(); gate.resolve(new ArrayBuffer(32)); await turns(); close(f);
});
for (const filters of ['id=chapter1%0A', 'tlang=en%0A', 'completed=chapter1', 'history=private', 'order=asc', 'tlang[]=en&tlang=ja&unknown=x', 'id=' + 'x'.repeat(129), Array.from({ length: 101 }, (_, index) => 'id=c' + index).join('&')]) test('native Continue rejects unadmitted query shape ' + filters.slice(0, 32), async () => {
  const f = await nativeContinueFixture(), b = f.budget(); await assert.rejects(f.access().read(filters, b)); assert.equal(f.calls.length, 0); assert.equal(f.digests, 0); b.close(); close(f);
});
for (const field of ['requestId', 'mangaId', 'accountId', 'catalogNamespace', 'serviceKey']) test('native Continue rejects a trailing newline in exact ' + field + ' syntax before dispatch', async () => {
  const f = await nativeContinueFixture(), transport = await f.module('platform-integrations/public-api/owned-json.ts');
  const input = { mangaId: 'manga:one', accountId: 'actor:one', accountEpoch: 7, catalogNamespace: f.identity.catalogNamespace, serviceKey: 'a'.repeat(64), requestId: 'one', filters: '' }; input[field] += '\n';
  await assert.rejects(transport.createNativeContinueJsonReader(() => f.endpoint)(input, { assertCurrent() {} })); assert.equal(f.calls.length, 0); close(f);
});

for (const publicMode of [false, true]) test((publicMode ? 'public' : 'native') + ': unchanged minimal validator accepts valid JSON and refuses foreign-prototype/newline rows', async () => {
  const f = await fixture(publicMode), contract = await f.module('platform-integrations/bounded-reader/contract.ts'), profile = publicMode ? 'public-retained-v1' : 'native-aggregate-v1';
  const valid = row(publicMode, 1, '1', publicMode ? {} : { id: 'chapter:valid', groupIds: ['group:valid'] });
  // A direct host object is deliberately not a plain object in the VM realm.
  // Every positive/newline response below uses actual owned bytes + JSON.parse.
  assert.throws(() => contract.chapterRow(valid, profile)); f.current = undefined; let candidate = valid;
  f.responder = url => f.envelope(url, { data: [candidate] });
  await f.controller.groups(); assert.equal(f.controller.snapshot.status, 'ready'); assert.equal(f.controller.snapshot.groups[0].id, valid.id);
  const intent = f.calls[0].url.href; assert.equal(f.calls[0].url.searchParams.has('current'), false); assert.equal(f.calls[0].url.searchParams.get('operation'), 'groups');
  let requests = 1;
  for (const suffix of ['\n', '\r\n']) for (const field of ['id', 'groupIds']) {
    candidate = field === 'id' ? { ...valid, id: valid.id + suffix } : { ...valid, groupIds: [valid.id + suffix] };
    await f.controller.groups(); assert.equal(f.calls.length, ++requests); assert.equal(f.calls.at(-1).url.href, intent);
    assert.equal(f.controller.snapshot.status, 'error'); assert(f.controller.snapshot.groups.every(value => value.id === valid.id)); assert.equal(f.navigated.length, 0);
  }
  close(f);
});
