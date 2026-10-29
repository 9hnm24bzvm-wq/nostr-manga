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
import { fixture as managementFixture } from './library-management-fixture.mjs';
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
const LibraryTools = await compiled('LibraryTools', read('LibraryManagementTools'));
const IdentitySettings = await compiled('IdentitySettings', read('PublicIdentitySettings'));
// Explicit native-dialog stand-in: real compiled page owns open/generation and
// real tools own disposal. Actual bits-ui focus/portal/Escape is a browser gate.
const DialogRoot = await compiled('DialogRoot', `<script>let {open,onOpenChange,children}=$props();</script><svelte:window onkeydown={event=>{if(open && event.key==='Escape')onOpenChange(false)}}/>{#if open}<div role="dialog"><button onclick={()=>onOpenChange(false)}>Close</button>{@render children?.()}</div>{/if}`, false);
const MangaDiscussion = await compiled('MangaDiscussion', read('MangaDiscussion'));
const ReaderPanel = await compiled('ReaderPanel', compose('src/lib/components/reader/reader-panel.svelte'));
const LastPage = await compiled('LastPage', compose('src/lib/components/reader/reader-last-page.svelte'));
const Reader = await compiled('Reader', compose('src/lib/components/reader/reader.svelte'));
const Controller = await compiled('Controller', `<script>import Identity from './Identity.mjs';import Runtime from './Runtime.mjs';let kind=$state('manga'),id=$state('TitleOne01');export function navigate(nextKind,nextId){kind=nextKind;id=nextId;}</script><Runtime/><Identity resourceKind={kind} resourceId={id}/>`, false);

function deferred() { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
const recipient = { origin: 'http://127.0.0.1:2', serviceKey: 'a'.repeat(64), selected: true };
function environment({ missingThread = false, scopeStatus = 'verified', publicAccountLibraryUnavailable = false } = {}) {
  const calls = { opened: 0, toasts: [], mappings: [], social: [], reviews: [] };
  const records = [{ family: 'series', key: 'TitleOne01', id: 'TitleOne01', status: 'reading', notification: true }];
  const state = writable({ ready: true, mode: 'device', records, revision: 3, scopeStatus });
  const signerIdentity = writable({ owner: 'b'.repeat(64), generation: 1 });
  const thread = missingThread ? undefined : { id: 'NativeThread', relationships: { stats: {} } };
  const chapter = { id: 'Chapter001', title: 'Chapter', chapter: '1', relationships: { thread, groups: [], manga: { id: 'TitleOne01', title: 'Title', relationships: {} } } };
  const nativeComments = () => { if (missingThread) throw Error('Native comments mounted without a native thread'); return Generic(...arguments); };
  const env = {
    Session: writable(undefined),
    Button, Input, Textarea, ProtocolIdentityPanel: Identity, LibraryManagementTools: LibraryTools, Dialog: {Root: DialogRoot, Content: Generic, Header: Generic, Title: Generic, Description: Generic}, Panel: ReaderPanel, LastPage,
    publicUuidLibraryEnabled: false, publicAccountLibraryUnavailable, localLibraryState: state, libraryIdentity: writable({ canUse: scopeStatus === 'verified' }), signerIdentity,
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

test('production native Settings tabs own management and preserve account/client protections', () => {
  const shell = compose('src/routes/+layout.svelte'); assert.equal((shell.match(/<LibraryRuntime\s*\/>/g) ?? []).length, 1);
  for (const route of ['src/routes/(app)/+layout.svelte', 'src/routes/(app)/settings/preferences/+page.svelte']) assert.doesNotMatch(compose(route), /LocalLibraryPanel|ProtocolIdentityPanel|PublicIdentitySettings/);
  const settings = compose('src/routes/(app)/settings/+layout.svelte');
  assert.match(settings, /library: \{\s*name: "Library",\s*icon: LibraryIcon\s*\}/);
  assert.match(settings, /identity: \{\s*name: "Advanced"/);
  assert.match(settings, /account: \{[\s\S]*?hidden: !\$Session/); assert.match(settings, /client: \{[\s\S]*?hidden: !\$Session/);
  assert.match(settings, /<nav aria-label="Settings"/); assert.match(settings, /aria-current=\{page.url.pathname === pathname \? "page" : undefined\}/);
  const page = compose('src/routes/(app)/settings/library/+page.svelte'); assert.equal((page.match(/<LocalLibraryPanel\s*\/>/g) ?? []).length, 1);
  assert.match(page, /These imports and CSV exports use this device library/); assert.match(page, /\$localLibraryState.mode === "account" && !\$Session/);
  const layout = compose('src/routes/(app)/settings/library/+layout.svelte'); assert.doesNotMatch(layout, /Protected/); assert.match(layout, /\{@render children\?\.\(\)\}/);
  for (const route of ['src/routes/(app)/library/+layout.svelte', 'src/routes/(app)/history/+layout.svelte']) assert.match(compose(route), /<Protected>/);
  for (const route of ['src/routes/(app)/settings/account/+layout.svelte', 'src/routes/(app)/settings/client/+layout.svelte']) assert.equal(compose(route), fs.readFileSync(path.join(frontend, route), 'utf8'));
  const library = compose('src/routes/(app)/library/+layout.svelte'); assert(library.indexOf('Manage library') < library.indexOf('<Protected>'));
  assert.match(library, /href="\/settings\/library">Manage library/);
  assert.match(fs.readFileSync(path.join(root, 'platform-integrations/frontend-reconstruction/settings/identity/+page.svelte'), 'utf8'), /<PublicIdentitySettings \/>/);
  for (const route of ['src/routes/(app)/chapter/[id]/+page@.svelte', 'src/routes/(app)/chapter/[id]/[page]/+page@.svelte']) assert.doesNotMatch(compose(route), /LocalLibraryPanel|ProtocolIdentityPanel/);
  assert.match(compose('src/routes/(app)/title/Manga.svelte'), /component: MangaDiscussion,\s*hidden: !isPublished/);
  assert.match(compose('src/lib/components/reader/reader-settings.svelte'), /href="\/settings\/library">Manage library/);
});

test('real native Settings pages retain failed-scope backups without accordions or routine keys', async () => {
  const { env, records } = environment({ scopeStatus: 'migration_required' });
  const library = await render(Library, env);
  assert.equal(library.target.querySelectorAll('section[data-local-library-panel]').length, 1);
  assert.equal(library.target.querySelectorAll('details,summary,[role=dialog]').length, 0);
  assert.equal(button(library.target, 'Export device backup').disabled, false);
  assert.equal(button(library.target, 'Use device library').disabled, true);
  assert.doesNotMatch(library.target.textContent, /TitleOne01|canonical|namespace|Scope verification/);
  await library.close();
  const identity = await render(IdentitySettings, env);
  assert.equal(identity.target.querySelectorAll('section[data-protocol-identity-panel]').length, 1);
  assert.equal(identity.target.querySelectorAll('[data-source-conversation],input').length, 0);
  assert.equal(button(identity.target, 'Use a different identity').disabled, false);
  assert(!identity.target.textContent.includes('b'.repeat(64)));
  const advanced=identity.target.querySelector('[data-advanced-identity]');assert.equal(advanced.open,false);advanced.querySelector('summary').click();await settle();assert.equal(advanced.open,true);
  await click(identity.target, 'Edit public profile');
  assert.equal(identity.target.querySelectorAll('[data-public-identity-tool="profile"]').length, 1);
  assert(identity.target.querySelectorAll('input').length > 0);
  await click(identity.target, 'Close'); assert.equal(identity.target.querySelectorAll('[data-public-identity-tool]').length, 0);
  await identity.close();
  const reopened = await render(Library, env); assert.equal(records.length, 1); assert.match(reopened.target.textContent, /1 saved titles/); await reopened.close();
});

test('storage open failure never hides the mode, backup or recovery page', async () => {
  const {env}=environment();env.openLibrary=async()=>{throw Error('Device storage unavailable')};
  const view=await render(Library,env);assert.match(view.target.querySelector('[role=alert]').textContent,/Device storage unavailable/);
  for(const name of ['Export device backup','Use account library','Library recovery']) assert.equal(button(view.target,name).disabled,false);
  await view.close();
});

test('UUID storage failure preserves backup/recovery while unsupported native account mode stays absent', async () => {
  const {env}=environment({publicAccountLibraryUnavailable:true});env.openLibrary=async()=>{throw Error('Device storage unavailable')};
  const view=await render(Library,env);assert.match(view.target.querySelector('[role=alert]').textContent,/Device storage unavailable/);
  for(const name of ['Export device backup','Library recovery'])assert.equal(button(view.target,name).disabled,false);
  assert.equal([...view.target.querySelectorAll('button')].some(row=>row.textContent.trim()==='Use account library'),false);
  await view.close();
});

test('home to title A to title B to reader keeps one context and clears drafts/reviews synchronously', async () => {
  const { env, calls } = environment(); const view = await render(Controller, env);
  assert.equal(view.target.querySelectorAll('[data-local-library-panel], [data-protocol-identity-panel]').length, 0);
  assert.equal(view.target.querySelectorAll('[data-source-conversation]').length, 1);
  assert.equal(calls.opened, 1); assert.deepEqual(calls.mappings, []);
  await click(view.target, 'Comments from other sites'); await selectRecipient(view.target); await click(view.target, 'Load verified public conversation');
  const publishing=view.target.querySelector('[data-source-publishing]');assert.equal(publishing.open,false);publishing.querySelector('summary').click();await settle();assert.equal(publishing.open,true);
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
    const view = await render(Controller, env); await click(view.target, 'Comments from other sites'); await selectRecipient(view.target);
    button(view.target, 'Load verified public conversation').click(); await settle();
    if (reason === 'target') { view.component.navigate('manga', 'TitleTwo02'); flushSync(); }
    else if (reason === 'dismiss') { await click(view.target, 'Comments from other sites'); }
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

async function openPublication(view) { await click(view.target, 'Set up public sync'); await click(view.target, 'Publish library'); await click(view.target, 'Preview public snapshot'); const label=[...view.target.querySelectorAll('label')].find(row=>row.textContent.includes('I understand that this reviewed snapshot becomes public'));assert(label);const checkbox=label.querySelector('input');checkbox.checked=true;checkbox.dispatchEvent(new Event('change',{bubbles:true}));await settle(); }
async function dismiss(view, reason) { if(reason==='route') await view.close();else if(reason==='task-switch') await click(view.target,'Back to public sync');else if(reason==='Close') await click(view.target,'Close');else {window.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Escape',bubbles:true}));await settle();} }

for (const [reason,gateAt] of [['Close','worker'],['Escape','snapshot'],['Close','proof'],['Escape','final-read'],['route','snapshot'],['task-switch','snapshot']]) test(`actual compiled task ${reason} joins original durable signing/transport fence at ${gateAt}`, {timeout:5000}, async()=>{
  const f=await managementFixture({gateAt});let view,sending,guard;
  try{
    const {env,state}=environment();state.update(value=>({...value,revision:0}));env.signerIdentity=f.publication.signerIdentity;
    env.previewPublication=async()=>f.review;
    env.publishReviewed=(selected,approved,assertCurrent)=>{guard=assertCurrent;sending=f.publication.publishReviewed(selected,approved,assertCurrent);return sending;};
    view=await render(Library,env);await openPublication(view);button(view.target,'Sign and publish reviewed snapshot').click();
    await f.entered;assert.doesNotThrow(guard);await dismiss(view,reason);assert.throws(guard,/closed/i);
    f.release();await sending.catch(()=>{});await settle();
    const jobs=await f.protocol.jobs();assert.equal(jobs.length,gateAt==='worker'?0:1);assert.equal(f.calls.posts,0);
    if(jobs.length){assert(f.nostr.verifyEvent(jobs[0].event));assert.deepEqual(JSON.parse(jobs[0].event.content),{manifest:f.graph.root.hash,version:1});const batch=await f.protocol.jobBatch(jobs[0].id,0);assert.deepEqual([...batch.objects[0].data],[...f.graph.objects[0].data]);}
    if(gateAt==='worker')assert.deepEqual(f.calls.kinds,[]);if(gateAt==='snapshot')assert.deepEqual(f.calls.kinds,[11999]);if(['proof','final-read'].includes(gateAt))assert.deepEqual(f.calls.kinds,[11999,27235]);
    if(reason!=='route'){assert.equal(view.target.querySelectorAll('[data-library-tool]').length,reason==='task-switch'?1:0);if(reason==='task-switch')assert(view.target.querySelector('[data-library-tool=sync]'));await view.close();view=undefined;}
    else view=undefined;
  }finally{f.release();if(view)await view.close();await f.dispose();}
});

for(const reason of ['Close','Escape'])test(`actual recovery task ${reason} stops a held fresh proof before transport`,{timeout:5000},async()=>{
  const f=await managementFixture({gateAt:'recovery-proof'});let view,operation;
  try{const {env}=environment();env.signerIdentity=f.publication.signerIdentity;env.previewRecovery=(relay,assertCurrent)=>operation=f.publication.previewRecovery(relay,assertCurrent);
    view=await render(Library,env);await click(view.target,'Set up public sync');await click(view.target,'Recover public library');button(view.target,'Preview verified public recovery').click();await f.entered;await dismiss(view,reason);f.release();await assert.rejects(operation,/closed/i);assert.equal(f.calls.posts,0);assert.deepEqual(f.calls.kinds,[27235]);assert.equal((await f.protocol.jobs()).length,0);await view.close();view=undefined;
  }finally{f.release();if(view)await view.close();await f.dispose();}
});

for(const reason of ['Close','Escape','route-owner'])test(`closed Lists held read cannot change mode or navigate: ${reason}`,{timeout:5000},async()=>{
  const {env,records,state}=environment();const list='e'.repeat(32);records.push({family:'lists',key:list,id:list,name:'My saved list'});state.update(value=>({...value,records}));
  const gate=deferred(),entered=deferred();let hold=false,alive=true,modes=0,navigations=0;const snapshot=env.currentDeviceSnapshot;
  env.createRouteActionOwner=()=>({capture:()=>({alive:()=>alive})});env.currentDeviceSnapshot=async()=>{const value=await snapshot();if(hold){entered.resolve();await gate.promise;}return value;};env.useMode=async()=>{modes++};env.goto=async()=>{navigations++};
  const view=await render(Library,env);try{await click(view.target,'Manage lists');assert.equal(button(view.target,'Create list').disabled,false);hold=true;view.target.querySelector(`a[href="/library?list=${list}"]`).dispatchEvent(new window.MouseEvent('click',{bubbles:true,cancelable:true}));await entered.promise;
    if(reason==='route-owner')alive=false;else await dismiss(view,reason);gate.resolve();await settle();await settle();assert.equal(modes,0);assert.equal(navigations,0);if(reason==='route-owner')assert.equal(view.target.querySelectorAll('[data-library-tool="lists"]').length,1);
  }finally{gate.resolve();await view.close();}
});


test('current metadata read remains inspectable while routine action rows hide raw proof', async()=>{
  const {env}=environment();
  env.metadataForEditing=async(relay,owner,kind)=>({headGuard:true,relay,owner,kind,head:{id:'c'.repeat(64),projected:true},event:{id:'c'.repeat(64),kind,pubkey:owner,tags:[],content:kind===0?JSON.stringify({about:'Saved profile content'}):'',created_at:1}});
  env.profileFields=base=>JSON.parse(base.event.content);env.relayLines=()=>[];
  let view=await render(IdentitySettings,env);view.target.querySelector('[data-advanced-identity] summary').click();await settle();await click(view.target,'Edit public profile');await selectRecipient(view.target);await click(view.target,'Load my current information');
  assert(!view.target.textContent.includes('Saved profile content'));await click(view.target,'Inspect current data');assert(view.target.textContent.includes('Saved profile content'));await view.close();
  env.publicActionRecords=async()=>[{event:{id:'d'.repeat(64),kind:1111,pubkey:'b'.repeat(64),tags:[],content:'Reviewed comment',created_at:1},relay:recipient,state:'prepared',message:'Saved on this device',cancelled:false}];
  view=await render(IdentitySettings,env);view.target.querySelector('[data-advanced-identity] summary').click();await settle();await click(view.target,'Saved public actions');const card=view.target.querySelector('[data-public-action-id]');assert.equal(card.dataset.publicActionId,'d'.repeat(64));assert.match(card.textContent,/Ready to send/);assert(!card.textContent.includes('d'.repeat(64)));await click(view.target,'Inspect saved action');assert(card.textContent.includes('Reviewed comment'));assert(card.textContent.includes('d'.repeat(64)));await view.close();
});

test('external source comments can be read without a signer, with posting kept in an explicit advanced disclosure',async()=>{const{env,calls}=environment();env.signerIdentity.set({owner:null,generation:2});const view=await render(Controller,env);try{assert.equal(view.target.querySelector('textarea'),null);assert.equal(calls.social.length,0);await click(view.target,'Comments from other sites');await selectRecipient(view.target);await click(view.target,'Load verified public conversation');assert.match(view.target.textContent,/From TitleOne01/);assert.match(view.target.textContent,/separate from the conversation above/);assert.equal(view.target.querySelector('[data-source-publishing]').open,false);assert.equal(button(view.target,'Review public comment').disabled,true);assert.equal(button(view.target,'Review public reaction').disabled,true);assert.equal(calls.reviews.length,0);await click(view.target,'Comments from other sites');assert.equal(view.target.querySelector('textarea'),null);assert.equal(calls.reviews.length,0);}finally{await view.close();}});

test('advanced identity page leads with the one site account and closing its disclosure dismisses signing tools',async()=>{const {env,calls}=environment();env.Session.set({identity:{id:'Reader1234',username:'Reader',is_verified:true}});const view=await render(IdentitySettings,env);try{assert.equal(view.target.querySelector('h1').textContent,'Account');assert.ok(view.target.querySelector('a[href="/settings/account"]'));assert.ok(view.target.querySelector('a[href="/user/Reader1234"]'));assert.ok(view.target.querySelector('a[href="/auth/logout"]'));const advanced=view.target.querySelector('[data-advanced-identity]');assert.equal(advanced.open,false);await click(view.target,'Edit public profile');assert.equal(view.target.querySelector('[role="dialog"]'),null);advanced.querySelector('summary').click();await settle();await click(view.target,'Edit public profile');assert.ok(view.target.querySelector('[role="dialog"]'));advanced.querySelector('summary').click();await settle();assert.equal(view.target.querySelector('[role="dialog"]'),null);assert.equal(calls.reviews.length,0);}finally{await view.close();}});
