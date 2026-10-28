import {restoreReaderUxPredecessor} from './reviewed-reader-ux-source-successor.mjs';
// Execute the actual five-stage route and Chapter API composition in Node VM.
// Catalogue replies and native cookie/grant reads are explicit stand-ins. The
// page/layout/editor associations are exact source checks, not a DOM/browser,
// signature, PostgreSQL or original-source deletion acceptance claim.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
import { createPublicUploadOverlays } from '../../platform-integrations/public-api/upload-overlays.mjs';
import { restoreReviewedLanguageSource } from './reviewed-local-page-source-successors.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const route = 'src/routes/(app)/chapter/[id]/edit/';
const chapter = '11111111-1111-8111-8111-111111111111';
const alias = '22222222-2222-8222-8222-222222222222';
const parent = '33333333-3333-8333-8333-333333333333';
const row = { id: chapter, version: 7, title: 'Actual chapter', language: 'en', relationships: { manga: { id: parent }, groups: [] } };
const plain = value => JSON.parse(JSON.stringify(value));
function composed(relative, enabled = true) {
  const file = path.join(frontend, relative);
  let code = fs.readFileSync(file, 'utf8');
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled }), createPublicUploadOverlays(frontend, { enabled })]) code = plugin.transform.handler(code, file)?.code ?? code;
  return code;
}
async function fixture({ enabled = true, reply = { status: 200, body: row }, cookie = false, canUpdate = false, failure } = {}) {
  const calls = { public: [], cookie: 0, grants: 0, native: 0 };
  const context = vm.createContext({ Boolean, URLSearchParams });
  const dependency = (name, values) => new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context, identifier: name });
  const forbidden = () => { calls.native++; throw Error('Native HTTP fallback reached'); };
  const api = new vm.SourceTextModule(stripTypeScriptTypes(composed('src/lib/API/Chapter.ts', enabled), { mode: 'transform' }), { context });
  await api.link(name => {
    if (name === '$publicApi/site-client') return dependency(name, { publicCatalogueRead: async (operation, options) => { calls.public.push({ operation, options }); if (failure) throw failure; return reply; }, blockNativeCatalogueWrite: () => { throw Error('Explicit signed chapter editor required'); } });
    if (name === '$env/static/public') return dependency(name, { PUBLIC_API_URL: '/native' });
    if (name === '$lib') return dependency(name, { HTTP: class { constructor() { if (enabled) forbidden(); } GET() { calls.native++; return reply; } } });
    throw Error('Unexpected Chapter API import ' + name);
  });
  await api.evaluate();
  const loader = new vm.SourceTextModule(stripTypeScriptTypes(composed(route + '+page.ts', enabled), { mode: 'transform' }), { context });
  await loader.link(name => {
    if (name === '$lib') return dependency(name, { Permission: { Chapter: { canUpdate: () => { calls.grants++; return canUpdate; } } } });
    if (name === '$lib/API') return dependency(name, { default: { Chapter: api.namespace } });
    if (name === '$lib/stores') return dependency(name, { Session: { isValid: async () => { calls.cookie++; return cookie; } } });
    if (name === '@sveltejs/kit') return dependency(name, { error: status => { throw Object.assign(Error('HTTP ' + status), { status }); } });
    if (name === 'http-status-codes') return dependency(name, { StatusCodes: { OK: 200, FORBIDDEN: 403, INTERNAL_SERVER_ERROR: 500 } });
    throw Error('Unexpected chapter loader import ' + name);
  });
  await loader.evaluate();
  return { calls, api: api.namespace, load: (id = chapter) => loader.namespace.load({ params: { id } }) };
}

test('composed chapter edit loader reaches actual public Chapter.Get without a native cookie or native grants', async () => {
  const f = await fixture();
  assert.deepEqual(plain(await f.load()), { id: chapter, chapter: row });
  assert.deepEqual(plain(f.calls), { public: [{ operation: 'chapter.get', options: { id: chapter } }], cookie: 0, grants: 0, native: 0 });
});
test('an actual canonical chapter response, rather than the requested alias, owns the editor target', async () => {
  const f = await fixture();
  const data = await f.load(alias);
  assert.equal(data.id, chapter);
  assert.equal(data.chapter.relationships.manga.id, parent);
  assert.equal(f.calls.public[0].options.id, alias);
});
for (const status of [401, 403, 404, 409, 503]) test('public chapter read refusal ' + status + ' never falls through to cookie admission', async () => {
  const f = await fixture({ reply: { status } });
  await assert.rejects(f.load, error => error.status === status);
  assert.equal(f.calls.public.length, 1);
  assert.equal(f.calls.cookie + f.calls.grants + f.calls.native, 0);
});
for (const reply of [undefined, { status: 200 }, { status: 201, body: row }]) test('absent or non-200 public chapter success does not invent editor data: ' + JSON.stringify(reply), async () => {
  const f = await fixture({ reply: reply ?? null });
  await assert.rejects(f.load, error => error.status === 502);
  assert.equal(f.calls.cookie + f.calls.grants + f.calls.native, 0);
});
test('public transport failure is not replaced with a native cookie/HTTP fallback', async () => {
  const failure = Error('Synthetic unavailable catalogue'), f = await fixture({ failure });
  await assert.rejects(f.load, error => error === failure);
  assert.equal(f.calls.cookie + f.calls.grants + f.calls.native, 0);
});
test('gate-off loader retains native cookie and native update authority checks', async () => {
  for (const [cookie, canUpdate, allowed] of [[false, true, false], [true, false, false], [true, true, true]]) {
    const f = await fixture({ enabled: false, cookie, canUpdate });
    if (allowed) assert.equal((await f.load()).chapter, row);
    else await assert.rejects(f.load, error => error.status === 403);
    assert.equal(f.calls.cookie, 1);
    assert.equal(f.calls.public.length, 0);
    assert.equal(f.calls.grants, cookie ? 1 : 0);
  }
});
test('selected signer discovers the canonical chapter edit link without granting any mutation authority', () => {
  const on = composed('src/lib/components/ChapterListEntry.svelte'), off = composed('src/lib/components/ChapterListEntry.svelte', false);
  assert.match(on, /import \{ signerIdentity \} from "\$localLibrary\/publication"/);
  const guard = on.match(/\{#if ([^\r\n]+)\}\s*<a[\s\S]*?href="\/chapter\/\{chapter.id\}\/edit"/);
  assert.ok(guard, 'actual edit anchor must have an explicit condition');
  const visible = new Function('$signerIdentity', '$Session', 'Permission', 'chapter', 'return (' + guard[1] + ')');
  const forbidden = { Chapter: { canUpdate() { throw Error('Cookie authority must not admit a public editor'); } } };
  assert.equal(!!visible({ owner: 'a'.repeat(64) }, null, forbidden, row), true);
  assert.equal(!!visible({ owner: null }, null, forbidden, row), false);
  assert.match(off, /\{#if \$Session && Permission.Chapter.canUpdate\(chapter\)\}/);
  assert.doesNotMatch(off, /signerIdentity/);
});
test('fully composed edit layout/page reach the existing keyed signed child and preserve its immutable/removal contract', () => {
  const layout = composed(route + '+layout.svelte'), page = composed(route + '+page.svelte');
  assert.match(layout, /@render children\?\.\(\)/);
  assert.doesNotMatch(layout, /ProtectedVerified|Session/);
  assert.match(composed(route + '+layout.svelte', false), /ProtectedVerified/);
  assert.match(page, /import PublicChapterEditor from "\$publicApi\/PublicChapterEditor.svelte"/);
  assert.match(page, /\{#key data.chapter.id\}<PublicChapterEditor initial=\{data.chapter\} \/>\{\/key\}/);
  const editor = fs.readFileSync(path.join(root, 'platform-integrations/public-api/PublicChapterEditor.svelte'));
  assert.equal(createHash('sha256').update(restoreReviewedLanguageSource(restoreReaderUxPredecessor('weebdex-platform/platform-integrations/public-api/PublicChapterEditor.svelte',editor.toString()), 'chapterEditor')).digest('hex'), 'b368c2419b3eb1babe92bc3c13d274265249cf7c2200f63fb43415467689d328');
  // Signed editor affordance successor retains exact source binding.
  assert.match(editor.toString(), /owner\.chapterAccess\(chapter\.version\)/);
  assert.match(editor.toString(), /loadUploadAccount\(current\)/);
  assert.match(editor.toString(), /publicUploadOwner\(\(\) => initial.id, account, 'chapter', 'caller'\)/);
  assert.match(editor.toString(), /removalChoice = approvesHide \? 'approve-hide' : 'request-review'/);
  assert.match(editor.toString(), /Original publications and their pages cannot be changed/);
  assert.match(editor.toString(), /Where page editing is available, prepare your changes privately below/);
  assert.match(editor.toString(), /<PublicChapterPagesEditor \{chapter\} \{owner\} \{access\}/);
  assert.match(editor.toString(), /onDestroy\(\(\) => \{ disposed = true; owner\?\.dispose\(\); \}\)/);
});
test('public native Chapter mutation methods stay blocked instead of exposing a revision API', async () => {
  const f = await fixture();
  for (const [method, args] of [['Update', [chapter, { version: 7 }]], ['Claim', [chapter]], ['Delete', [chapter]]]) assert.throws(() => f.api[method](...args), /Explicit signed chapter editor required/);
  assert.equal(f.calls.public.length + f.calls.native, 0);
});
