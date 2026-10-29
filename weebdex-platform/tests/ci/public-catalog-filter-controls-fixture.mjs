// Finite mounted Svelte boundary: actual overlay-composed retained filters,
// actual input/input-group controls, public-language control, parsing and client.
// JSDOM, router/store context, decorative primitives and select/popover shells
// stand in for the browser/SvelteKit/bits-ui. No server or real network is used.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createVmModuleLoader } from './vm-module-fixture.mjs';
import { createPublicSearchModeFixture } from './public-search-mode-fixture.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend'), directory = path.join(root, 'platform-integrations/public-api');
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT, domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert.ok(dependencies && domModules, 'Use existing WEEBDEX_FRONTEND_ROOT and WEEBDEX_BROWSER_DOM_NODE_MODULES; do not install dependencies');
const require = createRequire(path.join(dependencies, 'package.json'));
const { compile, compileModule } = require('svelte/compiler');
assert.equal(require('svelte/package.json').version, '5.55.1');
assert.equal(require(path.join(domModules, 'jsdom/package.json')).version, '27.0.1');
const { JSDOM } = require(path.join(domModules, 'jsdom'));
const dom = new JSDOM('<main id="search"></main>', { url: 'https://fixture.invalid/search', pretendToBeVisual: true });
export const window = dom.window;
const savedGlobals = new Map();
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLMediaElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) {
  savedGlobals.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
  globalThis[key] = window[key];
}
export class BrowserSearchParams extends URLSearchParams {}
const context = vm.createContext({ URL, URLSearchParams: BrowserSearchParams, AbortController, TextEncoder, TextDecoder, structuredClone, console, fetch() { throw Error('Unexpected real network'); } });
const layout = fs.readFileSync(path.join(frontend, 'src/routes/+layout.ts'), 'utf8');
vm.runInContext(stripTypeScriptTypes(layout.slice(layout.indexOf('const _toString ='), layout.indexOf('Number.prototype.short ='))), context);
savedGlobals.set('URLSearchParams', Object.getOwnPropertyDescriptor(globalThis, 'URLSearchParams'));
globalThis.URLSearchParams = BrowserSearchParams;
export const searchMode = createPublicSearchModeFixture(context);
const loader = createVmModuleLoader({
  createModule(file, importModuleDynamically) { return searchMode.module(file) ?? new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: file, importModuleDynamically }); },
  resolve(specifier, importer) {
    const boundary = searchMode.resolve(specifier, importer); if (boundary) return boundary;
    const base = specifier.startsWith('$lib/') ? path.join(frontend, 'src/lib', specifier.slice(5)) : path.resolve(path.dirname(importer.identifier), specifier);
    return base.endsWith('.ts') ? base : base + '.ts';
  }
});
export const moduleNamespace = name => loader.namespace(path.join(directory, name + '.ts'));
export const query = await moduleNamespace('query'), normalize = await moduleNamespace('normalize'), searchQuery = await moduleNamespace('manga-search-query');
const { PublicApiClient } = await moduleNamespace('client'), { RetainedCatalogueClient } = await moduleNamespace('site-client');
const Constants = { Manga: await loader.namespace(path.join(frontend, 'src/lib/constants/Manga.ts')), MangaFollow: await loader.namespace(path.join(frontend, 'src/lib/constants/MangaFollow.ts')) };
const Locale = await loader.namespace(path.join(frontend, 'src/lib/Locale.ts'));
const svelteUrl = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const svelte = await import(svelteUrl), { mount, unmount, flushSync } = svelte;
const { writable } = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/index-client.js')));
const generation = fs.mkdtempSync(path.join(os.tmpdir(), 'catalog-filter-controls-'));
fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(generation, 'node_modules'));
function writeModule(name, code) { const file = path.join(generation, name + '.mjs'); fs.writeFileSync(file, code); return import(pathToFileURL(file)); }
async function component(name, source) { return (await writeModule(name, compile(source, { filename: name + '.svelte', generate: 'client', runes: true }).js.code)).default; }
const { page } = await writeModule('route-state', compileModule('export const page=$state({url:new URL("https://fixture.invalid/search"),title:undefined});', { filename: 'route-state.svelte.js', generate: 'client' }).js.code);
export function composed(relative) {
  const file = path.join(frontend, relative); let source = fs.readFileSync(file, 'utf8');
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled: true })]) source = plugin.transform.handler(source, file)?.code ?? source;
  return source;
}
// Import substitution is confined to documented environment/UI boundaries.
// Production root scripts and templates are compiled unchanged otherwise.
function bindings(source, expression) {
  const names = new Set();
  source = source.replace(/^\s*import\s+([\s\S]*?)\s+from\s+['"][^'"]+['"];\s*$/gm, (whole, clause) => {
    if (clause.startsWith('type ')) return '';
    if (clause.startsWith('* as ')) names.add(clause.slice(5).trim());
    else if (clause.startsWith('{')) for (const part of clause.slice(1, -1).split(',')) { const text = part.trim(); if (!text || text.startsWith('type ')) continue; names.add(text.split(/\s+as\s+/).at(-1)); }
    else names.add(clause.trim());
    return '';
  });
  return source.replace(/<script lang="ts">/, `<script lang="ts">\nconst {${[...names].join(',')}}=${expression};\n`);
}
const common = { ...svelte, ...await moduleNamespace('language'), Constants, Locale, page, cn: (...values) => values.filter(value => typeof value === 'string').join(' '), equal: require('fast-deep-equal'), buttonVariants: () => '', ...searchQuery };
globalThis.__catalogFilterEnvironment = common;
const shell = await component('shell', '<script>let {children,child}=$props();</script><div>{#if child}{@render child({props:{}})}{:else}{@render children?.()}{/if}</div>');
const empty = await component('empty', '<span aria-hidden="true"></span>');
const Button = await component('button', '<script>let {children,type="button",disabled=false,onclick}=$props();</script><button {type} {disabled} {onclick}>{@render children?.()}</button>');
const Label = await component('label', '<script>let {children,for:htmlFor}=$props();</script><label for={htmlFor}>{@render children?.()}</label>');
const Checkbox = await component('checkbox', '<script>let {checked=$bindable(false),id}=$props();</script><input type="checkbox" {id} bind:checked />');
const selectRoot = await component('select-root', `<script>import {setContext} from ${JSON.stringify(svelteUrl)};let {value=$bindable([]),type="multiple",disabled=false,onValueChange,children}=$props();setContext('finite-select',{get value(){return value},get disabled(){return disabled},choose(next){value=type==='multiple'?(value.includes(next)?value.filter(item=>item!==next):[...value,next]):next;onValueChange?.(value);}});</script><div data-fixture-select>{@render children?.()}</div>`);
const selectItem = await component('select-item', `<script>import {getContext} from ${JSON.stringify(svelteUrl)};let {value,children}=$props();const selection=getContext('finite-select');</script><button type="button" role="option" data-value={value} disabled={selection.disabled} aria-selected={Array.isArray(selection.value)?selection.value.includes(value):selection.value===value} onclick={()=>selection.choose(value)}>{@render children?.()}</button>`);
const select = { Root: selectRoot, Trigger: Button, Content: shell, Item: selectItem };
const Input = await component('input', bindings(fs.readFileSync(path.join(frontend, 'src/lib/components/ui/input/input.svelte'), 'utf8'), 'globalThis.__catalogFilterEnvironment'));
common.Input = Input;
const InputGroupInput = await component('input-group-input', bindings(fs.readFileSync(path.join(frontend, 'src/lib/components/ui/input-group/input-group-input.svelte'), 'utf8'), 'globalThis.__catalogFilterEnvironment'));
const PublicLanguageFilter = await component('public-language-filter', bindings(fs.readFileSync(path.join(directory, 'PublicLanguageFilter.svelte'), 'utf8'), 'globalThis.__catalogFilterEnvironment'));
Object.assign(common, { Button, Label, Checkbox, InputGroup: { Root: shell, Addon: shell, Input: InputGroupInput }, Combobox: select, Select: select, RadioGroup: select, Popover: { Root: shell, Trigger: Button, Content: shell }, Command: { Group: shell, Item: shell }, ImageLoader: empty, ViewModeToggles: empty, TagComboBox: shell, PublicLanguageFilter, ChevronDownIcon: empty, SearchIcon: empty, ArrowDownWideNarrow: empty, ArrowUpNarrowWide: empty });
const MangaSearchFilter = await component('MangaSearchFilter', bindings(composed('src/lib/components/MangaSearchFilter.svelte'), 'globalThis.__catalogFilterEnvironment'));
let chapterComponent,chapterVersion=0;
export async function prepareChapterComponent(extra = {},legacy = false) {
  Object.assign(common, extra);
  chapterComponent = await component('ChapterListFilter'+(++chapterVersion), bindings(legacy?fs.readFileSync(path.join(root,'tests/fixtures/catalog-filter-controls/ChapterListFilter-composed-68a5e06.svelte'),'utf8'):composed('src/lib/components/ChapterListFilter.svelte'), 'globalThis.__catalogFilterEnvironment'));
}
export async function productionComponent(name, extra = {}) {
  Object.assign(common, extra);
  return component(name, bindings(fs.readFileSync(path.join(directory, name + '.svelte'), 'utf8'), 'globalThis.__catalogFilterEnvironment'));
}
export async function snapshotComponent(name,file){return component(name,bindings(fs.readFileSync(path.join(root,'tests/fixtures/catalog-filter-controls',file),'utf8'),'globalThis.__catalogFilterEnvironment'));}
export const uuid = value => '00000000-0000-8000-8000-' + String(value).padStart(12, '0');
export const titleRow = (attributes = {}) => ({ id: uuid(1), type: 'manga', attributes: { title: { en: 'Actual title' }, originalLanguage: 'ja', contentRating: 'safe', status: 'ongoing', availableTranslatedLanguages: ['en', 'fr'], ...attributes }, relationships: [] });
export const envelope = (data, { total = data.length, limit = data.length, offset = 0 } = {}) => ({ status: 200, body: { result: 'ok', response: 'collection', data, total, limit, offset } });
export function fixture(handler = () => envelope([])) {
  const requests = [], api = new PublicApiClient(async request => { requests.push(request); return handler(request); });
  const bridge = new RetainedCatalogueClient(api);
  return { requests, api, read: (operation, query = new BrowserSearchParams(), id) => bridge.read(operation, { query, id }), readOptions: (operation, options) => bridge.read(operation, options) };
}
export async function settle() { for (let index = 0; index < 6; index++) { await new Promise(resolve => setImmediate(resolve)); flushSync(); } }
export function button(host, text) { const found = [...host.querySelectorAll('button')].find(node => node.textContent.trim() === text); assert.ok(found, 'button ' + text); return found; }
export function field(host, placeholder) { const found = [...host.querySelectorAll('input')].find(node => node.placeholder === placeholder); assert.ok(found, 'input ' + placeholder); return found; }
export function input(node, value) { node.value = value; node.dispatchEvent(new window.Event('input', { bubbles: true })); flushSync(); }
export function submit(host) { host.querySelector('form').dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true })); flushSync(); }
export function reset(host) { host.querySelector('form').dispatchEvent(new window.Event('reset', { bubbles: true, cancelable: true })); flushSync(); }
export const plain = value => JSON.parse(JSON.stringify(value));
export async function ui({ path: route = '/search', chapter = false, title = normalize.normalizeManga(titleRow()), feed = { total: 0, data: [] }, preferences = {}, environment = {} } = {}) {
  const routes = [], errors = [], host = document.createElement('main');
  const navigate = value => { window.history.replaceState({}, '', value); page.url = new URL(window.location.href); flushSync(); };
  navigate(route); page.title = title;
  Object.assign(common, { prefs: writable({ items_per_page: '42', show_safe: true, show_suggestive: true, show_erotic: true, show_hentai: false, languages: [], tags: [], translated_languages: [], groups: [], ...preferences }), populateTagList: async () => {}, getManga: () => () => page.title, goto: value => { routes.push(value); navigate(value); }, toast: { error: (...args) => errors.push(args) }, ...environment });
  document.body.append(host);
  const instance = mount(chapter ? chapterComponent : MangaSearchFilter, { target: host, props: { feed, isShowFilter: true } });
  await settle();
  return { host, routes, errors, feed, navigate, setTitle(next) { page.title = next; flushSync(); }, async dispose() { await unmount(instance); host.remove(); } };
}
// Exact old-source counterfactual for the real public language control. The
// retained search model starts langx undefined; Svelte forbids binding undefined
// to a child prop with a non-undefined $bindable fallback.
export async function oldLanguageDefaultError() {
  const source = fs.readFileSync(path.join(directory, 'PublicLanguageFilter.svelte'), 'utf8');
  assert.equal(source.split('value=$bindable()').length, 2);
  common.OldPublicLanguageFilter = await component('OldPublicLanguageFilter', bindings(source.replace('value=$bindable()', 'value=$bindable([])'), 'globalThis.__catalogFilterEnvironment'));
  const Wrapper = await component('OldLanguageBinding', '<script>const {OldPublicLanguageFilter}=globalThis.__catalogFilterEnvironment;let value=$state();</script><OldPublicLanguageFilter bind:value label="Excluded original languages" />');
  const host = document.createElement('main'); document.body.append(host); let instance;
  try { instance = mount(Wrapper, { target: host }); return undefined; } catch (error) { return error; }
  finally { if (instance) await unmount(instance); host.remove(); delete common.OldPublicLanguageFilter; }
}
export function cleanup() {
  dom.window.close(); fs.rmSync(generation, { recursive: true, force: true }); delete globalThis.__catalogFilterEnvironment;
  for (const [key, descriptor] of savedGlobals) if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key];
}
