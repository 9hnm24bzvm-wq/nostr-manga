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
import { titleLibraryTrigger, titleLibraryControlName } from '../../scripts/browser-select-controls.mjs';

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
  const dom = new JSDOM(markup().repeat(copies) + (hiddenCopy ? '<div hidden>' + markup() + '</div>' : '') + '<button data-slot="select-trigger" aria-haspopup="listbox">Reading</button><div class="min-w-45"><button aria-haspopup="dialog">Reading</button></div>', { runScripts: 'outside-only' });
  const { window } = dom;
  const computed = window.getComputedStyle.bind(window);
  // These sources have no generated pseudo-element content. JSDOM does not
  // implement pseudo-element style lookup, so return only that explicit fact.
  window.getComputedStyle = (element, pseudo) => pseudo ? { content: 'none' } : computed(element);
  window.module = { exports: {} };
  window.eval(injected + ';window.naming = {getElementAccessibleNameText, getAriaRole, elementText};');
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

test('account recovery and ordinary follow still use shared controls and durable saved rows', () => {
  const journeys = read('scripts/browser-journeys.mjs');
  assert.match(journeys, /const follow = status => selectTitleLibraryStatus\(page, status\)/);
  assert.match(journeys, /await follow\("Reading"\); localBefore = await until\(device/);
  assert.match(journeys, /row\.family === "series" && row\.id === manga/);
  const panel = read('platform-integrations/local-library/LocalLibraryPanel.svelte');
  assert.match(panel, /const series = \$derived\(\$localLibraryState.records.filter\(row => row.family === 'series'\)\)/);
  assert.match(panel, /Choose saved series<\/option>\{#each series as row\}<option value=\{row.id\}/);
  assert.match(read('platform-integrations/local-library/library.ts'), /if \(!\(await request\(rows.get\(\['series', series\]\)\)\)\) throw Error\('Choose an existing scope-bound saved series'\)/);
  assert.match(journeys, /row\.family === 'members' && row\.list === list.id && row\.series === manga && row\.position === 3/);
});

function signerModule(window) {
  const publication = read('platform-integrations/local-library/publication.ts');
  const operation = publication.match(/export async function connectSigner\(\) \{[\s\S]*?\n\}/)[0];
  const request = read('platform-integrations/local-library/protocol/nostr.js').match(/export async function signerRequest\(operation\) \{[\s\S]*?\n\}/)[0];
  const identity = { value: { owner: null, generation: 0 }, set(value) { this.value = value; } };
  const context = vm.createContext({ window, signerIdentity: identity, get: value => value.value, performance, setTimeout, clearTimeout });
  vm.runInContext(stripTypeScriptTypes('let signer = null;\n' + request.replace('export ', '') + '\n' + operation.replace('export ', '') + '\n globalThis.connectSigner = connectSigner;'), context);
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
  await assert.rejects(module.context.connectSigner(), /compatible NIP-07 browser signer is required/);
  assert.equal(prompts, 0); assert.equal(module.identity.value.generation, 0);
});
