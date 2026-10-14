#!/usr/bin/env node
// Finite signed-journey call-site and locator seam. Locator rows are explicit
// stand-ins. Pinned package/retained title semantics are verified separately by
// test_browser_select_controls.mjs. This does not establish browser acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { selectTitleLibraryStatus, titleLibraryTrigger } from '../../scripts/browser-select-controls.mjs';

const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
assert.match(source, /import \{ selectTitleLibraryStatus, titleLibraryTrigger \} from "\.\/browser-select-controls\.mjs";/);
const followSource = source.match(/const follow = ([^\n]+);/);
assert.ok(followSource, 'Signed journey must keep its actual shared-helper call site');
const negativeSource = source.match(/insist\((await titleLibraryTrigger\(page\)\.count\(\) === 0), "(Current catalogue title offered native save controls under a mismatched old scope)"\);/);
assert.ok(negativeSource, 'The existing migration fence must count the real title Trigger');
assert.doesNotMatch(source, /getByRole\(["']combobox["']/);

function pageStandIn(titleNames = []) {
  const events = [], rows = titleNames.map(name => ({ role: 'button', name, title: true, popup: 'listbox' }));
  rows.push({ role: 'button', name: 'Reading', title: false, popup: 'listbox' });
  rows.push({ role: 'button', name: 'Add to Library', title: true, popup: 'dialog' });
  rows.push({ role: 'combobox', name: 'Chapter language', title: false });
  function locator(matches) {
    return {
      rows: matches,
      and(other) { return locator(matches.filter(row => other.rows.includes(row))); },
      async count() { return matches.length; },
      async click() { assert.equal(matches.length, 1, 'No ambiguous or missing control may be silently selected'); events.push(matches[0]); },
    };
  }
  return { events, page: {
    getByRole(role, options) {
      if (role === 'option') { assert.equal(options.exact, true); return locator([{ role, name: options.name }]); }
      return locator(rows.filter(row => row.role === role && (options.name instanceof RegExp ? options.name.test(row.name) : row.name === options.name)));
    },
    locator(selector) { assert.equal(selector, 'div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]'); return locator(rows.filter(row => row.role === 'button' && row.title && row.popup === 'listbox')); },
  } };
}
const insist = (condition, message) => assert.ok(condition, message);
function followIn(page) { return vm.runInNewContext('(' + followSource[1] + ')', { page, selectTitleLibraryStatus }); }
function migrationFenceIn(page) {
  return vm.runInNewContext('(async () => insist(' + negativeSource[1] + ', ' + JSON.stringify(negativeSource[2]) + '))', { page, titleLibraryTrigger, insist })();
}

test('actual signed follow call uses shared strict native Trigger and exact option', async () => {
  for (const status of ['Reading', 'Plan to Read']) {
    const value = pageStandIn(['Add to Library']);
    await followIn(value.page)(status);
    assert.deepEqual(value.events.map(row => [row.role, row.name]), [['button', 'Add to Library'], ['option', status]]);
  }
});

test('actual signed follow fails on absent or ambiguous real Trigger', async () => {
  for (const names of [[], ['Reading', 'Reading']]) {
    const value = pageStandIn(names);
    await assert.rejects(followIn(value.page)('Reading'), /ambiguous or missing/);
    assert.equal(value.events.length, 0);
  }
});

test('migration no-save assertion rejects every visible native Trigger', async () => {
  for (const name of ['Add to Library', 'Reading', 'Plan to Read', 'On Hold', 'Completed', 'Dropped', 'Re-reading']) {
    const value = pageStandIn([name]);
    assert.equal(await value.page.getByRole('combobox', { name: /Add to Library|Reading|Plan to Read|On Hold|Completed|Dropped|Re-reading/ }).count(), 0, 'The obsolete role would pass vacuously despite this visible native Trigger');
    await assert.rejects(migrationFenceIn(value.page), /Current catalogue title offered native save controls under a mismatched old scope/);
    assert.equal(value.events.length, 0, 'A negative assertion never clicks a control');
  }
});

test('migration no-save assertion passes when real title Trigger is absent', async () => {
  const value = pageStandIn(); await migrationFenceIn(value.page);
  assert.equal(value.events.length, 0);
});
