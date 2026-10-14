#!/usr/bin/env node
// Source-derived DOM/accessibility seams, not Chromium/Svelte/network acceptance.
// Uses the same pinned Playwright naming/text implementation as the reviewed
// title-control seam. No browser, fixture, build or dependency install starts.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { actorFollowHeader, actorFollowControl } from '../../scripts/browser-actor-follow-controls.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend'), read = relative => fs.readFileSync(path.join(root, relative), 'utf8');
const composed = kind => {
  const file = path.join(frontend, `src/routes/(app)/${kind.toLowerCase()}/[id]/${kind}.svelte`);
  let source = fs.readFileSync(file, 'utf8');
  for (const factory of [createFrontendOverlays, createReaderOverlays, createLocalLibraryOverlays]) source = factory(frontend).transform.handler(source, file)?.code ?? source;
  return source;
};
const spinner = read('frontend/src/lib/components/ui/spinner/spinner.svelte');
assert.match(spinner, /<Loader2Icon role="status" aria-label="Loading"/);
const label = spinner.match(/aria-label="([^"]+)"/)[1];
const nativeButton = read('frontend/src/lib/components/ui/button/button.svelte');
assert.match(nativeButton, /<button\s[\s\S]*?data-slot="button"[\s\S]*?\{disabled\}[\s\S]*?\{@render children\?\.\(\)\}/);
for (const kind of ['Group', 'User']) {
  const source = composed(kind), follow = source.match(/<Button\s[^>]*onclick=\{updateFollowStatus\}[\s\S]*?<\/Button>/)?.[0];
  const block = source.match(/<Button\s[^>]*onclick=\{updateBlockStatus\}[\s\S]*?<\/Button>/)?.[0];
  assert.ok(follow); assert.ok(block);
  assert.match(source, /<section class="space-y-2">\s*<header[\s\S]*?<h1/);
  assert.match(source, /let isUpdatingFollowStatus = \$derived\(\$followState.loading \|\| \$followState.updating\)/);
  assert.match(follow, /disabled=\{\$mutex \|\| \$followState.loading \|\| \$followState.updating \|\| \(!\$followState.ready && !\$followState.error\)\}/);
  assert.match(follow, /\{#if isUpdatingFollowStatus\}\s*<Spinner \/>[\s\S]*?\{#if \$followState.error\}\s*Retry follow status\s*\{:else if isFollowed\}\s*Unfollow\s*\{:else\}\s*Follow/);
  assert.match(block, /<BanIcon \/>\s*\{#if isBlocked\}\s*Unblock\s*\{:else\}\s*Block/);
  assert.doesNotMatch(block, /<Spinner/);
  assert.doesNotMatch(follow + block, /aria-label|aria-hidden/);
}

const require = createRequire(import.meta.url);
const domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES ?? path.join(root, '../manga-protocol-lab/browser-client/node_modules');
assert.equal(JSON.parse(fs.readFileSync(path.join(domModules, 'jsdom/package.json'))).version, '27.0.1');
assert.equal(createHash('sha256').update(fs.readFileSync(path.join(domModules, 'jsdom/lib/api.js'))).digest('hex'), 'b4feb8ae099e1683da79137cf484a0952915e54d239140f124ecd7df5439b033');
const { JSDOM } = require(path.join(domModules, 'jsdom'));
const core = process.env.WEEBDEX_PLAYWRIGHT_CORE_ROOT ?? '/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright-core';
assert.equal(JSON.parse(fs.readFileSync(path.join(core, 'package.json'))).version, '1.62.1');
const bundle = fs.readFileSync(path.join(core, 'lib/coreBundle.js'), 'utf8');
const assignment = bundle.split('\n').find(line => line.trimStart().startsWith('source4 = '));
assert.ok(assignment);
const injected = vm.runInNewContext('let source4;' + assignment + '; source4');
assert.equal(createHash('sha256').update(injected).digest('hex'), 'fc43c7ab27d27b4e2241190a6f7a990089328dc95ca29785eb54ed59159f6315');

function domPage(text, { busy = false, disabled = false, copies = 1, hiddenCopy = false, duplicateHeader = false } = {}) {
  // Mirrors the source-checked native header/button/Spinner/text topology.
  const button = () => `<button data-slot="button" ${disabled ? 'disabled' : ''}>${busy ? `<svg role="status" aria-label="${label}"></svg>` : '<svg aria-hidden="true"></svg>'} \n ${text} \n </button>`;
  const header = () => `<section><header><h1>Public entity</h1>${button().repeat(copies)}${hiddenCopy ? '<div hidden>' + button() + '</div>' : ''}<button data-slot="button">Follow other</button><a data-slot="button">${text}</a><button>${text}</button></header></section>`;
  const dom = new JSDOM(header() + (duplicateHeader ? header() : '') + `<header><h1>Outside section</h1>${button()}</header><section><header>${button()}</header></section><div>${button()}</div>`, { runScripts: 'outside-only' }), { window } = dom;
  const computed = window.getComputedStyle.bind(window);
  window.getComputedStyle = (element, pseudo) => pseudo ? { content: 'none' } : computed(element);
  window.module = { exports: {} };
  window.eval(injected + ';window.naming = {getElementAccessibleNameText, getAriaRole, elementText};');
  const events = [];
  const locator = rows => ({
    filter({ has, hasText }) { return locator(rows.filter(row => has ? row.querySelector('h1') : hasText.test(window.naming.elementText(new Map(), row).full))); },
    locator(selector) { assert.equal(selector, 'button[data-slot="button"]:visible'); return locator(rows.flatMap(row => [...row.querySelectorAll(selector.replace(':visible', ''))]).filter(row => !row.closest('[hidden]'))); },
    async count() { return rows.length; },
    async waitFor({ state }) { assert.equal(state, 'visible'); assert.equal(rows.length, 1, 'strict unique actor control'); },
    async click() { assert.equal(rows.length, 1, 'strict unique actor control'); assert.equal(rows[0].disabled, false, 'native disabled control'); events.push(rows[0]); },
    async isDisabled() { assert.equal(rows.length, 1, 'strict unique actor control'); return rows[0].disabled; },
    async isEnabled() { assert.equal(rows.length, 1, 'strict unique actor control'); return !rows[0].disabled; },
  });
  const page = { locator(selector) { if (selector === 'h1') return { selector }; assert.equal(selector, 'section > header:visible'); return locator([...window.document.querySelectorAll('section > header')].filter(row => !row.closest('[hidden]'))); } };
  return { dom, window, page, events };
}

for (const text of ['Follow', 'Unfollow', 'Retry follow status', 'Block', 'Unblock']) {
  for (const disabled of [false, true]) test(`actual pinned naming and exact actor label: ${text}, disabled=${disabled}`, async () => {
    const busy = disabled && ['Follow', 'Unfollow'].includes(text);
    const value = domPage(text, { busy, disabled, hiddenCopy: true });
    const actual = value.window.document.querySelector('button');
    assert.equal(value.window.naming.getElementAccessibleNameText(actual), (busy ? 'Loading ' : '') + text);
    assert.equal(value.window.naming.elementText(new Map(), actual).normalized, text);
    if (busy) assert.notEqual(value.window.naming.getElementAccessibleNameText(actual), text, 'Former exact role-name control cannot match the busy native button');
    const control = actorFollowControl(value.page, text);
    assert.equal(await control.count(), 1); await control.waitFor({ state: 'visible' });
    assert.equal(await control.isDisabled(), disabled); assert.equal(await control.isEnabled(), !disabled);
    if (disabled) await assert.rejects(control.click(), /native disabled control/); else await control.click();
    assert.equal(value.events.length, disabled ? 0 : 1);
    assert.equal(await actorFollowControl(value.page, text + '|other').count(), 0, 'Explicit labels must not become regex alternatives');
    value.dom.window.close();
  });
}

for (const options of [{ copies: 0 }, { copies: 2 }, { duplicateHeader: true }]) test('missing/duplicate native actor controls fail closed: ' + JSON.stringify(options), async () => {
  const value = domPage('Follow', options);
  await assert.rejects(actorFollowControl(value.page, 'Follow').waitFor({ state: 'visible' }), /strict unique actor control/);
  await assert.rejects(actorFollowControl(value.page, 'Follow').click(), /strict unique actor control/);
  assert.equal(value.events.length, 0); value.dom.window.close();
});

test('all actor action labels use the strict header/text helper, with original hold/count invariants', () => {
  const source = read('scripts/browser-actor-follow-journeys.mjs');
  assert.match(source, /const header = \(\) => actorFollowHeader\(page\)/);
  assert.match(source, /const button = name => actorFollowControl\(page, name\)/);
  assert.doesNotMatch(source, /getByRole\('button'/);
  assert.match(source, /await button\('Retry follow status'\)\.dblclick\(\); await retried.wait\(\)/);
  assert.match(source, /await clickDisabledAgain\('Follow'\)/);
  assert.match(source, /await page.mouse.click\([\s\S]*?clickCount: 2/);
  assert.match(source, /pending.nativeStatus === 204/);
  assert.match(source, /watch.counts.POST === 1 && watch.counts.DELETE === 0 && watch.counts.PUT === 0/);
  assert.match(source, /watch.counts.GET === beforeReentry \+ 1/);
});
