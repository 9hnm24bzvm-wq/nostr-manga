#!/usr/bin/env node
// Actual selection-helper regression seam. Native role, popup attachment and
// strict Locator behavior are explicit DOM/driver stand-ins, not browser proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectListboxTrigger, clickListboxOption, setListboxOptions } from '../../scripts/browser-select-controls.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
let cases = 0, nativePackageAudited = false;
function check(name, operation) { operation(); cases++; }
async function verify(name, operation) { await operation(); cases++; }
check('retained Combobox aliases the retained Select trigger', () => {
  const aliases = fs.readFileSync(path.join(root, 'frontend/src/lib/components/ui/combobox/index.ts'), 'utf8');
  const trigger = fs.readFileSync(path.join(root, 'frontend/src/lib/components/ui/select/select-trigger.svelte'), 'utf8');
  assert.match(aliases, /import Trigger from "\$lib\/components\/ui\/select\/select-trigger\.svelte"/);
  assert.match(trigger, /<SelectPrimitive\.Trigger/); assert.match(trigger, /data-slot="select-trigger"/);
  assert.doesNotMatch(trigger, /role="combobox"/);
});
check('preference page can have an unrelated mounted command input', () => {
  const preference = fs.readFileSync(path.join(root, 'frontend/src/routes/(app)/settings/preferences/+page.svelte'), 'utf8');
  const tags = fs.readFileSync(path.join(root, 'frontend/src/lib/components/TagComboBox.svelte'), 'utf8');
  const popup = fs.readFileSync(path.join(root, 'frontend/src/lib/components/ui/combobox/combobox-content.svelte'), 'utf8');
  assert.match(preference, /<TagComboBox bind:value=\{draft\.tags\}/);
  assert.match(tags, /<Command\.Input placeholder="Search tags\.\.\."/);
  assert.match(popup, /<Command\.Input placeholder="Search\.\.\."/);
});
check('each repaired caller is bound to its retained Select-based control', () => {
  const journey = fs.readFileSync(path.join(root, 'scripts/browser-ui11-16-journeys.mjs'), 'utf8');
  const read = file => fs.readFileSync(path.join(root, 'frontend/src', file), 'utf8');
  const preferences = read('routes/(app)/settings/preferences/+page.svelte');
  const reader = read('lib/components/reader/reader-page-selector.svelte');
  const chapterFilter = read('lib/components/ChapterListFilter.svelte');
  const follow = read('routes/(app)/title/MangaActions.svelte');
  assert.match(preferences, /<Combobox.Root type="multiple" bind:value=\{draft.translated_languages\}/);
  assert.match(preferences, /<Combobox.Root type="multiple" bind:value=\{draft.languages\}/);
  assert.match(preferences, /<Select.Root type="single" bind:value=\{draft.items_per_page\}/);
  assert.match(reader, /<Combobox.Root/); assert.match(reader, /<Combobox.Trigger/);
  assert.match(chapterFilter, /<Combobox.Root/); assert.match(chapterFilter, /Any Language/);
  assert.match(follow, /<SelectPrimitive.Trigger[^>]+data-slot="select-trigger"/);
  assert(journey.includes('selectOptions(selectListboxTrigger(section), label ? [label] : [])'));
  assert(journey.includes('const readerSelector = () => selectListboxTrigger(page).filter('));
  assert(journey.includes('selectListboxTrigger(page).filter({ hasText: /^\\s*Reading\\s*$/ })'));
  assert(journey.includes('const trigger = () => selectListboxTrigger(form).first()'));
  assert(journey.includes('selectOptions(selectListboxTrigger(languages), ["Spanish"])'));
  assert(journey.includes('selectOptions(selectListboxTrigger(languages), [])'));
  assert(journey.includes('selectListboxTrigger(page).filter({ hasText: /Items Per Page:/ })'));
  assert.doesNotMatch(journey, /getByRole\("combobox"/);
});
if (process.env.WEEBDEX_SELECT_SOURCE_NODE_MODULES) check('pinned native package renders button without invented combobox role', () => {
  const dependency = path.resolve(process.env.WEEBDEX_SELECT_SOURCE_NODE_MODULES, 'bits-ui');
  assert.equal(JSON.parse(fs.readFileSync(path.join(dependency, 'package.json'))).version, '2.16.5');
  const state = fs.readFileSync(path.join(dependency, 'dist/bits/select/select.svelte.js'), 'utf8');
  const start = state.indexOf('export class SelectTriggerState '), end = state.indexOf('export class SelectContentState ', start);
  const native = state.slice(start, end), props = native.slice(native.lastIndexOf('props = $derived.by'));
  assert.match(props, /"aria-haspopup": "listbox"/); assert.doesNotMatch(props, /\brole\s*:/);
  const content = state.slice(end, state.indexOf('export class SelectItemState ', end)); assert.match(content, /role: "listbox"/);
  const component = fs.readFileSync(path.join(dependency, 'dist/bits/select/components/select-trigger.svelte'), 'utf8');
  assert.match(component, /<button \{\.\.\.mergedProps\}>/);
  nativePackageAudited = true;
});

function fixture({ popups, open = true, triggerText = 'Translated Languages', extraNodes = [] } = {}) {
  const tagInput = { kind: 'input', value: 'keep tag search' }, calls = [], page = {};
  const popup = (visible, labels, selected = [], input = true) => ({ kind: 'popup', visible, children: [
    ...(input ? [{ kind: 'input', value: '' }] : []),
    ...labels.map(label => ({ kind: 'option', text: label, selected: selected.includes(label) }))
  ] });
  const allPopups = popups ?? [popup(false, ['Spanish'], ['Spanish']), popup(false, ['English', 'Spanish', 'Spanish (LATAM)'], ['English'])];
  const intended = allPopups.at(-1);
  const trigger = { kind: 'trigger', text: triggerText, role: 'button', onClick() { if (open) intended.visible = true; } };
  const children = node => node.kind === 'page' ? [tagInput, trigger, ...extraNodes, ...allPopups] : node.children ?? [];
  const descendants = node => children(node).flatMap(child => [child, ...descendants(child)]);
  function matches(node, selector) {
    if (selector === 'button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible') return node.kind === 'trigger' && node.visible !== false;
    if (selector === '[data-slot="select-content"][role="listbox"]:visible') return node.kind === 'popup' && node.visible;
    if (selector === '[data-slot="command-input"]') return node.kind === 'input';
    if (selector === '[data-slot="select-item"]') return node.kind === 'option';
    if (selector === '[data-slot="select-item"][aria-selected="true"]') return node.kind === 'option' && node.selected;
    throw Error('Unexpected selector: ' + selector);
  }
  class Locator {
    constructor(read) { this.read = read; }
    locator(selector) { return new Locator(() => this.read().flatMap(descendants).filter(node => matches(node, selector))); }
    filter({ hasText }) { return new Locator(() => this.read().filter(node => hasText.test(node.text ?? ''))); }
    first() { return new Locator(() => this.read().slice(0, 1)); }
    async count() { return this.read().length; }
    single() { const rows = this.read(); if (rows.length !== 1) throw Error('strict locator mismatch'); return rows[0]; }
    async waitFor({ state }) { assert.equal(state, 'visible'); this.single(); }
    async fill(value) { const node = this.single(); assert.equal(node.kind, 'input'); node.value = value; calls.push(['fill', node, value]); }
    async click() { const node = this.single(); calls.push(['click', node]); if (node.kind === 'trigger') node.onClick(); else if (node.kind === 'option') node.selected = !node.selected; else throw Error('Unexpected click'); }
  }
  const locator = new Locator(() => [{ kind: 'page' }]);
  page.locator = selector => locator.locator(selector);
  page.keyboard = { async press(key) { assert.equal(key, 'Escape'); calls.push(['escape']); for (const popup of allPopups) popup.visible = false; } };
  page.getByRole = role => new Locator(() => descendants({ kind: 'page' }).filter(node => node.role === role));
  return { page, trigger: selectListboxTrigger(page), tagInput, popups: allPopups, intended, calls, popup };
}

await verify('old combobox-role assumption misses actual native trigger', async () => {
  const f = fixture(); assert.equal(await f.page.getByRole('combobox').count(), 0); assert.equal(await f.trigger.count(), 1);
});
await verify('trigger helper ignores hidden Select triggers and unrelated genuine ARIA comboboxes', async () => {
  const f = fixture({ triggerText: '3 / 3', extraNodes: [{ kind: 'trigger', visible: false, text: '3 / 3' }, { kind: 'other', role: 'combobox', text: '3 / 3' }] });
  assert.equal(await f.trigger.count(), 1);
  assert.equal(await f.page.getByRole('combobox').count(), 1);
  const journey = fs.readFileSync(path.join(root, 'scripts/browser-ui11-16-journeys.mjs'), 'utf8');
  const expression = journey.match(/const readerSelector = \(\) => ([^;]+);/)[1];
  const actualReaderSelector = new Function('page', 'selectListboxTrigger', 'return ' + expression)(f.page, selectListboxTrigger);
  assert.equal(await actualReaderSelector.count(), 1);
});
await verify('old page-global command input is genuinely ambiguous once popup opens', async () => {
  const f = fixture(); await f.trigger.click(); assert.equal(await f.page.locator('[data-slot="command-input"]').count(), 3);
  await assert.rejects(f.page.locator('[data-slot="command-input"]').fill(''), /strict locator mismatch/);
});
await verify('actual helper selects Spanish without touching unrelated or closed-popup input', async () => {
  const f = fixture(); await setListboxOptions(f.page, f.trigger, ['Spanish']);
  assert.deepEqual(f.intended.children.filter(node => node.selected).map(node => node.text), ['Spanish']);
  assert.equal(f.tagInput.value, 'keep tag search'); assert.equal(f.popups[0].children[0].value, '');
  assert.equal(f.popups[0].children.find(node => node.kind === 'option').selected, true);
  assert.equal(f.calls.filter(call => call[0] === 'escape').length, 1);
});
await verify('empty desired selection clears current choices and requires a live popup', async () => {
  const f = fixture(); await setListboxOptions(f.page, f.trigger, []); assert.equal(f.intended.children.some(node => node.selected), false);
});
await verify('multiple desired languages preserve exact labels including regexp punctuation', async () => {
  const f = fixture(); await setListboxOptions(f.page, f.trigger, ['Spanish', 'Spanish (LATAM)']);
  assert.deepEqual(f.intended.children.filter(node => node.selected).map(node => node.text), ['Spanish', 'Spanish (LATAM)']);
});
await verify('missing active popup fails even for empty selection', async () => {
  const f = fixture({ open: false }); await assert.rejects(setListboxOptions(f.page, f.trigger, []), /strict locator mismatch/);
  assert.equal(f.calls.some(call => call[0] === 'fill' || call[0] === 'escape'), false);
});
await verify('ambiguous active popups fail before any selection or search', async () => {
  const base = fixture(); const a = base.popup(true, ['Spanish']), b = base.popup(false, ['Spanish']);
  const f = fixture({ popups: [a, b] }); await assert.rejects(setListboxOptions(f.page, f.trigger, ['Spanish']), /strict locator mismatch/);
  assert.equal(f.calls.some(call => call[0] === 'fill' || call[0] === 'escape'), false);
});
await verify('closed popup cannot satisfy a missing active option', async () => {
  const base = fixture(); const f = fixture({ popups: [base.popup(false, ['Spanish']), base.popup(false, ['English'])] });
  await assert.rejects(setListboxOptions(f.page, f.trigger, ['Spanish']), /strict locator mismatch/);
});
await verify('duplicate options in active popup remain a strict failure', async () => {
  const base = fixture(); const f = fixture({ popups: [base.popup(false, ['Spanish', 'Spanish'])] });
  await assert.rejects(setListboxOptions(f.page, f.trigger, ['Spanish']), /strict locator mismatch/);
});
await verify('plain Select without command input still selects through its active popup', async () => {
  const base = fixture(); const f = fixture({ popups: [base.popup(false, ['20', '50'], ['20'], false)] });
  await setListboxOptions(f.page, f.trigger, ['50']); assert.deepEqual(f.intended.children.filter(node => node.selected).map(node => node.text), ['50']);
});
await verify('single-option helper scopes away a closed matching popup', async () => {
  const f = fixture(); await f.trigger.click(); await clickListboxOption(f.page, 'Spanish');
  assert.equal(f.intended.children.find(node => node.text === 'Spanish').selected, true);
});
await verify('single-option helper also rejects ambiguous active popups with only one matching option', async () => {
  const base = fixture(); const f = fixture({ popups: [base.popup(true, ['English']), base.popup(false, ['Spanish'])] });
  await f.trigger.click(); await assert.rejects(clickListboxOption(f.page, 'Spanish'), /strict locator mismatch/);
  assert.equal(f.calls.filter(call => call[0] === 'click').length, 1);
});
await verify('single-option helper rejects a closed-only matching option', async () => {
  const f = fixture({ open: false }); await assert.rejects(clickListboxOption(f.page, 'Spanish'), /strict locator mismatch/);
});
console.log(JSON.stringify({ status: 'passed', cases, native_package_audited: nativePackageAudited, boundary: 'Actual helper execution with explicit native DOM/strict-locator stand-ins and retained source checks; no real browser, network, service or database.' }));
