// AUTHORED, UNEXECUTED. Actual badge provider/runtime modules, controlled external
// lifecycle and I/O. Backend response binding is authored separately, unexecuted.
import test from 'node:test';
import assert from 'node:assert/strict';
import { badgeRuntimeFixture } from '../fixtures/bounded-reader/badge-runtime-fixture.mjs';
import { turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
async function fixture() {
  const f = await badgeRuntimeFixture(false, true); let rows, href = 'https://reader.test/title/' + f.mangaId;
  f.source.mode = 'account'; f.source.context = { generation: 1, mode: 'account', accountId: 'account:one' };
  f.source.admission.authority = { actor: 'account:one', epoch: 2 }; f.native.set({ actor: 'account:one', epoch: 2 });
  f.responder = url => url.href === f.capabilityUrl ? { catalog_namespace: f.catalogIdentity.catalogNamespace, service_key: null } : url.pathname.endsWith('/completion') ? { version: 1, mangaId: f.mangaId, catalogIdentity: f.catalogIdentity, account: { id: 'account:one', epoch: 2 }, consistency: 'live-request-v1', continueProof: 'observed-only', data: url.searchParams.getAll('id').map(id => ({ id, completed: true })) } : f.envelope(url);
  const badge = await f.badges({ manga: () => f.mangaId, href: () => href, chapters: () => rows });
  const body = () => ({ data: [f.chapter], chapterPage: { version: 1, mangaId: f.mangaId, idProfile: 'native-id-v1', consistency: 'response-snapshot-v1', catalogIdentity: { ...f.catalogIdentity } } });
  const publish = (request, value) => { assert.doesNotThrow(() => request.accept(value)); rows = value.data; f.effects(); };
  return { ...f, get dbReads() { return f.dbReads; }, badge, body, publish, hash() { href += '#chapters'; f.navigationStart(); } };
}
test('missing binding preserves feed admission but performs no badge history lookup', async () => {
  const f = await fixture(), request = f.badge.requestPage(); f.publish(request, { data: [f.chapter] }); await f.mount(); await turns();
  assert.equal(f.badge.state.snapshot.values, undefined); assert.match(f.badge.state.snapshot.message, /snapshot-bound/);
  assert.equal(f.calls.filter(call => call.url.pathname.endsWith('/completion')).length, 0); f.dispose();
});
test('history initialization and revisions do not reject the bound public chapter page', async () => {
  const f = await fixture(), request = f.badge.requestPage();
  f.source.ready = true; f.source.scopeStatus = 'verified'; f.source.revision++; f.source.accountRevision++;
  f.publish(request, f.body()); await f.mount(); await turns();
  assert.equal(f.badge.state.snapshot.status, 'ready'); assert.equal(f.badge.state.snapshot.values[f.chapter.id], true); assert.equal(f.dbReads, 0); f.dispose();
});
test('hash-only navigation does not orphan exact accepted chapter IDs', async () => {
  const f = await fixture(), request = f.badge.requestPage(); f.publish(request, f.body()); await f.mount(); await turns(); f.hash(); await turns();
  assert.equal(f.badge.state.snapshot.status, 'ready'); assert.equal(f.badge.state.snapshot.values[f.chapter.id], true); f.dispose();
});
for (const field of ['mangaId', 'idProfile', 'consistency', 'namespace', 'row-parent']) test('invalid same-response ' + field + ' never admits private badges', async () => {
  const f = await fixture(), request = f.badge.requestPage(), value = f.body();
  if (field === 'namespace') value.chapterPage.catalogIdentity.catalogNamespace = 'invalid';
  else if (field === 'row-parent') value.data = [{ ...f.chapter, relationships: { ...f.chapter.relationships, manga: { id: 'other' } } }];
  else value.chapterPage[field] = 'other';
  f.publish(request, value); await f.mount(); await turns(); assert.equal(f.badge.state.snapshot.values, undefined); assert.equal(f.calls.filter(call => call.url.pathname.endsWith('/completion')).length, 0); f.dispose();
});
test('an old page response cannot erase a newer accepted page', async () => {
  const f = await fixture(), old = f.badge.requestPage(), current = f.badge.requestPage(); f.publish(current, f.body()); await f.mount(); await turns();
  old.accept({ data: [] }); await turns(); assert.equal(f.badge.state.snapshot.status, 'ready'); f.dispose();
});

test('hash change followed by navigation start cannot rebase an older page request', async () => {
  const f = await fixture(), request = f.badge.requestPage(); f.hash(); f.navigationStart();
  f.publish(request, f.body()); await f.mount(); await turns();
  assert.equal(f.badge.state.snapshot.values, undefined); assert.match(f.badge.state.snapshot.message, /authority changed/);
  assert.equal(f.calls.filter(call => call.url.pathname.endsWith('/completion')).length, 0); f.dispose();
});

test('badge-only retry cannot pretend to recover missing page admission', async () => {
  const f = await fixture(); f.publish(f.badge.requestPage(), { data: [f.chapter] }); await f.mount(); await turns();
  const before = f.calls.length; await f.badge.retry(); await turns();
  assert.equal(f.calls.length, before); assert.equal(f.badge.state.snapshot.status, 'unknown');
  assert.match(f.badge.state.snapshot.message, /Reload the chapter list/); f.dispose();
});
