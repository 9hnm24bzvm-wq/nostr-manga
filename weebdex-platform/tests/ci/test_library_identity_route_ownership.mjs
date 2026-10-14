#!/usr/bin/env node
// Real pinned Svelte client components, runes, lifecycle, and DOM. Services and
// native UI primitives are explicit stand-ins. JSDOM does not establish layout,
// browser, signer, HTTP, PostgreSQL, or production performance acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test, { after } from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT;
assert(dependencies, 'Use the existing exact-lock frontend dependency root');
const require = createRequire(path.join(dependencies, 'package.json'));
const { compile } = require('svelte/compiler'), ts = require('typescript');
assert.equal(require('svelte/package.json').version, '5.55.1');
assert.equal(require('typescript/package.json').version, '5.9.3');
const domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(domModules, 'Use the existing pinned JSDOM dependency root');
assert.equal(require(path.join(domModules, 'jsdom/package.json')).version, '27.0.1');
const { JSDOM } = require(path.join(domModules, 'jsdom'));
const { window } = new JSDOM('<main></main>', { pretendToBeVisual: true, url: 'http://127.0.0.1:1' });
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'HTMLMediaElement', 'HTMLFormElement', 'Text', 'Comment', 'Event', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) globalThis[key] = window[key];
// JSDOM has no scrolling/layout implementation; this is an explicit no-op.
window.HTMLElement.prototype.scrollIntoView = () => {};
const client = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const { mount, unmount, flushSync } = await import(client);
const { writable } = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/index-client.js')));
const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)];
const compose = relative => {
  const file = path.join(frontend, relative);
  let source = fs.readFileSync(file, 'utf8');
  for (const plugin of plugins) source = plugin.transform.handler(source, file)?.code ?? source;
  return source;
};
const read = name => fs.readFileSync(path.join(root, 'platform-integrations/local-library', name + '.svelte'), 'utf8');
fs.mkdirSync(path.join(root, '.build'), { recursive: true });
const generation = fs.mkdtempSync(path.join(root, '.build/route-ownership-'));
fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(generation, 'node_modules'));
after(() => { window.close(); fs.rmSync(generation, { recursive: true, force: true }); delete globalThis.__relocationEnvironment; });

async function compiled(name, source, replaceImports = true) {
  if (replaceImports) source = source.replace(/<script lang="ts">([\s\S]*?)<\/script>/, (_match, script) => {
    const ast = ts.createSourceFile(name + '.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    for (const item of [...ast.statements].reverse()) {
      if (!ts.isImportDeclaration(item)) continue;
      const specifier = item.moduleSpecifier.text, clause = item.importClause;
      let replacement = '';
      if (specifier === 'svelte') replacement = item.getText(ast).replace(/(['"])svelte\1/, JSON.stringify(client));
      else if (clause && !clause.isTypeOnly) {
        const names = [];
        if (clause.name) names.push(clause.name.text);
        if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) names.push(clause.namedBindings.name.text);
        else if (clause.namedBindings) for (const entry of clause.namedBindings.elements) if (!entry.isTypeOnly) names.push(entry.name.text);
        if (names.length) replacement = `const { ${names.join(', ')} } = globalThis.__relocationEnvironment;`;
      }
      script = script.slice(0, item.getStart(ast)) + replacement + script.slice(item.end);
    }
    return '<script lang="ts">' + script + '</script>';
  });
  const result = compile(source, { filename: name + '.svelte', generate: 'client', runes: true });
  const file = path.join(generation, name + '.mjs'); fs.writeFileSync(file, result.js.code);
  return (await import(pathToFileURL(file))).default;
}
const Generic = await compiled('Generic', '<script>let {children}=$props();</script><div>{@render children?.()}</div>', false);
const Button = await compiled('Button', '<script>let {children,href,type="button",variant,size,...rest}=$props();</script>{#if href}<a {href} {...rest}>{@render children?.()}</a>{:else}<button {type} {...rest}>{@render children?.()}</button>{/if}', false);
const Input = await compiled('Input', '<script>let {value=$bindable(),...rest}=$props();</script><input bind:value {...rest}/>', false);
const Textarea = await compiled('Textarea', '<script>let {value=$bindable(),...rest}=$props();</script><textarea bind:value {...rest}></textarea>', false);
const Identity = await compiled('Identity', read('ProtocolIdentityPanel'));
const Runtime = await compiled('Runtime', read('LibraryRuntime'));
const Library = await compiled('Library', read('LocalLibraryPanel'));
const MangaDiscussion = await compiled('MangaDiscussion', read('MangaDiscussion'));
const ReaderPanel = await compiled('ReaderPanel', compose('src/lib/components/reader/reader-panel.svelte'));
const LastPage = await compiled('LastPage', compose('src/lib/components/reader/reader-last-page.svelte'));
const Reader = await compiled('Reader', compose('src/lib/components/reader/reader.svelte'));
const Controller = await compiled('Controller', `<script>import Identity from './Identity.mjs';import Runtime from './Runtime.mjs';let kind=$state('manga'),id=$state('TitleOne01');export function navigate(nextKind,nextId){kind=nextKind;id=nextId;}</script><Runtime/><Identity resourceKind={kind} resourceId={id}/>`, false);

function deferred() { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
const recipient = { origin: 'http://127.0.0.1:2', serviceKey: 'a'.repeat(64), selected: true };
function environment({ missingThread = false, scopeStatus = 'verified' } = {}) {
  const calls = { opened: 0, toasts: [], mappings: [], social: [], reviews: [] };
  const records = [{ family: 'series', key: 'TitleOne01', id: 'TitleOne01', status: 'reading', notification: true }];
  const state = writable({ ready: true, mode: 'device', records, revision: 3, scopeStatus });
  const signerIdentity = writable({ owner: 'b'.repeat(64), generation: 1 });
  const thread = missingThread ? undefined : { id: 'NativeThread', relationships: { stats: {} } };
  const chapter = { id: 'Chapter001', title: 'Chapter', chapter: '1', relationships: { thread, groups: [], manga: { id: 'TitleOne01', title: 'Title', relationships: {} } } };
  const nativeComments = () => { if (missingThread) throw Error('Native comments mounted without a native thread'); return Generic(...arguments); };
  const env = {
    Button, Input, Textarea, ProtocolIdentityPanel: Identity, Panel: ReaderPanel, LastPage,
    localLibraryState: state, libraryIdentity: writable({ canUse: scopeStatus === 'verified' }), signerIdentity,
    message: cause => cause instanceof Error ? cause.message : String(cause),
    openLibrary: async () => { calls.opened++; }, toast: { error: (...args) => calls.toasts.push(args) },
    relayPreferences: async () => ({ relays: [recipient], revision: 1 }), publicActionRecords: async () => [], publicationJobs: async () => [],
    currentDeviceSnapshot: async () => ({ scope: { origin: 'http://127.0.0.1:1', catalogNamespace: null, serviceKey: null }, revision: 3, records }),
    capabilities: async () => ({ available: true, origin: recipient.origin, service_key: recipient.serviceKey }),
    createRouteActionOwner: () => ({ capture: () => ({ alive: () => true }) }),
    sameScope: () => true, statuses: ['reading', 'completed'], recordKey: (_family, row) => row.key,
    sourceResource: async (kind, id) => { calls.mappings.push([kind, id]); return { kind, id, event_id: id, publisher: 'c'.repeat(64), source_kind: kind === 'manga' ? 1002 : 1001 }; },
    readSocial: async (_relay, source) => { calls.social.push(source.id); return [{ id: source.id + '-comment', kind: 1111, content: 'From ' + source.id, pubkey: 'd'.repeat(64), created_at: 1 }]; },
    reviewComment: (relay, source, content) => { const value = { purpose: 'Comment', relay, source, owner: 'b'.repeat(64), generation: 1, kind: 1111, tags: [], content }; calls.reviews.push(value); return value; },
    reader: { chapter, prefs: writable({ layout: 'Scroll', panel_position: 'left', panel: true, gap: 0 }), pages: [], isViewingLastPage: true, isShowComments: false },
    getManga: () => () => ({ id: 'TitleOne01', relationships: { thread } }),
    useReader: () => env.reader, useComments: () => missingThread ? undefined : { thread, stats: {} },
    useSidebar: () => ({ open: true, openMobile: true, setOpen() {}, toggle() {} }),
    Comments: { Votes: missingThread ? nativeComments : Generic, Comments: missingThread ? nativeComments : Generic },
    Sidebar: Object.fromEntries(['Root', 'Outer', 'Header', 'Content', 'Separator'].map(name => [name, Generic])),
    Tabs: Object.fromEntries(['Root', 'List', 'Trigger', 'Content'].map(name => [name, Generic])),
    Constants: { Reader: { Layout: { Scroll: 'Scroll' }, CommentsVisibility: { Always: 'Always', Collapsed: 'Collapsed' } }, User: { Role: {}, RoleOrder: [] } },
    Utils: { chapterHeadline: () => 'Chapter', makeSlug: () => '' }, cn: () => '', pluralize: word => word,
    buttonVariants: () => '', isMobile: { current: false }, commentsVisibility: writable('Always'), skipEndOfChapterPage: writable(false), readerTab: writable('settings'),
    PUBLIC_SITE_NAME: 'Fixture',
  };
  const complete = new Proxy(env, { get(target, name) { if (name in target) return target[name]; return /^[A-Z]/.test(String(name)) ? Generic : async () => {}; } });
  return { env: complete, calls, state, records };
}
async function settle() { await new Promise(resolve => setTimeout(resolve, 0)); flushSync(); }
async function render(Component, env, props = {}) {
  globalThis.__relocationEnvironment = env;
  const target = document.createElement('div'); document.querySelector('main').append(target);
  const component = mount(Component, { target, props }); flushSync(); await settle();
  return { target, component, async close() { await unmount(component); target.remove(); } };
}
const button = (target, name) => { const matches = [...target.querySelectorAll('button')].filter(row => row.textContent.trim() === name); assert.equal(matches.length, 1, name); return matches[0]; };
async function click(target, name) { button(target, name).click(); await settle(); }
async function selectRecipient(target) { const select = target.querySelector('select'); select.value = recipient.origin; select.dispatchEvent(new Event('change', { bubbles: true })); await settle(); }

test('production route ownership has one invisible root, anonymous settings, and no global management', () => {
  const shell = compose('src/routes/+layout.svelte'); assert.equal((shell.match(/<LibraryRuntime\s*\/>/g) ?? []).length, 1);
  const app = compose('src/routes/(app)/+layout.svelte'); assert.doesNotMatch(app, /LocalLibraryPanel|ProtocolIdentityPanel/);
  const settings = compose('src/routes/(app)/settings/preferences/+page.svelte');
  assert.equal((settings.match(/<LocalLibraryPanel\s*\/>/g) ?? []).length, 1);
  assert.equal((settings.match(/<ProtocolIdentityPanel\s*\/>/g) ?? []).length, 1);
  assert(settings.lastIndexOf('</form>') < settings.indexOf('<LocalLibraryPanel />'));
  const library = compose('src/routes/(app)/library/+layout.svelte'); assert(library.indexOf('Manage library') < library.indexOf('<Protected>'));
  for (const route of ['src/routes/(app)/chapter/[id]/+page@.svelte', 'src/routes/(app)/chapter/[id]/[page]/+page@.svelte']) assert.doesNotMatch(compose(route), /LocalLibraryPanel|ProtocolIdentityPanel/);
  const title = compose('src/routes/(app)/title/Manga.svelte'); assert.match(title, /component: MangaDiscussion,\s*hidden: !isPublished/); assert.doesNotMatch(title, /<ProtocolIdentityPanel/);
  assert.match(compose('src/lib/components/reader/reader-settings.svelte'), /href="\/settings\/preferences#local-library">Manage library/);
});

test('real settings components retain anonymous failed-scope backups without outer accordions', async () => {
  const { env, records } = environment({ scopeStatus: 'migration_required' });
  const library = await render(Library, env);
  assert.equal(library.target.querySelector('section[data-local-library-panel]').id, 'local-library');
  assert.equal(library.target.querySelectorAll('details[data-local-library-panel]').length, 0);
  assert.equal(button(library.target, 'Export device backup').disabled, false);
  assert.equal(button(library.target, 'Use device library').disabled, true);
  assert.equal(library.target.querySelector('details').open, false, 'Technical scope details start collapsed');
  await library.close();
  const identity = await render(Identity, env);
  assert.equal(identity.target.querySelector('section[data-protocol-identity-panel]').id, 'public-identity');
  assert.equal(identity.target.querySelectorAll('[data-source-conversation]').length, 0);
  assert.equal(button(identity.target, 'Connect external signer').disabled, false);
  const summaries = [...identity.target.querySelectorAll('summary')].map(row => row.textContent);
  assert(summaries.includes('Public profile and follows'));
  assert(summaries.includes('Saved public actions'));
  assert.doesNotMatch(summaries.join('\n'), /follow\/relay lists|exact signed public-action outbox/);
  assert.equal(identity.target.querySelectorAll('input').length > 0, true);
  await identity.close();
  const reopened = await render(Library, env); assert.equal(records.length, 1); assert.match(reopened.target.textContent, /1 followed series/); await reopened.close();
});

test('home to title A to title B to reader keeps one context and clears drafts/reviews synchronously', async () => {
  const { env, calls } = environment(); const view = await render(Controller, env);
  assert.equal(view.target.querySelectorAll('[data-local-library-panel], [data-protocol-identity-panel]').length, 0);
  assert.equal(view.target.querySelectorAll('[data-source-conversation]').length, 1);
  assert.equal(calls.opened, 1); assert.deepEqual(calls.mappings, []);
  await click(view.target, 'Signed comments'); await selectRecipient(view.target); await click(view.target, 'Load verified public conversation');
  const comment = view.target.querySelector('textarea'); comment.value = 'Title A draft'; comment.dispatchEvent(new Event('input', { bubbles: true })); await settle();
  await click(view.target, 'Review public comment'); assert.match(view.target.textContent, /Title A draft/);
  const consent = view.target.querySelector('input[type="checkbox"]'); consent.checked = true; consent.dispatchEvent(new Event('change', { bubbles: true })); await settle();
  assert.equal(button(view.target, 'Sign and send reviewed public action').disabled, false);
  view.component.navigate('manga', 'TitleTwo02'); flushSync(); await settle();
  assert.equal(view.target.querySelector('[data-source-conversation]').dataset.resourceId, 'TitleTwo02');
  assert.doesNotMatch(view.target.textContent, /Title A draft|From TitleOne01/);
  assert.equal(view.target.querySelector('textarea'), null, 'No old source means no composer/review');
  await click(view.target, 'Load verified public conversation'); assert.equal(view.target.querySelector('textarea').value, '');
  view.component.navigate('chapter', 'Chapter001'); flushSync(); await settle();
  assert.equal(view.target.querySelector('[data-source-conversation]').dataset.resourceKind, 'chapter');
  assert.deepEqual(calls.mappings, [['manga', 'TitleOne01'], ['manga', 'TitleTwo02']]);
  assert.equal(calls.opened, 1, 'Root lifecycle stays mounted while targets change');
  window.dispatchEvent(new CustomEvent('weebdex-library-error', { detail: 'synthetic save failed' })); assert.equal(calls.toasts.length, 1);
  await view.close(); window.dispatchEvent(new CustomEvent('weebdex-library-error', { detail: 'after teardown' })); assert.equal(calls.toasts.length, 1);
});

test('late source read cannot attach after target navigation or disclosure dismissal', async () => {
  for (const reason of ['target', 'dismiss', 'unmount']) {
    const { env } = environment(), gate = deferred(); env.readSocial = () => gate.promise;
    const view = await render(Controller, env); await click(view.target, 'Signed comments'); await selectRecipient(view.target);
    button(view.target, 'Load verified public conversation').click(); await settle();
    if (reason === 'target') { view.component.navigate('manga', 'TitleTwo02'); flushSync(); }
    else if (reason === 'dismiss') { await click(view.target, 'Signed comments'); }
    else await view.close();
    gate.resolve([{ id: 'old-result', kind: 1111, content: 'Stale source text', pubkey: 'd'.repeat(64) }]); await settle();
    assert.doesNotMatch(view.target.textContent, /Stale source text/);
    if (reason !== 'unmount') await view.close();
  }
});

test('real compiled source-only title and reader mount without native account discussion context', async () => {
  const { env } = environment({ missingThread: true });
  const title = await render(MangaDiscussion, env); assert.equal(title.target.querySelectorAll('[data-source-conversation]').length, 1); await title.close();
  const reader = await render(Reader, env); assert.equal(reader.target.querySelectorAll('[data-source-conversation]').length, 1); assert.equal(reader.target.querySelectorAll('[data-local-library-panel], [data-protocol-identity-panel]').length, 0); await reader.close();
});

test('actual management unmount invalidates the lifetime callback passed with the immutable reviewed snapshot', async () => {
  const { env } = environment(); const gate = deferred(), entered = deferred(); let captured, consent, assertion, rejected = false;
  env.previewPublication = async () => ({ deviceRevision: 3, protocolRevision: 0, owner: 'b'.repeat(64), generation: 1, graph: { records: 0, root: { hash: 'a'.repeat(64) } }, excluded: [], recipients: [recipient] });
  env.publishReviewed = async (selected, approved, assertCurrent) => { captured = structuredClone(selected); consent = approved; assertion = assertCurrent; entered.resolve(); await gate.promise; try { assertCurrent(); } catch { rejected = true; } };
  const view = await render(Library, env);
  await click(view.target, 'Preview public snapshot');
  const label = [...view.target.querySelectorAll('label')].find(row => row.textContent.includes('I understand that this reviewed snapshot becomes public'));
  assert(label); const checkbox = label.querySelector('input'); checkbox.checked = true; checkbox.dispatchEvent(new Event('change', { bubbles: true })); await settle();
  button(view.target, 'Sign and publish reviewed snapshot').click(); await entered.promise;
  assert.equal(consent, true); assert.equal(captured.deviceRevision, 3); assert.doesNotThrow(assertion);
  await view.close(); assert.throws(assertion, /Library settings were closed/); gate.resolve(); await settle(); assert.equal(rejected, true);
});
