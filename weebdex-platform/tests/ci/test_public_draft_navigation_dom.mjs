// Historical signed route regression; not current native UI qualification.
import {historicalContributionSource,historicalContributionOverlayFactories} from './historical-contribution-ui-fixture.mjs';
const {createFrontendOverlays,createReaderOverlays,createLocalLibraryOverlays,createPublicApiOverlays,createPublicUploadOverlays}=await historicalContributionOverlayFactories();
// Actual composed preview loaders, route pages, and retained Manga tab markup.
// Pinned Svelte/JSDOM execute the UI; history routing and signed transport are
// finite stand-ins, not SvelteKit/browser/HTTP/signature/database acceptance.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import { root, fixture, row, id, other, actor, entity, deferred, RequestFailure } from './report-draft-fixture.mjs';

const dependencies = process.env.WEEBDEX_FRONTEND_ROOT;
const require = createRequire(path.join(dependencies, 'package.json'));
const { compile } = require('svelte/compiler'), ts = require('typescript');
const domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert.equal(require('svelte/package.json').version, '5.55.1');
assert.equal(require(path.join(domModules, 'jsdom/package.json')).version, '27.0.1');
const { JSDOM } = require(path.join(domModules, 'jsdom'));
const { window } = new JSDOM('<main></main>', { url: 'https://catalog.example/', pretendToBeVisual: true });
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'HTMLMediaElement', 'HTMLScriptElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) globalThis[key] = window[key];
const svelte = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const { mount, unmount, flushSync } = await import(svelte);
const stores = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/index-client.js')));
const frontend = path.join(root, 'frontend');
const direct = 'src/routes/(app)/title/[id]/+page.ts';
const slug = 'src/routes/(app)/title/[id]/[slug]/+page.ts';
function composed(relative, enabled = true) {
  const file = path.join(frontend, relative);
  let source = fs.readFileSync(file, 'utf8');
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled }), createPublicUploadOverlays(frontend, { enabled })]) source = plugin.transform.handler(source, file)?.code ?? source;
  return source;
}
// Execute the retained app's actual query-string convention, not a rewritten URL helper.
const originalToString = URLSearchParams.prototype.toString;
const appLayout = fs.readFileSync(path.join(frontend, 'src/routes/+layout.ts'), 'utf8');
new Function(stripTypeScriptTypes(appLayout.slice(appLayout.indexOf('const _toString'), appLayout.indexOf('Number.prototype.short')), { mode: 'transform' }))();
const utilsSource = fs.readFileSync(path.join(frontend, 'src/lib/Utils.ts'), 'utf8');
const makeSlugSource = utilsSource.slice(utilsSource.indexOf('export const makeSlug'), utilsSource.indexOf('export const markdownPreprocessor')).replace('export ', '');
const makeSlug = new Function('slugify', stripTypeScriptTypes(makeSlugSource, { mode: 'transform' }) + '; return makeSlug;')(require('slugify'));
fs.mkdirSync(path.join(root, '.build'), { recursive: true });
const generation = fs.mkdtempSync(path.join(root, '.build/draft-navigation-dom-'));
// Register before symlink/compilation; remove only this test-created generation.
test.after(() => {
  try { URLSearchParams.prototype.toString = originalToString; window.close(); delete globalThis.__draftNavigation; }
  finally { fs.rmSync(generation, { recursive: true, force: true }); assert.equal(fs.existsSync(generation), false); }
});

fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(generation, 'node_modules'));


async function component(name, source, rewrite = true) {
  if (rewrite) source = source.replace(/(<script[^>]*>)([\s\S]*?)(<\/script>)/g, (_all, opening, script, closing) => {
    const ast = ts.createSourceFile(name + '.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    for (const item of [...ast.statements].reverse()) {
      if (!ts.isImportDeclaration(item)) continue;
      const specifier = item.moduleSpecifier.text, clause = item.importClause;
      let replacement = '';
      if (specifier === 'svelte') replacement = item.getText(ast).replace(/(['"])svelte\1/, JSON.stringify(svelte));
      else if (clause && !clause.isTypeOnly) {
        const base = `globalThis.__draftNavigation.modules[${JSON.stringify(specifier)}]`, bindings = [];
        if (clause.name) bindings.push(`const ${clause.name.text}=${base}.default;`);
        if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) bindings.push(`const ${clause.namedBindings.name.text}=${base};`);
        else if (clause.namedBindings) for (const binding of clause.namedBindings.elements) if (!binding.isTypeOnly) bindings.push(`const ${binding.name.text}=${base}.${binding.propertyName?.text ?? binding.name.text};`);
        replacement = bindings.join('');
      }
      script = script.slice(0, item.getStart(ast)) + replacement + script.slice(item.end);
    }
    return opening + script + closing;
  });
  const file = path.join(generation, name + '.mjs');
  fs.writeFileSync(file, compile(source, { filename: name + '.svelte', generate: 'client', runes: true }).js.code);
  return (await import(pathToFileURL(file))).default;
}
const Empty = await component('Empty', '<span></span>', false);
const Generic = await component('Generic', '<script>let {children}=$props();</script><div>{@render children?.()}</div>', false);
const Chapters = await component('Chapters', '<p data-section="chapters">Chapter section boundary</p>', false);
const Covers = await component('Covers', '<p data-section="covers">Cover section boundary</p>', false);
const Manga = await component('Manga', composed('src/routes/(app)/title/Manga.svelte'));
const DirectPage = await component('DirectPage', composed(direct.replace('.ts', '.svelte')));
const SlugPage = await component('SlugPage', composed(slug.replace('.ts', '.svelte')));
const Harness = await component('Harness', `<script>
  import Direct from './DirectPage.mjs'; import Slug from './SlugPage.mjs';
  let data=$state(), route=$state(new URL(window.location.href)), failure=$state('');
  globalThis.__draftNavigation.location=()=>route;
  globalThis.__draftNavigation.install=(value,message='')=>{route=new URL(window.location.href);data=value;failure=message;};
</script>{#if failure}<p role="alert">{failure}</p>{:else if data}{#if route.pathname.split('/').filter(Boolean).length===3}<Slug {data}/>{:else}<Direct {data}/>{/if}{/if}`, false);

async function loaders(f, { enabled = true, oldSlug = false, cookie = false } = {}) {
  const context = vm.createContext({ window, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, AbortController, console });
  const modules = new Map(), directory = path.join(root, 'platform-integrations/public-api');
  const native = { session: 0, draft: 0, public: 0, permission: 0 };
  function stub(name, exports) {
    if (!modules.has(name)) modules.set(name, new vm.SyntheticModule(Object.keys(exports), function () { for (const [key, value] of Object.entries(exports)) this.setExport(key, value); }, { context, identifier: name }));
    return modules.get(name);
  }
  async function load(file) {
    if (!modules.has(file)) {
      const relative = path.relative(frontend, file).split(path.sep).join('/');
      const source = file.startsWith(frontend + path.sep) ? composed(relative, relative === slug && oldSlug ? false : enabled) : fs.readFileSync(file, 'utf8');
      const module = new vm.SourceTextModule(stripTypeScriptTypes(source, { mode: 'transform', sourceUrl: file }), { context, identifier: file });
      modules.set(file, module);
      await module.link((specifier, importer) => {
        if (specifier === '$lib') return stub('lib', { API: { Manga: { GetDraft: async target => { native.draft++; return { status: 200, body: { id: target, state: 'draft' } }; }, Get: async target => { native.public++; return { status: 200, body: { id: target, state: 'published' } }; } } }, Permission: { Manga: { canUpdate: () => { native.permission++; return cookie; } } } });
        if (specifier === '$lib/stores/index.js') return stub('session', { Session: { isValid: async () => { native.session++; return cookie; } } });
        if (specifier === '@sveltejs/kit') return stub('kit', { error: status => { throw Error('HTTP ' + status); } });
        if (specifier === 'http-status-codes') return stub('status', { StatusCodes: { OK: 200, FORBIDDEN: 403, INTERNAL_SERVER_ERROR: 500 } });
        if (specifier.includes('local-library/publication')) return stub('publication', { capabilities: f.deps.capabilities, selectedSigner: f.deps.selectedSigner });
        if (specifier.includes('protocol/nostr.js')) return stub('nostr', { origin: value => new URL(value).origin, proveService: f.deps.proveService });
        if (specifier === './nostr-upload-request') return stub('request', { createNostrUploadRequest: f.deps.createRequest, PublicUploadRequestError: RequestFailure });
        return load(specifier.startsWith('$publicApi/') ? path.join(directory, specifier.slice('$publicApi/'.length) + '.ts') : path.resolve(path.dirname(importer.identifier), specifier + '.ts'));
      });
    }
    return modules.get(file);
  }
  const directModule = await load(path.join(frontend, direct)); await directModule.evaluate();
  const slugModule = await load(path.join(frontend, slug)); await slugModule.evaluate();
  return { direct: directModule.namespace.load, slug: slugModule.namespace.load, native };
}
const routeUrl = (withSlug = false, query = 'draft=true') => `/title/${id}${withSlug ? '/actual-draft' : ''}?${query}`;
const args = () => ({ url: new URL(window.location.href), params: { id: window.location.pathname.split('/')[2], slug: window.location.pathname.split('/')[3] } });
async function settle() { for (let i = 0; i < 8; i++) { await new Promise(resolve => setImmediate(resolve)); flushSync(); } }
function draftWithCover(state = 'draft', version = 7) {
  const value = row(state, version);
  value.relationships.push({ type: 'cover_art', id: other, attributes: { fileName: 'actual-cover.webp', dimensions: [256, 364], version: 1 } });
  return value;
}
function previewFixture(options = {}) {
  return fixture({ ...options, handler: async request => {
    const handled = await options.handler?.(request); if (handled !== undefined) return handled;
    if (new URL(request.url).pathname === '/api/v1/manga/' + id) return { status: 404 };
  } });
}
async function ui(options = {}) {
  window.history.replaceState({}, '', options.url ?? routeUrl());
  const f = previewFixture({ row: draftWithCover(), ...options });
  const routes = await loaders(f, options);
  const env = { modules: {}, nativeCalls: 0 }, host = document.createElement('main');
  document.body.append(host);
  const forbidden = () => { env.nativeCalls++; throw Error('Unexpected native side effect'); };
  const Constants = { State: { Published: 'published' }, Manga: { PublicationStatus: { Ongoing: 'ongoing' }, PublicationStatusText: { ongoing: 'Ongoing' }, ContentRating: { Safe: 'safe' }, ContentRatingText: { safe: 'Safe' } }, Tag: { GroupText: {} } };
  env.modules = {
    '$app/state': { page: { get url() { return env.location(); }, state: {} } },
    '$app/navigation': { replaceState: (url, state) => { window.history.replaceState(state, '', url); env.install(env.data); } },
    '$env/static/public': { PUBLIC_SITE_NAME: 'Fixture', PUBLIC_COVER_URL: '/covers' },
    '$lib': { Constants, API: { ChapterHistory: { List: forbidden }, Statistics: { Manga: forbidden } }, Utils: { makeSlug, parseDescription: value => value, groupTags: () => ({}) }, Typography: { exec() {} }, ExternalLink: { group: () => ({}) } },
    '$lib/components/comments': { Provider: Generic }, '$lib/components/comments/context.svelte': { useComments: () => ({}) },
    '$lib/components/ui/badge': { Badge: Generic, badgeVariants: () => '' },
    '$lib/components/ui/breadcrumb/index.js': Object.fromEntries(['Root', 'List', 'Item', 'Link', 'Separator'].map(name => [name, Generic])),
    '$lib/components/ui/button': { buttonVariants: () => '' }, '$lib/components/ui/lightbox': { useLightbox: () => ({ set() {}, open() {} }) },
    '$lib/Context': { setManga() {}, setReadHistory() {} }, '$lib/stores': { isMobile: { current: false } }, '$lib/Utils': { cn: () => '' },
    '@lucide/svelte': Object.fromEntries(['Bookmark', 'Expand', 'Eye', 'FilesIcon', 'Images', 'LinkIcon', 'MessageSquare', 'MessagesSquare', 'Star', 'StarIcon'].map(name => [name, Empty])),
    'http-status-codes': { StatusCodes: { OK: 200 } }, pluralize: { default: require('pluralize') }, 'svelte/store': stores,
    '../Manga.svelte': { default: Manga }, '../../Manga.svelte': { default: Manga },
    './MangaChapterListSection.svelte': { default: Chapters }, './MangaCoversSection.svelte': { default: Covers },
    '$publicApi/media': { retainedCoverUrl: () => '/covers/fixture.webp' },
    '$publicApi/rating-summary': { instanceRatingLabel: () => 'Not loaded' },
    '$localLibrary/library': { libraryIdentity: stores.writable({ canUse: false }), libraryViewKey: stores.writable('fixture'), captureLibraryView: () => 'fixture' }
  };
  for (const name of ['$lib/components/ui/Collapsible.svelte', '$lib/components/ui/Scrollable.svelte']) env.modules[name] = { default: Generic };
  for (const name of ['$lib/components/ui/ImageLoader.svelte', '$lib/components/ui/NotFound.svelte', '$localLibrary/MangaDiscussion.svelte', './Analytics.svelte', './MangaActions.svelte', './MangaRecommendationsSection.svelte', './MangaRelatedSection.svelte']) env.modules[name] = { default: Empty };
  globalThis.__draftNavigation = env;
  const instance = mount(Harness, { target: host });
  let generation = 0;
  async function render() {
    const current = ++generation;
    // Finite router cancellation boundary. It is not substituted for a claim of
    // SvelteKit cancellation, which still requires permitted native browser QA.
    env.data = undefined; env.install(undefined);
    try {
      const load = window.location.pathname.split('/').filter(Boolean).length === 3 ? routes.slug : routes.direct;
      const data = await load(args());
      if (current === generation) { env.data = data; env.install(data); }
    } catch (error) { if (current === generation) env.install(undefined, error.message); }
    await settle();
  }
  let navigation;
  host.addEventListener('click', event => {
    const link = event.target.closest('nav a'); if (!link) return;
    event.preventDefault(); window.history.pushState({}, '', link.href); navigation = render();
  });
  async function history(direction) {
    const changed = new Promise(resolve => window.addEventListener('popstate', resolve, { once: true }));
    window.history[direction](); await changed; await render();
  }
  await render();
  return { ...f, routes, env, host, render, history,
    async click(name) { const link = [...host.querySelectorAll('nav a')].find(node => node.textContent.trim() === name); assert.ok(link, 'Missing enabled tab ' + name); link.click(); await navigation; },
    async navigate(url) { window.history.pushState({}, '', url); await render(); },
    async dispose() { generation++; await unmount(instance); host.remove(); }
  };
}

test('enabled slug re-exports the exact signed loader; old native slug is the failing control', async () => {
  const f = previewFixture(), enabled = await loaders(f), old = await loaders(f, { oldSlug: true });
  assert.equal(enabled.direct, enabled.slug);
  window.history.replaceState({}, '', routeUrl(true));
  await assert.rejects(() => old.slug(args()), /HTTP 403/);
  assert.equal(f.calls.length, 0);
  const result = await enabled.slug(args());
  assert.equal(result.manga.id, id); assert.equal(result.manga.state, 'draft');
  assert.deepEqual(enabled.native, { session: 0, draft: 0, public: 0, permission: 0 });
});

test('gate-off keeps cookie admission, draft read and grants for both URL shapes', async () => {
  for (const shape of ['direct', 'slug']) {
    const f = previewFixture(), denied = await loaders(f, { enabled: false }), allowed = await loaders(f, { enabled: false, cookie: true });
    window.history.replaceState({}, '', routeUrl(shape === 'slug'));
    await assert.rejects(() => denied[shape](args()), /HTTP 403/);
    const result = await allowed[shape](args());
    assert.equal(result.manga.state, 'draft');
    assert.deepEqual(allowed.native, { session: 1, draft: 1, public: 0, permission: 1 });
    assert.equal(f.calls.length, 0);
  }
});

test('non-draft shared route retains the public API branch and never requests signer authority', async () => {
  const f = previewFixture(), routes = await loaders(f);
  for (const shape of ['direct', 'slug']) {
    window.history.replaceState({}, '', routeUrl(shape === 'slug', 'tab=covers'));
    assert.equal((await routes[shape](args())).manga.state, 'published');
  }
  assert.equal(routes.native.public, 2); assert.equal(f.calls.length, 0);
});

for (const state of ['draft', 'submitted', 'rejected']) test('direct slug preview preserves actual ' + state + ' state and enabled retained cover tab', async () => {
  const f = await ui({ url: routeUrl(true, 'draft=true&tab=covers'), row: draftWithCover(state, 9) });
  try {
    assert.equal(f.env.data.manga.state, state); assert.equal(f.env.data.manga.version, 9);
    assert.ok(f.host.querySelector('[data-section="covers"]'));
    assert.deepEqual([...f.host.querySelectorAll('nav a')].map(node => node.textContent.trim()), ['Chapters', 'Covers']);
    assert.equal(f.env.nativeCalls, 0); assert.equal(f.routes.native.session, 0);
  } finally { await f.dispose(); }
});

test('actual enabled tab click, Back, Forward and reload retain private preview and re-read current draft revision', async () => {
  const f = await ui();
  try {
    assert.ok(f.host.querySelector('[data-section="chapters"]'));
    const coverLink = [...f.host.querySelectorAll('nav a')].find(node => node.textContent.trim() === 'Covers');
    assert.equal(new URL(coverLink.href).pathname, `/title/${id}/actual-draft`);
    assert.equal(new URL(coverLink.href).searchParams.get('draft'), 'true');
    await f.click('Covers');
    assert.ok(f.host.querySelector('[data-section="covers"]'));
    f.setRow(draftWithCover('submitted', 8));
    await f.history('back');
    assert.equal(window.location.pathname, `/title/${id}`);
    assert.ok(f.host.querySelector('[data-section="chapters"]'));
    assert.equal(f.env.data.manga.state, 'submitted');
    await f.history('forward');
    assert.ok(f.host.querySelector('[data-section="covers"]'));
    f.setRow(draftWithCover('rejected', 9));
    await f.render();
    assert.equal(f.env.data.manga.state, 'rejected'); assert.equal(f.env.data.manga.version, 9);
    await f.click('Chapters');
    assert.ok(f.host.querySelector('[data-section="chapters"]'));
    assert.equal(window.location.search, '?draft=true');
    assert.equal(f.calls.filter(call => call.url.endsWith('/nostr/account-context')).length, 6);
    assert.ok(f.calls.every(call => call.method === 'GET'));
    assert.deepEqual(f.routes.native, { session: 0, draft: 0, public: 0, permission: 0 });
  } finally { await f.dispose(); }
});

for (const refusal of [403, 404, 503]) test('slug preview refusal ' + refusal + ' renders no stale title and has no native fallback', async () => {
  const f = await ui({ url: routeUrl(true), handler: request => request.url.includes('/manga/') ? { status: refusal } : undefined });
  try { assert.match(f.host.textContent, new RegExp('HTTP ' + refusal)); assert.equal(f.env.data, undefined); assert.equal(f.routes.native.draft, 0); } finally { await f.dispose(); }
});

test('disconnected signer and mismatched service/account identity fail closed on slug entry', async () => {
  for (const change of [f => { f.states.identity = false; }, f => { f.account.nostr.serviceKey = 'c'.repeat(64); }, f => { f.account.nostr.pubkey = 'c'.repeat(64); }]) {
    const f = previewFixture(); change(f); const routes = await loaders(f);
    window.history.replaceState({}, '', routeUrl(true));
    await assert.rejects(() => routes.slug(args()));
    assert.ok(f.calls.every(call => call.url.endsWith('/nostr/account-context')));
    assert.equal(routes.native.draft, 0);
  }
});

test('signed preview reads actual captured account and rechecks service before private dispatch', async () => {
  const f = previewFixture(); f.account.user.id = other;
  const original = f.deps.capabilities; let checks = 0;
  f.deps.capabilities = async () => { const policy = await original(); if (++checks >= 4) policy.service_key = 'c'.repeat(64); return policy; };
  const routes = await loaders(f); window.history.replaceState({}, '', routeUrl(true));
  await assert.rejects(() => routes.slug(args()), /service\/catalogue changed/);
  assert.ok(f.calls.every(call => call.url.endsWith('/nostr/account-context')));
  assert.equal(f.account.user.id, other); assert.notEqual(f.account.user.id, actor);
});

for (const cause of ['route', 'signer']) test('late signed slug read is rejected after ' + cause + ' changes', async () => {
  const gate = deferred(), f = previewFixture({ handler: async request => { if (request.url.includes('/manga/')) { await gate.promise; return entity(row()); } } });
  const routes = await loaders(f); window.history.replaceState({}, '', routeUrl(true));
  const pending = routes.slug(args()); const refusal = assert.rejects(() => pending, cause === 'route' ? /Title route changed/ : /Signer changed/);
  await settle();
  if (cause === 'route') window.history.replaceState({}, '', `/title/${other}/another?draft=true`); else f.states.identity = false;
  gate.release(); await refusal;
  assert.equal(routes.native.draft, 0); assert.ok(f.calls.every(call => call.method === 'GET'));
});

test('old slug negative control enters the signed preview but breaks on the actual enabled Covers link', async () => {
  const f = await ui({ oldSlug: true });
  try {
    assert.ok(f.host.querySelector('[data-section="chapters"]'));
    await f.click('Covers');
    assert.match(f.host.textContent, /HTTP 403/);
    assert.equal(f.env.data, undefined);
    assert.equal(f.routes.native.session, 1); assert.equal(f.routes.native.draft, 0);
    assert.equal(f.calls.filter(call => call.url.endsWith('/nostr/account-context')).length, 1);
  } finally { await f.dispose(); }
});

test('delayed Covers navigation cannot replace newer direct-route data or restore an old private preview', async () => {
  const gate = deferred(); let hold = false;
  const f = await ui({ handler: async request => {
    if (hold && new URL(request.url).pathname === '/api/v1/manga/draft/' + id) {
      hold = false; await gate.promise; return entity(draftWithCover('draft', 7));
    }
  } });
  try {
    hold = true; const pending = f.click('Covers'); await settle();
    assert.equal(f.env.data, undefined); assert.doesNotMatch(f.host.textContent, /Actual draft/);
    f.setRow(draftWithCover('submitted', 8));
    await f.navigate(routeUrl());
    gate.release(); await pending;
    assert.equal(f.env.data.manga.state, 'submitted'); assert.equal(f.env.data.manga.version, 8);
    assert.ok(f.host.querySelector('[data-section="chapters"]'));
    assert.equal(f.host.querySelector('[role="alert"]'), null);
    assert.ok(f.calls.every(call => call.method === 'GET'));
  } finally { gate.release(); await f.dispose(); }
});

test('signer loss before a tab request clears the previous private preview without cookie fallback', async () => {
  const f = await ui();
  try {
    assert.ok(f.host.querySelector('[data-section="chapters"]'));
    f.states.identity = false; await f.click('Covers');
    assert.match(f.host.textContent, /Signer changed/); assert.equal(f.env.data, undefined);
    assert.doesNotMatch(f.host.textContent, /Actual draft/);
    assert.equal(f.routes.native.session, 0);
    assert.equal(f.calls.filter(call => call.url.endsWith('/nostr/account-context')).length, 1);
  } finally { await f.dispose(); }
});
