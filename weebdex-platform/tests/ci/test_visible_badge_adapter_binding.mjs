// AUTHORED, UNEXECUTED. Real retained adapter/client/query/normalizers, controlled HTTP seam.
import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { createPublicApiModuleLoader } from './vm-module-fixture.mjs';
const title = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', chapter = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', group = 'cccccccc-cccc-4ccc-8ccc-cccccccccccc';
const witness = Object.freeze({ version: 1, mangaId: title, idProfile: 'public-uuid-v1', consistency: 'response-snapshot-v1', catalogIdentity: { catalogNamespace: 'dddddddd-dddd-4ddd-8ddd-dddddddddddd', serviceKey: null } });
const collection = (data, extra = {}) => ({ result: 'ok', response: 'collection', limit: 100, offset: 0, total: data.length, data, ...extra });
for (const grouped of [false, true]) for (const binding of [witness, undefined, { malformed: true }]) test(`adapter preserves but never synthesizes binding: grouped=${grouped}, bound=${binding !== undefined}`, async () => {
  const context = vm.createContext({ URL, URLSearchParams, AbortController, TextEncoder, TextDecoder, console });
  const loader = createPublicApiModuleLoader({ context });
  const { PublicApiClient } = await loader.namespace('client');
  const { RetainedCatalogueClient } = await loader.namespace('site-client');
  const calls = [], api = new PublicApiClient(async request => {
    const url = new URL(request.path, "https://badge.fixture.invalid"); calls.push({ ...request, url });
    if (url.pathname === '/nostr/native-forum/contexts') return { status: 200, body: { result: 'ok', data: [], unavailable: request.body.resources } };
    if (url.pathname === '/manga') return { status: 200, body: collection([{ id: title, type: 'manga', attributes: { title: { en: 'Actual parent hydration' }, originalLanguage: 'ja' }, relationships: [] }], { chapterPage: { ...witness, catalogIdentity: { ...witness.catalogIdentity, catalogNamespace: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee' } } }) };
    assert.equal(url.pathname, grouped ? '/chapter' : `/manga/${title}/feed`);
    assert.equal(url.searchParams.get('limit'), '1');
    if (grouped) { assert.equal(url.searchParams.get('manga'), title); assert.deepEqual(url.searchParams.getAll('groups[]'), [group]); }
    return { status: 200, body: collection([{ id: chapter, type: 'chapter', attributes: { chapter: '1', title: 'Actual chapter', translatedLanguage: 'en' }, relationships: [{ id: title, type: 'manga' }] }], binding === undefined ? {} : { chapterPage: binding }) };
  });
  const result = await new RetainedCatalogueClient(api).read('chapter.list', { id: title, query: new URLSearchParams({ limit: '1', ...(grouped ? { group } : {}) }) });
  assert.equal(result.status, 200); assert.equal(result.body.data[0].relationships.manga.id, title);
  assert.equal(result.body.chapterPage, binding, 'Never infer identity from later parent/forum hydration');
  assert.equal(Object.hasOwn(result.body, 'chapterPage'), binding !== undefined);
  assert.equal(calls.filter(call => call.url.pathname === '/manga').length, 1);
  assert.equal(calls.filter(call => call.url.pathname === '/nostr/native-forum/contexts').length, 1);
});
