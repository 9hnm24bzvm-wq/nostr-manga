#!/usr/bin/env node
// Source-derived DOM/accessibility and signer-control seams. JSDOM supplies no
// browser layout. The real pinned Playwright naming/text functions and composed
// sources are executed; driver actions and store subscriptions are stand-ins.
// No browser, fixture, network, database, build or dependency install is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { titleLibraryTrigger, titleLibraryControlName, nativeSelectForLabel } from '../../scripts/browser-select-controls.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend'), read = relative => fs.readFileSync(path.join(root, relative), 'utf8');
const file = path.join(frontend, 'src/routes/(app)/title/MangaActions.svelte');
let composed = fs.readFileSync(file, 'utf8');
for (const factory of [createFrontendOverlays, createReaderOverlays, createLocalLibraryOverlays]) composed = factory(frontend).transform.handler(composed, file)?.code ?? composed;
const trigger = composed.match(/<SelectPrimitive\.Trigger\b[\s\S]*?<\/SelectPrimitive\.Trigger>/)?.[0];
assert.ok(trigger);
assert.match(composed, /<div class="min-w-45">\s*\{#if \$libraryIdentity.canUse\}/);
assert.match(trigger, /data-slot="select-trigger"/);
assert.match(trigger, /<span class="w-0 flex-auto">[\s\S]*?Add to Library[\s\S]*?<\/span>/);
assert.match(trigger, /<Spinner[\s\S]*?isUpdatingReadingStatus && "scale-100 "/);
assert.doesNotMatch(trigger, /aria-hidden|\{#if isUpdatingReadingStatus\}/);
const spinner = read('frontend/src/lib/components/ui/spinner/spinner.svelte');
assert.match(spinner, /<Loader2Icon role="status" aria-label="Loading"/);
const label = spinner.match(/aria-label="([^"]+)"/)[1];

const require = createRequire(import.meta.url);
const domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES ?? path.join(root, '../manga-protocol-lab/browser-client/node_modules');
assert.equal(JSON.parse(fs.readFileSync(path.join(domModules, 'jsdom/package.json'))).version, '27.0.1');
assert.equal(createHash('sha256').update(fs.readFileSync(path.join(domModules, 'jsdom/lib/api.js'))).digest('hex'), 'b4feb8ae099e1683da79137cf484a0952915e54d239140f124ecd7df5439b033');
const { JSDOM } = require(path.join(domModules, 'jsdom'));
const core = process.env.WEEBDEX_PLAYWRIGHT_CORE_ROOT ?? '/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright-core';
assert.equal(JSON.parse(fs.readFileSync(path.join(core, 'package.json'))).version, '1.62.1');
const bundle = fs.readFileSync(path.join(core, 'lib/coreBundle.js'), 'utf8');
const assignment = bundle.split('\n').find(line => line.trimStart().startsWith('source4 = '));
assert.ok(assignment, 'The reviewed generated injected-source assignment must be present');
const injected = vm.runInNewContext('let source4;' + assignment + '; source4');
assert.equal(createHash('sha256').update(injected).digest('hex'), 'fc43c7ab27d27b4e2241190a6f7a990089328dc95ca29785eb54ed59159f6315');

function domPage(status = 'Add to Library', { copies = 1, hiddenCopy = false } = {}) {
  // The static fragment mirrors the source-checked title/button/span/Spinner
  // topology above. CSS transform supplies the actual scale-0 accessibility case.
  const markup = () => `<div class="min-w-45"><button data-slot="select-trigger" aria-haspopup="listbox"><span> ${status} </span><div><svg role="status" aria-label="${label}" style="transform:scale(0)"></svg></div></button></div>`;
  const { dom, window } = makeDOM(markup().repeat(copies) + (hiddenCopy ? '<div hidden>' + markup() + '</div>' : '') + '<button data-slot="select-trigger" aria-haspopup="listbox">Reading</button><div class="min-w-45"><button aria-haspopup="dialog">Reading</button></div>');
  const events = [];
  const locator = rows => ({
    async count() { return rows.length; },
    filter({ hasText }) { return locator(rows.filter(row => hasText.test(window.naming.elementText(new Map(), row).full))); },
    async waitFor({ state }) { assert.equal(state, 'visible'); assert.equal(rows.length, 1, 'strict unique title control'); },
    async click() { assert.equal(rows.length, 1, 'strict unique title control'); events.push(rows[0]); },
  });
  const page = {
    locator(selector) { assert.equal(selector, 'div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible'); return locator([...window.document.querySelectorAll(selector.replace(':visible', ''))].filter(row => !row.closest('[hidden]'))); },
    getByRole(role, options) { return locator([...window.document.querySelectorAll('button')].filter(row => window.naming.getAriaRole(row) === role && options.name.test(window.naming.getElementAccessibleNameText(row)))); },
  };
  return { dom, window, page, events };
}

function makeDOM(markup) {
  const dom = new JSDOM(markup, { runScripts: 'outside-only' }), { window } = dom;
  const computed = window.getComputedStyle.bind(window);
  // These sources have no generated pseudo-element content. JSDOM does not
  // implement pseudo-element style lookup, so return only that explicit fact.
  window.getComputedStyle = (element, pseudo) => pseudo ? { content: 'none' } : computed(element);
  window.module = { exports: {} };
  window.eval(injected + ';window.naming = {getElementAccessibleNameText, getAriaRole, elementText, getElementLabels, textIsEngine};');
  return { dom, window };
}

test('actual Playwright naming proves scale-zero Spinner breaks the former exact role name', () => {
  for (const status of ['Add to Library', 'Reading', 'Plan to Read']) {
    const value = domPage(status);
    const button = value.window.document.querySelector('button');
    assert.equal(value.window.naming.getElementAccessibleNameText(button), status + ' Loading');
    assert.equal(value.window.naming.elementText(new Map(), button).normalized, status);
    assert.equal(titleLibraryControlName.test(value.window.naming.getElementAccessibleNameText(button)), false);
    value.dom.window.close();
  }
});

test('shared title selector retains exact visible status and strict structural uniqueness', async () => {
  for (const status of ['Add to Library', 'Reading', 'Plan to Read', 'On Hold', 'Completed', 'Dropped', 'Re-reading']) {
    const value = domPage(status, { hiddenCopy: true });
    assert.equal(await titleLibraryTrigger(value.page).count(), 1);
    await titleLibraryTrigger(value.page, status).click();
    assert.equal(value.events.length, 1);
    assert.equal(await titleLibraryTrigger(value.page, 'Reading|Plan to Read').count(), 0, 'Explicit status strings must not become regex alternatives');
    value.dom.window.close();
  }
  for (const copies of [0, 2]) {
    const value = domPage('Reading', { copies });
    await assert.rejects(titleLibraryTrigger(value.page).click(), /strict unique title control/);
    assert.equal(value.events.length, 0);
    value.dom.window.close();
  }
});

test('native wrapped select uses exact immediate label despite descendant option text', async () => {
  const panel = read('platform-integrations/local-library/LibraryManagementTools.svelte');
  assert.match(panel, /<label>List <select[^>]+bind:value=\{memberList\}/);
  assert.match(panel, /<label>Title <select[^>]+bind:value=\{memberSeries\}/);
  const identity = read('platform-integrations/local-library/ProtocolIdentityPanel.svelte');
  assert.match(identity, /<label>Verified recipient <select[^>]+bind:value=\{selectedOrigin\}/);
  assert.doesNotMatch(identity, /Target type|Current instance catalogue ID/);
  assert.match(panel, /<label>Status for saved \{row.id\}<select/);
  const cases = [
    ['List', 'list-id', 'Choose list', 'Local example'],
    ['Title', 'SeedManga1', 'Choose a saved title', 'SeedManga1 (reading)'],
    ['Verified recipient', 'http://127.0.0.1:1', 'Choose instance', 'http://127.0.0.1:1 · public-key'],
    ['Status for saved SeedManga1', 'completed', 'reading', 'completed'],
    ['Status for saved SeedManga10', 'on_hold', 'reading', 'on_hold'],
  ];
  const fragment = cases.map(([name, value, placeholder, choice]) => `<label>${name} <select><option value="">${placeholder}</option><option value="${value}">${choice}</option></select></label>`).join('');
  const value = makeDOM(fragment), calls = [];
  const scope = {
    locator(selector) {
      const match = selector.match(/^label:text-is\((.*)\) > select$/); assert.ok(match);
      const name = JSON.parse(match[1]);
      const rows = [...value.window.document.querySelectorAll('label')].filter(label => value.window.naming.textIsEngine.matches(label, [name], undefined, { _cacheText: new Map() })).flatMap(label => [...label.children].filter(child => child.tagName === 'SELECT'));
      return { async count() { return rows.length; }, async selectOption(selected) { assert.equal(rows.length, 1, 'strict unique labelled native select'); assert([...rows[0].options].some(option => option.value === selected)); rows[0].value = selected; calls.push([name, selected]); } };
    }
  };
  for (const [index, [name, selected]] of cases.entries()) {
    const control = [...value.window.document.querySelectorAll('select')][index];
    const labels = value.window.naming.getElementLabels(new Map(), control);
    assert.equal(labels.some(label => label.normalized === name), false, 'The former exact getByLabel must fail on actual wrapped option text');
    assert(labels[0].normalized.startsWith(name + ' '));
    await nativeSelectForLabel(scope, name).selectOption(selected);
  }
  assert.deepEqual(calls, cases.map(([name, selected]) => [name, selected]));
  value.window.document.body.insertAdjacentHTML('beforeend', fragment);
  await assert.rejects(nativeSelectForLabel(scope, 'List').selectOption('list-id'), /strict unique labelled native select/);
  assert.equal(await nativeSelectForLabel(scope, 'List name').count(), 0);
  const journeys = read('scripts/browser-journeys.mjs');
  assert.match(journeys, /nativeSelectForLabel\(target, 'List'\)\.selectOption\(list.id\)/);
  assert.match(journeys, /nativeSelectForLabel\(target, 'Title'\)\.selectOption\(manga\)/);
  value.dom.window.close();
});

test('account recovery and ordinary follow still use shared controls and durable saved rows', () => {
  const journeys = read('scripts/browser-journeys.mjs');
  assert.match(journeys, /const follow = status => selectTitleLibraryStatus\(page, status\)/);
  assert.match(journeys, /await follow\("Reading"\); localBefore = await until\(device/);
  assert.match(journeys, /row\.family === "series" && row\.id === manga/);
  const panel = read('platform-integrations/local-library/LibraryManagementTools.svelte');
  assert.match(panel, /const series = \$derived\(\$localLibraryState.records.filter\(row => row.family === 'series'\)\)/);
  assert.match(panel, /Choose a saved title<\/option>\{#each series.slice\(titlePage \* 100, \(titlePage \+ 1\) \* 100\) as row\}<option value=\{row.id\}/);
  assert.match(read('platform-integrations/local-library/library.ts'), /if \(!\(await request\(rows.get\(\['series', series\]\)\)\)\) throw Error\('Choose an existing scope-bound saved series'\)/);
  assert.match(journeys, /row\.family === 'members' && row\.list === list.id && row\.series === manga && row\.position === 0/);
});

function signerModule(window) {
  const publication = read('platform-integrations/local-library/publication.ts');
  const operation = publication.slice(publication.indexOf('export async function connectSigner('), publication.indexOf('\nexport function disconnectSigner'));
  const clear = publication.slice(publication.indexOf('function clearSelectedSigner()'), publication.indexOf('\n// The draft owns its key'));
  assert(operation.startsWith('export async function connectSigner('));
  const request = read('platform-integrations/local-library/protocol/nostr.js').match(/export async function signerRequest\(operation\) \{[\s\S]*?\n\}/)[0];
  const identity = { value: { owner: null, generation: 0 }, set(value) { this.value = value; } };
  const context = vm.createContext({ window, signerIdentity: identity, get: value => value.value, performance, setTimeout, clearTimeout });
  vm.runInContext(stripTypeScriptTypes('let signer = null, clearLocalSigner = null;\n' + clear + '\n' + request.replace('export ', '') + '\n' + operation.replace('export ', '') + '\n globalThis.connectSigner = connectSigner;'), context);
  return { context, identity };
}

test('ordinary dismissal binding reaches the actual signer prompt without authorizing signing', async () => {
  const journeys = read('scripts/browser-journeys.mjs');
  const binding = journeys.match(/inspect\('signer-binding-installed', \(\) => page.evaluate\((\(\) => \{ window.nostr = [^\n]+?\})\)\)/)?.[1];
  assert.ok(binding, 'Execute the exact ordinary journey binding');
  const window = {}, module = signerModule(window);
  vm.runInContext('(' + binding + ')()', module.context);
  let prompts = 0, signs = 0;
  const getKey = window.nostr.getPublicKey, sign = window.nostr.signEvent;
  window.nostr.getPublicKey = async () => { prompts++; return getKey(); };
  window.nostr.signEvent = async (...args) => { signs++; return sign(...args); };
  await assert.rejects(module.context.connectSigner(), /Synthetic signer prompt dismissed/);
  assert.equal(prompts, 1); assert.equal(signs, 0);
  assert.equal(module.identity.value.owner, null); assert.equal(module.identity.value.generation, 1);
  assert.match(journeys, /insist\(await outboxCount\(\) === 0, 'Dismissed signer created publication jobs'\)/);
});

test('former incomplete binding is rejected before the prompt, proving harness cause', async () => {
  let prompts = 0;
  const module = signerModule({ nostr: { async getPublicKey() { prompts++; throw Error('Synthetic signer prompt dismissed'); } } });
  await assert.rejects(module.context.connectSigner(), /No compatible NIP-07 browser signer was found\. Create or import a key/);
  assert.equal(prompts, 0); assert.equal(module.identity.value.generation, 0);
});

test('account import preview follows the actual explicit source without changing review authority', () => {
  const journeys = read('scripts/browser-journeys.mjs'), library = read('platform-integrations/local-library/library.ts'), panel = read('platform-integrations/local-library/LocalLibraryPanel.svelte');
  const source = library.match(/source: '(Explicit account-library import)'/)[1];
  assert.match(panel, /Import preview: \{imported.source\}/);
  const text = 'Import preview: ' + source + ' · 1 rows';
  assert.equal(text.includes('Import preview: account'), false);
  assert(text.includes('Import preview: Explicit account-library import'));
  assert.match(journeys, /getByText\('Import preview: Explicit account-library import', \{ exact: false \}\)/);
  assert.match(library, /previewAccountImport[\s\S]*?assertCatalogueScope[\s\S]*?authoritativeAccount/);
  assert.match(library, /applyImport[\s\S]*?accountId[\s\S]*?authoritativeAccount/);
  assert.match(journeys, /getByRole\('button', \{ name: 'Merge reviewed import', exact: true \}\)\.click\(\)/);
});
