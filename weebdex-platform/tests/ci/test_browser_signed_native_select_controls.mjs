#!/usr/bin/env node
// Actual signed native-select call-site seams. Immediate label text, native
// selects and strict driver actions are explicit stand-ins; pinned Playwright
// label/text engine behavior is checked by the shared source-seam test. This
// does not establish browser, IndexedDB, signer or recipient acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { nativeSelectForLabel } from '../../scripts/browser-select-controls.mjs';

const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const panel = name => fs.readFileSync(new URL('../../platform-integrations/local-library/' + name + '.svelte', import.meta.url), 'utf8');
const libraryMarkup = panel('LocalLibraryPanel'), identityMarkup = panel('ProtocolIdentityPanel');
const calls = [...source.matchAll(/nativeSelectForLabel\((library\(\)|identity\(\)), ("[^"]*"(?: \+ manga)?)\)\.selectOption\(([^;\n]+?)\)/g)];
assert.equal(calls.length, 8, 'Every signed native select call must use the reviewed helper');
assert.doesNotMatch(source, /getByLabel\([^;\n]+?\)\.selectOption\(/, 'Exact descendant-contaminated native labels must not remain');
const expected = [
  ['identity()', 'Verified recipient', 'protocol'],
  ['identity()', 'Target type', 'kind'],
  ['library()', 'List', 'deviceList.id'],
  ['library()', 'Series', 'manga'],
  ['identity()', 'Verified recipient', 'second.origin'],
  ['identity()', 'Verified recipient', 'protocol'],
  ['identity()', 'Verified recipient', 'second.origin'],
  ['library()', 'Status for saved Synthetic1', '"completed"'],
];

test('all actual signed native-select calls retain their exact scope, label and option value', async () => {
  const protocol = 'http://127.0.0.1:4101', second = { origin: 'http://127.0.0.1:4102' };
  const manga = 'Synthetic1', deviceList = { id: 'b'.repeat(32) }, kind = 'chapter';
  const selected = [];
  for (const [index, call] of calls.entries()) {
    const label = vm.runInNewContext(call[2], { manga });
    assert.deepEqual([call[1], label, call[3]], expected[index]);
    const value = vm.runInNewContext(call[3], { protocol, second, manga, deviceList, kind });
    const scope = { locator(selector) {
      assert.equal(selector, 'label:text-is(' + JSON.stringify(label) + ') > select');
      return { async selectOption(option) { assert.equal(option, value); selected.push([label, option]); } };
    } };
    const otherScope = { locator() { assert.fail('Native select escaped its intended signed panel'); } };
    const context = { nativeSelectForLabel, library: () => call[1] === 'library()' ? scope : otherScope,
      identity: () => call[1] === 'identity()' ? scope : otherScope, protocol, second, manga, deviceList, kind };
    await vm.runInNewContext('(async () => ' + call[0] + ')()', context);
  }
  assert.equal(selected.length, 8);
  assert.equal(selected.filter(([label]) => label === 'Verified recipient').length, 4);
});

test('actual panel markup uses implicit labels that also contain native option text', () => {
  for (const label of ['List', 'Series']) assert.match(libraryMarkup, new RegExp('<label>' + label + ' <select[^>]+>[\\s\\S]*?<option'));
  assert.match(libraryMarkup, /<label>Status for saved \{row.id\}<select[^>]+>[\s\S]*?<option/);
  for (const label of ['Verified recipient', 'Target type']) assert.match(identityMarkup, new RegExp('<label>' + label + ' <select[^>]+>[\\s\\S]*?<option'));
});

test('native select helper retains strict selection for missing or duplicate signed labels', async () => {
  for (const count of [0, 2]) {
    let mutations = 0;
    const scope = { locator(selector) {
      assert.equal(selector, 'label:text-is("Verified recipient") > select');
      return { async selectOption() { assert.equal(count, 1, 'strict native recipient control'); mutations++; } };
    } };
    await assert.rejects(nativeSelectForLabel(scope, 'Verified recipient').selectOption('http://127.0.0.1:4101'), /strict native recipient control/);
    assert.equal(mutations, 0);
  }
});
