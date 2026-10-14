#!/usr/bin/env node
// Execute the actual signed-journey setup and denial callbacks with explicit
// native-details/locator stand-ins. No browser, signer, network, service, or
// database is started; these seams do not establish signed browser acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';

const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const panelSource = fs.readFileSync(new URL('../../platform-integrations/local-library/LocalLibraryPanel.svelte', import.meta.url), 'utf8');
const prepareStart = source.indexOf('  const prepareSignerControls = async () => {');
const prepareEnd = source.indexOf('\n  const selectSigner =', prepareStart);
assert(prepareStart > 0 && prepareEnd > prepareStart);
const prepareSource = source.slice(prepareStart, prepareEnd);
const denialSource = source.match(/await inspect\('signer-selection-denied', (async \(\) => \{[^\n]+\})\);/)[1];
const expandSource = source.match(/  const expand = ([^\n]+);/)[1];
const sectionSource = source.match(/  const section = ([^\n]+);/)[1];
const publicationTitle = 'Optional Nostr publication and verified recovery';
const buttonName = 'Connect external browser signer';

function fixture({ failAt, duplicates = 1, jobCount = 0 } = {}) {
  const events = [], state = { route: '/title/synthetic', outer: false, section: false, notice: '' };
  let scope;
  const point = name => { events.push(name); if (name === failAt) throw Error('synthetic setup interruption: ' + name); };
  const button = {
    async waitFor({ state: expected }) {
      assert.equal(expected, 'visible'); point('button-visible-unique');
      assert.equal(duplicates, 1, 'strict locator rejects missing or duplicate signer controls');
      assert(state.outer && state.section, 'signer control is hidden under closed native details');
    },
    async click() {
      point('button-click'); assert.equal(duplicates, 1); assert(state.outer && state.section);
      assert.equal(scope.deniedConnect, true, 'only an explicitly denied synthetic selection may execute');
      state.notice = 'Synthetic signer selection denied';
    },
  };
  const publication = {
    async getAttribute(name) { assert.equal(name, 'open'); return state.section ? '' : null; },
  };
  const summary = {
    locator(selector) { assert.equal(selector, '..'); return publication; },
    async click() { point('publication-summary-click'); assert(state.outer); state.section = true; },
  };
  const panel = {
    async waitFor({ state: expected }) { assert.equal(expected, 'visible'); point('panel-visible'); assert.equal(state.route, '/library'); },
    async getAttribute(name) { assert.equal(name, 'open'); return state.outer ? '' : null; },
    locator(selector) {
      if (selector === ':scope > summary') return { async click() { point('panel-summary-click'); state.outer = true; } };
      assert.equal(selector, 'summary');
      return { filter({ hasText }) { assert(hasText.test(publicationTitle)); assert(!hasText.test('prefix ' + publicationTitle)); return summary; } };
    },
    getByRole(role, options) { assert.equal(role, 'button'); assert.equal(options.name, buttonName); assert.equal(options.exact, true); return button; },
    getByText(text, options) {
      assert.equal(text, 'Synthetic signer selection denied'); assert.equal(options.exact, false);
      return { async waitFor({ state: expected }) { assert.equal(expected, 'visible'); point('denial-visible'); assert.equal(state.notice, text); assert(state.outer && state.section); } };
    },
  };
  scope = vm.createContext({
    deniedConnect: false,
    library: () => panel,
    async open(route) { point('navigate-library'); assert.equal(route, '/library'); state.route = route; state.outer = false; state.section = false; },
    async idle(target) { assert.equal(target, panel); point('operation-settled'); },
    async jobs() { point('outbox-read'); return Array(jobCount).fill({}); },
    insist(condition, message) { assert(condition, message); },
  });
  vm.runInContext('const expand = ' + expandSource + '; const section = ' + sectionSource + ';' + prepareSource, scope);
  return { state, events, button, scope, denial: vm.runInContext('(' + denialSource + ')', scope), prepare: vm.runInContext('prepareSignerControls', scope) };
}

test('production-composed app layout mounts the local panel before child title routes', () => {
  const frontend = fileURLToPath(new URL('../../frontend/', import.meta.url));
  const layout = path.join(frontend, 'src/routes/(app)/+layout.svelte');
  const plugin = createFrontendOverlays(frontend);
  const composed = plugin.transform.handler(fs.readFileSync(layout, 'utf8'), layout).code;
  assert.match(composed, /import LocalLibraryPanel from "\$localLibrary\/LocalLibraryPanel\.svelte"/);
  assert.match(composed, /<LocalLibraryPanel \/>\s*<ProtocolIdentityPanel \/>\s*\{@render children\(\)\}/);
});

test('real local panel and optional signer section both start as closed native details', () => {
  const tag = panelSource.match(/<details\b[^>]*\bdata-local-library-panel[^>]*>/)[0];
  assert.doesNotMatch(tag, /\bopen(?:\s|=|>)/);
  assert.match(panelSource, /<details class="rounded border p-2">\s*<summary class="cursor-pointer">Optional Nostr publication and verified recovery<\/summary>/);
});

test('actual signer denial reestablishes its visible controls after failed title follow', async () => {
  const value = fixture(), report = {}, check = createJourneyCheck(report);
  await assert.rejects(value.button.waitFor({ state: 'visible' }), /hidden under closed native details/);
  value.events.length = 0;
  await assert.rejects(check('signed-library-mapped-follow-trigger-visible-unique', async () => { throw Error('original title failure'); }), /original title failure/);
  await check('signed-library-signer-selection-denied', value.denial);
  assert.deepEqual(value.events, ['navigate-library', 'panel-visible', 'panel-summary-click', 'publication-summary-click', 'button-visible-unique', 'button-click', 'operation-settled', 'denial-visible', 'outbox-read']);
  assert.deepEqual(report.journey_checks, [
    { check_id: 'signed-library-mapped-follow-trigger-visible-unique', status: 'failed' },
    { check_id: 'signed-library-signer-selection-denied', status: 'passed' },
  ], 'Independent signer readiness must never erase the preceding title failure');
});

test('every actual setup interruption prevents signer selection and preserves the error', async () => {
  for (const failAt of ['navigate-library', 'panel-visible', 'panel-summary-click', 'publication-summary-click', 'button-visible-unique']) {
    const value = fixture({ failAt });
    await assert.rejects(value.denial(), new RegExp('synthetic setup interruption: ' + failAt));
    assert.equal(value.events.at(-1), failAt);
    assert.equal(value.scope.deniedConnect, false);
    assert(!value.events.includes('button-click'));
  }
});

test('missing or ambiguous real signer controls still fail before clicking', async () => {
  for (const duplicates of [0, 2]) {
    const value = fixture({ duplicates });
    await assert.rejects(value.denial(), /strict locator rejects missing or duplicate signer controls/);
    assert(!value.events.includes('button-click'));
    assert.equal(value.scope.deniedConnect, false);
  }
});

test('denied selection must still leave the actual outbox empty', async () => {
  const value = fixture({ jobCount: 1 });
  await assert.rejects(value.denial(), /Denied signer selection queued a snapshot/);
  assert(value.events.includes('denial-visible'));
});

test('native empty-string open attributes do not toggle already-open panels closed', async () => {
  const value = fixture();
  const expand = vm.runInContext('expand', value.scope), section = vm.runInContext('section', value.scope);
  await value.prepare(); value.events.length = 0;
  await expand(value.scope.library()); await section(value.scope.library(), publicationTitle);
  assert.deepEqual(value.events, ['panel-visible']);
  assert(value.state.outer && value.state.section);
});
