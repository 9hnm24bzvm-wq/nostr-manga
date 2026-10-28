// SOURCE-AUTHORED, UNEXECUTED. Compose the exact current native/public overlay
// pipeline, then invoke its real page/keyboard methods with the real current
// controller -> query -> owned HTTP bytes -> DTO path. No Svelte/browser claim.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import assert from 'node:assert/strict';
import { nativeContinueFixture } from '../fixtures/bounded-reader/native-continue-fixture.mjs';
import { fixture, row, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
import { createBoundedReaderOverlays, boundedReaderOverlayManifest, assertBoundedReaderMarkup } from '../../platform-integrations/bounded-reader/overlays.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url)), frozen = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
const source = relative => fs.readFileSync(path.join(fs.existsSync(path.join(root, relative)) ? root : frozen, relative), 'utf8');
const load = relative => import(pathToFileURL(path.join(fs.existsSync(path.join(root, relative)) ? root : frozen, relative)).href);
const { createFrontendOverlays } = await load('platform-integrations/frontend-overlays.mjs');
const { createReaderOverlays } = await load('platform-integrations/frontend-reader-overlays.mjs');
const { createLocalLibraryOverlays } = await load('platform-integrations/local-library/overlays.mjs');
const { createPublicApiOverlays } = await load('platform-integrations/public-api/overlays.mjs');
const { createPublicUploadOverlays } = await load('platform-integrations/public-api/upload-overlays.mjs');
const frontend = path.join(frozen, 'frontend'), pipelines = new Map();
function compose(relative, publicMode) {
  const file = path.join(frontend, relative), original = fs.readFileSync(file, 'utf8'); let input = original;
  if (!pipelines.has(publicMode)) pipelines.set(publicMode, [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled: publicMode }), createPublicUploadOverlays(frontend, { enabled: publicMode })]);
  for (const plugin of pipelines.get(publicMode)) input = plugin.transform.handler(input, file)?.code ?? input;
  const output = createBoundedReaderOverlays(frontend).transform.handler(input, file)?.code ?? input;
  return { input, output, original };
}
const constants = { Reader: { Layout: { Paged: 'paged', Scroll: 'scroll' }, Direction: { RTL: 'rtl', LTR: 'ltr' } } };
function pageMethod(code, name) {
  const normalized = code.replace(/\r\n/g, '\n'), start = normalized.indexOf('\tasync ' + name + '('), end = normalized.indexOf(name === 'gotoPreviousPage' ? '\n\tasync gotoNextPage' : '\n\treadonly listener', start);
  assert(start > 0 && end > start); const method = stripTypeScriptTypes(normalized.slice(start, end).trim().replace(/^async /, 'async function '), { mode: 'transform' });
  return new Function('get', 'skipEndOfChapterPage', 'Constants', 'return ' + method)(value => value, false, constants);
}
function pageReader(controller, auto = true) {
  const pages = Array.from({ length: 4 }, (_, index) => ({ index })); let titles = 0, scrolled = 0;
  return { pages, currentPage: pages[0], currentSpread: undefined, spreads: [], prefs: { double: false, auto_advance: auto, layout: 'paged' }, isViewingLastPage: false,
    mutex: { exec: work => work() }, navigation: controller, get navigationState() { return controller.snapshot; }, async backToTitle() { titles++; }, async scrollIntoView() { scrolled++; }, get titles() { return titles; }, get scrolled() { return scrolled; } };
}
for (const publicMode of [false, true]) {
  test((publicMode ? 'public' : 'native') + ': actual reader consumers are bounded; only the explicit device title branch retains its aggregate', () => {
    for (const relative of new Set(boundedReaderOverlayManifest.map(patch => patch.path))) {
      const { input, output } = compose(relative, publicMode);
      assert.doesNotMatch(output, /reader\.aggregated|this\.aggregated|chapterIndex|changeChapter\(|findChapter\(/, relative);
      if (relative.endsWith('/MangaActions.svelte')) { assert.equal(output.match(/API\.Chapter\.Aggregate/g)?.length, 1); assert.match(output, /captureDeviceReading\(\)/); }
      else assert.doesNotMatch(output, /API\.Chapter\.Aggregate/, relative);
      if (relative.endsWith('.svelte')) { assertBoundedReaderMarkup(input, output, relative); assert.throws(() => assertBoundedReaderMarkup(input, output + '<p>Unreviewed markup</p>', relative)); }
    }
    assert.match(compose('src/lib/components/reader/reader-provider.svelte', publicMode).output, /createBoundedReader/);
    assert.match(compose('src/routes/(app)/title/MangaActions.svelte', publicMode).output, /<TitleReadingActions/);
  });
  test((publicMode ? 'public' : 'native') + ': actual page methods preserve endcard, auto-advance and previous last-page intent', async () => {
    const f = await fixture(publicMode); await f.controller.refresh(); const code = compose('src/lib/components/reader/context.svelte.ts', publicMode).output;
    const previous = pageMethod(code, 'gotoPreviousPage'), next = pageMethod(code, 'gotoNextPage'), reader = pageReader(f.controller);
    await previous.call(reader); assert.equal(f.navigated[0].target.id, row(publicMode, 1).id); assert.equal(f.navigated[0].lastPage, true);
    reader.currentPage = reader.pages.at(-1); const before = f.calls.length; await next.call(reader); assert.equal(reader.isViewingLastPage, true); assert.equal(f.calls.length, before);
    await next.call(reader); assert.equal(f.navigated.at(-1).target.id, row(publicMode, 3).id); assert.equal(f.navigated.at(-1).lastPage, false); f.controller.dispose();
  });
  test((publicMode ? 'public' : 'native') + ': actual page methods preserve no-auto-advance and directly readable excluded chapter', async () => {
    const f = await fixture(publicMode); f.responder = url => f.envelope(url, { state: 'excluded-current', data: [], current: undefined }); await f.controller.refresh();
    const code = compose('src/lib/components/reader/context.svelte.ts', publicMode).output, previous = pageMethod(code, 'gotoPreviousPage'), next = pageMethod(code, 'gotoNextPage'), reader = pageReader(f.controller, false);
    await next.call(reader); assert.equal(reader.currentPage, reader.pages[1]); reader.currentPage = reader.pages.at(-1); await next.call(reader); assert.equal(reader.isViewingLastPage, true); await next.call(reader); reader.currentPage = reader.pages[0]; await previous.call(reader);
    assert.equal(f.calls.length, 1); assert.equal(f.navigated.length, 0); assert.equal(reader.titles, 0); f.controller.dispose();
  });
  test((publicMode ? 'public' : 'native') + ': actual double-page edges keep current spread geometry', async () => {
    const f = await fixture(publicMode), code = compose('src/lib/components/reader/context.svelte.ts', publicMode).output, next = pageMethod(code, 'gotoNextPage'), previous = pageMethod(code, 'gotoPreviousPage'), reader = pageReader(f.controller);
    reader.prefs.double = true; reader.spreads = [reader.pages.slice(0, 2), reader.pages.slice(2)]; reader.currentSpread = reader.spreads[0]; reader.currentPage = reader.pages[1]; await next.call(reader); assert.equal(reader.currentPage, reader.pages[2]); reader.currentSpread = reader.spreads[1]; await previous.call(reader); assert.equal(reader.currentPage, reader.pages[0]); assert.equal(f.calls.length, 0); f.controller.dispose();
  });
  test((publicMode ? 'public' : 'native') + ': actual last-page boundary validates before title fallback', async () => {
    const f = await fixture(publicMode); f.responder = url => f.envelope(url, url.searchParams.get('operation') === 'around' ? { next: null } : { state: 'boundary', data: [] }); await f.controller.refresh();
    const reader = pageReader(f.controller, false); reader.currentPage = reader.pages.at(-1); reader.isViewingLastPage = true;
    await pageMethod(compose('src/lib/components/reader/context.svelte.ts', publicMode).output, 'gotoNextPage').call(reader); assert.equal(reader.titles, 1); assert.equal(f.calls.length, 2); assert.equal(f.navigated.length, 0); f.controller.dispose();
  });
}
test('actual composed keyboard semantic first/last retains RTL/LTR and modifier/control suppression', async () => {
  const code = compose('src/lib/components/reader/reader-keyboard-listener.svelte', false).output.replace(/\r\n/g, '\n'), start = code.indexOf('\tconst ownsKeyboard'), end = code.indexOf('\n\tconst onKeyUp', start); assert(start > 0 && end > start); assert.match(code, /button, summary, a\[href\]/);
  class Element { closest() { return false; } } class HTMLElement extends Element {} class HTMLInputElement extends HTMLElement { closest() { return this; } } class HTMLTextAreaElement extends HTMLElement {}
  for (const direction of ['ltr', 'rtl']) for (const key of ['Semicolon', 'Quote']) {
    const f = await fixture(); await f.controller.refresh(); const reader = { navigation: f.controller, scroll: { locked: false }, gotoFirstChapter() { return f.controller.semantic('first'); }, gotoLastChapter() { return f.controller.semantic('last'); } };
    const factory = new Function('reader', '$readerPrefs', 'Constants', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'getIsOpen', 'window', 'const onKeyUp = () => {};\n' + stripTypeScriptTypes(code.slice(start, end), { mode: 'transform' }) + '\nreturn onKeyDown;');
    const handler = factory(reader, { direction }, constants, Element, HTMLElement, HTMLInputElement, HTMLTextAreaElement, () => false, {});
    const event = extra => ({ code: key, target: {}, defaultPrevented: false, composedPath() { return [this.target]; }, preventDefault() {}, stopPropagation() {}, stopImmediatePropagation() {}, ...extra });
    for (const modifier of ['ctrlKey', 'metaKey', 'altKey', 'shiftKey', 'defaultPrevented']) handler(event({ [modifier]: true })); handler(event({ target: new HTMLInputElement() })); assert.equal(f.calls.length, 1);
    handler(event()); await turns(); await turns(); const first = direction === 'ltr' ? key === 'Semicolon' : key === 'Quote'; assert.equal(f.calls.at(-1).url.searchParams.get('operation'), first ? 'first' : 'last'); assert.equal(f.navigated[0].target.id, first ? 'chapter1' : 'chapter3'); f.controller.dispose();
  }
});
test('owned title/group/edition UI binds one mode-specific primary, explicit Start and exact row choices', () => {
  const title = source('platform-integrations/bounded-reader/TitleReadingActions.svelte'), group = source('platform-integrations/bounded-reader/GroupPage.svelte'), editions = source('platform-integrations/bounded-reader/EditionPage.svelte');
  assert.match(title, /controller\.start\(\)/); assert.match(title, /controller\.continueNative\(\)/); assert.match(title, /devicePrimary\(!!blocked, prepareDevice\)/); assert.match(title, /<summary>More reading options<\/summary>/); assert.match(title, /Start from first chapter/); assert.doesNotMatch(title, /controller\.continueScan|controller\.openCandidate/);
  assert.match(group, /controller\.chooseGroup\(row\)/); assert.match(group, /controller\.groups\(true\)/); assert.match(editions, /controller\.chooseEdition\(winner\)/); assert.match(editions, /controller\.chooseEdition\(row\)/); assert.match(editions, /controller\.editions\(undefined, true\)/);
});
test('actual semantic button owner suppresses modified and handled mouse actions', async () => {
  const f = await fixture(), { plainAction } = await f.module('platform-integrations/bounded-reader/presentation.ts'); await f.controller.refresh();
  const event = extra => ({ button: 0, preventDefault() {}, ...extra }); for (const modifier of ['ctrlKey', 'metaKey', 'altKey', 'shiftKey', 'defaultPrevented']) plainAction(event({ [modifier]: true }), () => f.controller.semantic('next')); plainAction(event({ button: 1 }), () => f.controller.semantic('next')); assert.equal(f.calls.length, 1);
  plainAction(event(), () => f.controller.semantic('next')); await turns(); assert.equal(f.navigated[0].target.id, 'chapter3'); f.controller.dispose();
});

test('actual page selector highlights pages/spreads without a chapter index', () => {
  for (const publicMode of [false, true]) {
    const code = compose('src/lib/components/reader/reader-page-selector.svelte', publicMode).output;
    assert.match(code, /class:underline=\{reader.currentSpread === spread\}/); assert.match(code, /class:underline=\{reader.currentPage\?\.index === page.index\}/); assert.doesNotMatch(code, /chapterIndex/);
  }
});

test('pagination-only selector keeps status and retry visible outside collapsed chapter details', () => { const group = source('platform-integrations/bounded-reader/GroupPage.svelte'); assert.match(group, /showStatus = true/); assert.match(group, /<\/details>\s*\{#if showStatus\}<Status \{controller\} refresh \/>/); for (const publicMode of [false, true]) assert.match(compose('src/lib/components/reader/reader-pagination.svelte', publicMode).output, /<GroupPage/); });

for (const [mode, publicCatalogue, pending, expected] of [
  ['device', false, false, /checks device completion locally/],
  ['account', false, false, /native cookie account’s server history/],
  ['account', true, false, /unavailable in this public catalogue/],
  ['device', false, true, /selection is pending/]
]) test('non-primary observed-scan explanation remains mode-specific: ' + [mode, publicCatalogue, pending].join('/'), async () => {
  const f = await fixture(), presentation = await f.module('platform-integrations/bounded-reader/presentation.ts');
  assert.match(presentation.continueHistoryExplanation(mode, publicCatalogue, pending), expected);
  assert.doesNotMatch(source('platform-integrations/bounded-reader/TitleReadingActions.svelte'), /continueHistoryExplanation|Continue scanning/);
  f.controller.dispose();
});

// Invoke the actual composed device handler and its actual admission helpers.
// Store reactivity, aggregate HTTP and router/dialog rendering are explicit seams.
// The retained selection algorithm below is extracted, never reimplemented.
async function deviceTitle(publicMode = false, history = {}) {
  const f = await nativeContinueFixture(), actual = compose('src/routes/(app)/title/MangaActions.svelte', publicMode).output.replace(/\r\n/g, '\n');
  const a = actual.indexOf('\tlet isStartReading ='), guard = actual.indexOf('\tconst guardDeviceChoice =', a), b = actual.indexOf('\n\t};', guard) + '\n\t};'.length;
  const c = actual.indexOf('\tconst startRead = () =>'), d = actual.indexOf('\n\t\t});', c) + '\n\t\t});'.length;
  assert(a > 0 && guard > a && b > guard && c > 0 && d > c);
  f.source.mode = 'device'; f.source.context = { generation: 1, mode: 'device', accountId: undefined }; f.source.ready = true; f.source.scopeStatus = 'verified'; f.source.scope = { ...f.identity }; f.source.reason = undefined;
  const historyModule = await f.module('platform-integrations/bounded-reader/history.ts'), libraryModule = await f.module('platform-integrations/local-library/library.ts'), navigationModule = await f.module('platform-integrations/frontend-behavior/navigation-authority.ts');
  const calls = [], navigated = [], errors = []; let response = { status: 200, body: { chapters: [] } }, handler;
  const selection = { get epoch() { return f.source.selectionEpoch; }, get pending() { return f.source.selectionPending; } };
  class Failure extends Error { title = 'Unavailable'; body = 'Unavailable'; }
  const env = { $state: value => value, $derived: value => value, $effect: work => work(), onDestroy() {},
    manga: { id: 'manga:one' }, page: { url: new URL('https://reader.test/title/manga:one?tlang=fr&tlang=de&group=g:2&group=g:3') },
    window: { location: { pathname: '/title/manga:one' } }, $prefs: {}, prefs: { applyChapterPreferences(q) { q.append('tlang', 'en'); q.append('group', 'g:1'); } },
    $libraryIdentity: f.source.context, $localLibraryState: f.source, $readerLibrarySelection: selection,
    captureNavigationEpoch: navigationModule.captureNavigationEpoch, historySourceKey: historyModule.historySourceKey, readerLibrarySource: f.snapshot, assertLibraryAdmission: libraryModule.assertLibraryAdmission,
    history, mutex: { exec: work => work() }, isPublished: true, isEmpty: false, StatusCodes: { OK: 200, TOO_MANY_REQUESTS: 429 },
    Constants: { Error: { Error: Failure, Unknown: new Failure(), TooManyRequests: new Failure() } }, toast: { error: (...args) => errors.push(args) }, console: { error() {}, info() {} },
    goto: async url => navigated.push(url), API: { Chapter: { Aggregate: async (manga, query) => { calls.push({ manga, query: new URLSearchParams(query) }); return typeof response === 'function' ? response() : response; } } }
  };
  const names = Object.keys(env), body = stripTypeScriptTypes(actual.slice(a, b) + '\n' + actual.slice(c, d), { mode: 'transform' });
  handler = new Function(...names, 'let $readHistory = history;\n' + body + '\nreturn { startRead, clearDeviceSelection, guardDeviceChoice, closeDialog() { isChapterSelectionDialogOpen = false; }, setHistory(value) { $readHistory = value; }, setBoundedPending(value) { boundedReadingPending = value; }, state() { return { isStartReading, isChapterSelectionDialogOpen, targetAggregatedChapter, deviceReadError }; } };')(...Object.values(env));
  return { f, calls, navigated, errors, handler, env, invalidateNavigation: navigationModule.invalidateNavigationEpoch, async dispatchLink(url) { let prevented = false; handler.guardDeviceChoice({ preventDefault() { prevented = true; }, stopPropagation() {} }); if (!prevented) await env.goto(url); return prevented; }, set response(value) { response = value; }, close() { f.controller.dispose(); assert.equal(f.timers.size, 0); } };
}
const deviceGroups = () => [{ volume: '1', chapter: '3', entries: { newest: {} } }, { volume: '1', chapter: '2', entries: { middle: {} } }, { volume: null, chapter: null, entries: { oldest: {} } }];
for (const publicMode of [false, true]) for (const [kind, history, expected] of [
  ['no-history', {}, 'oldest'], ['partial-any-edition', { 'manga:one': { oldest: true } }, 'middle'],
  ['all-completed', { 'manga:one': { oldest: true, middle: true, newest: true } }, 'newest']
]) test((publicMode ? 'public' : 'native') + ': actual composed device ' + kind + ' automatically preserves retained group selection', async () => {
  const f = await deviceTitle(publicMode, history); f.response = { status: 200, body: { chapters: deviceGroups() } }; await f.handler.startRead();
  assert.deepEqual(f.navigated, ['/chapter/' + expected + '/1']); assert.equal(f.calls.length, 1); assert.equal(f.handler.state().isChapterSelectionDialogOpen, false);
  assert.deepEqual(f.calls[0].query.getAll('tlang'), ['fr', 'de']); assert.deepEqual(f.calls[0].query.getAll('group'), ['g:2', 'g:3']); assert.equal(f.handler.state().isStartReading, false); f.close();
});
test('actual device handler marks a group completed by any edition and retains its multi-edition chooser', async () => {
  const f = await deviceTitle(false, { 'manga:one': { oldAlternative: true } }), groups = deviceGroups(); groups[2].entries.oldAlternative = {}; groups[1].entries.middleAlternative = {};
  f.response = { status: 200, body: { chapters: groups } }; await f.handler.startRead(); assert.equal(f.navigated.length, 0); assert.equal(f.handler.state().isChapterSelectionDialogOpen, true);
  assert.deepEqual(Object.keys(f.handler.state().targetAggregatedChapter.entries), ['middle', 'middleAlternative']); let blocked = false; f.handler.guardDeviceChoice({ preventDefault() { blocked = true; }, stopPropagation() {} }); assert.equal(blocked, false); f.close();
});
for (const fault of ['history-pending', 'history-failed', 'selection-pending', 'account-mode', 'scope-unverified']) test('actual device ' + fault + ' cannot become an empty-history Start fallback', async () => {
  const f = await deviceTitle(); if (fault.startsWith('history-')) f.handler.setHistory(undefined); else if (fault === 'selection-pending') f.f.source.selectionPending = true; else if (fault === 'account-mode') f.f.source.mode = 'account'; else f.f.source.scopeStatus = 'mismatch';
  await f.handler.startRead(); assert.equal(f.calls.length, 0); assert.equal(f.navigated.length, 0); assert.equal(f.handler.state().isChapterSelectionDialogOpen, false); assert.match(f.handler.state().deviceReadError, /could not complete/); f.close();
});
for (const change of ['history', 'selection', 'scope', 'revision']) test('actual device aggregate wait fences changed ' + change + ' before selection', async () => {
  const f = await deviceTitle(), gate = Promise.withResolvers(); f.response = () => gate.promise; const work = f.handler.startRead(); await turns(); assert.equal(f.calls.length, 1);
  assert.equal(f.handler.clearDeviceSelection(), false); if (change === 'history') f.handler.setHistory({ 'manga:one': { oldest: true } }); else if (change === 'selection') { f.f.source.selectionEpoch += 2; f.f.source.selectionPending = true; } else if (change === 'scope') f.f.source.scope = { ...f.f.source.scope, catalogNamespace: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb' }; else f.f.source.revision++;
  gate.resolve({ status: 200, body: { chapters: deviceGroups() } }); await work; assert.equal(f.navigated.length, 0); assert.equal(f.handler.state().isChapterSelectionDialogOpen, false); assert.equal(f.errors.length, 0); f.close();
});
test('actual retained device chooser blocks stale selection and bounded Start competes with no legacy read', async () => {
  const f = await deviceTitle(), groups = deviceGroups(); groups[2].entries.other = {}; f.response = { status: 200, body: { chapters: groups } }; await f.handler.startRead();
  f.f.source.selectionEpoch++; let blocked = false; f.handler.guardDeviceChoice({ preventDefault() { blocked = true; }, stopPropagation() {} }); assert.equal(blocked, true); assert.equal(f.handler.state().isChapterSelectionDialogOpen, false);
  f.handler.setBoundedPending(true); await f.handler.startRead(); assert.equal(f.calls.length, 1); assert.equal(f.navigated.length, 0); f.close();
});
test('device aggregate >10K refusal remains explicit and never silently invokes another selection', async () => {
  const f = await deviceTitle(); f.response = { status: 400, body: { message: 'Aggregate exceeds 10000 chapters; narrow filters' } }; await f.handler.startRead();
  assert.equal(f.calls.length, 1); assert.equal(f.navigated.length, 0); assert.equal(f.handler.state().isChapterSelectionDialogOpen, false); assert.match(f.handler.state().deviceReadError, /narrow language\/group filters/); f.close();
});
for (const change of ['query', 'preferences', 'pathname', 'navigation-epoch', 'manga', 'closed', 'cleared']) test('actual device chooser immediately fences ' + change + ' before the invalidation effect runs', async () => {
  const f = await deviceTitle(), groups = deviceGroups(); groups[2].entries.other = {}; f.response = { status: 200, body: { chapters: groups } }; await f.handler.startRead(); assert.equal(f.handler.state().isChapterSelectionDialogOpen, true);
  if (change === 'query') f.env.page.url.searchParams.set('tlang', 'ja'); else if (change === 'preferences') f.env.$prefs.language = 'ja'; else if (change === 'pathname') f.env.window.location.pathname = '/title/other';
  else if (change === 'navigation-epoch') f.invalidateNavigation(); else if (change === 'manga') f.env.manga.id = 'other'; else if (change === 'closed') f.handler.closeDialog(); else f.handler.clearDeviceSelection();
  // The fixture never reruns $effect: this exercises the synchronous real guard.
  assert.equal(await f.dispatchLink('/chapter/oldest/1'), true); assert.equal(f.navigated.length, 0); assert.equal(f.handler.state().isChapterSelectionDialogOpen, false); f.close();
});

for (const publicMode of [false, true]) test((publicMode ? 'public' : 'native') + ': bounded edition selector preserves the retained panel class prop through its existing root', () => {
  const panel = compose('src/lib/components/reader/reader-panel.svelte', publicMode).output;
  const wrapper = compose('src/lib/components/reader/reader-group-selector.svelte', publicMode).output;
  const selector = source('platform-integrations/bounded-reader/EditionSelector.svelte');
  assert.match(panel, /<GroupSelector class="w-0 flex-auto"\s*\/>/);
  for (const code of [wrapper, selector]) assert.match(code, /let \{ class: className \}: \{ class\?: string \} = \$props\(\);/);
  assert.match(wrapper, /<EditionSelector class=\{className\}\s*\/>/);
  assert.match(selector, /<details class=\{\['rounded border p-2', className\]\}/);
});

test('semantic button preserves immediate void actions, ignored promises and synchronous failure identity', async () => {
  const f = await fixture(), { plainAction } = await f.module('platform-integrations/bounded-reader/presentation.ts');
  try {
    const order = [], event = () => ({ button: 0, preventDefault() { order.push('preventDefault'); } });
    assert.equal(plainAction(event(), () => { order.push('action'); }), undefined);
    assert.deepEqual(order, ['preventDefault', 'action']);
    assert.equal(plainAction(event(), () => undefined), undefined);
    const failure = Error('synchronous action failure');
    assert.throws(() => plainAction(event(), () => { throw failure; }), error => error === failure);
    const owned = Promise.resolve(); let observed = 0;
    for (const method of ['then', 'catch']) Object.defineProperty(owned, method, { get() { observed++; throw Error('Action owner was replaced'); } });
    assert.equal(plainAction(event(), () => owned), undefined); assert.equal(observed, 0);
  } finally { f.controller.dispose(); }
});

test('actual title Start guard may return void without clearing or starting blocked work', async () => {
  const f = await fixture(), { plainAction } = await f.module('platform-integrations/bounded-reader/presentation.ts');
  try {
    const declaration = source('platform-integrations/bounded-reader/TitleReadingActions.svelte').match(/^  const startFirst = .*;$/m)?.[0];
    assert(declaration, 'Actual title Start declaration must be present');
    for (const [blocked, cleared, expectedClear, expectedStart] of [[true, true, 0, 0], [false, false, 1, 0], [false, true, 1, 1]]) {
      let clears = 0, starts = 0, prevented = 0;
      const startFirst = new Function('blocked', 'clearDeviceSelection', 'controller', declaration + '\nreturn startFirst;')(
        blocked, () => { clears++; return cleared; }, { start() { starts++; return Promise.resolve(); } });
      assert.equal(plainAction({ button: 0, preventDefault() { prevented++; } }, startFirst), undefined);
      assert.equal(clears, expectedClear); assert.equal(starts, expectedStart); assert.equal(prevented, 1);
    }
  } finally { f.controller.dispose(); }
});

for (const publicMode of [false, true]) test((publicMode ? 'public' : 'native') + ': semantic button leaves transport rejection and pending cleanup with the real controller', async () => {
  const f = await fixture(publicMode), { plainAction } = await f.module('platform-integrations/bounded-reader/presentation.ts');
  try {
    await f.controller.refresh(); f.responder = async () => { throw Error('owned transport rejection'); };
    let operation; plainAction({ button: 0, preventDefault() {} }, () => operation = f.controller.semantic('next'));
    await operation;
    assert.equal(f.controller.snapshot.status, 'error'); assert.equal(f.controller.snapshot.pending, false);
    assert(f.controller.snapshot.message.length > 0); assert.equal(f.navigated.length, 0); assert.equal(f.calls.length, 2); assert.equal(f.timers.size, 0);
  } finally { f.controller.dispose(); }
});
