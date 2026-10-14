#!/usr/bin/env node
// Execute the actual signed-journey setup and denial callbacks with explicit
// route-owned section/native-details stand-ins and the actual getPublicKey branch.
// No browser, cryptographic signing, network, service, or database is started; these seams do not establish signed browser acceptance.
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
const librarySource = source.match(/  const library = ([^\n]+);/)[1];
const managementStart = source.indexOf('  const leaveReader = async () => {');
const managementEnd = source.indexOf('\n  const prepareSignerControls =', managementStart);
assert(managementStart > 0 && managementEnd > managementStart);
const managementSource = source.slice(managementStart, managementEnd);
const signerStart = source.indexOf('  const signerCall = async (source, operation, input) => {');
const signerEnd = source.indexOf('\n  const installSigner =', signerStart);
assert(signerStart > 0 && signerEnd > signerStart);
const signerSource = source.slice(signerStart, signerEnd);
const publicationTitle = 'Public sync & recovery';
const advancedTitle = 'Advanced library settings';
const buttonName = 'Connect external browser signer';

function fixture({ failAt, duplicates = 1, panelDuplicates = 1, jobCount = 0, signerSourcePage, signerFrameOrigin, approveSignerSourcePage = false, skipSignerCall = false } = {}) {
  const events = [], state = { route: '/title/synthetic', section: false, advanced: false, notice: '' };
  let scope;
  const origin = 'http://127.0.0.1:4000';
  const point = name => { events.push(name); if (name === failAt) throw Error('synthetic setup interruption: ' + name); };
  const inManagement = () => assert.equal(state.route, '/settings/preferences#local-library', 'controls belong to the Settings management route');
  const button = {
    async waitFor({ state: expected }) {
      assert.equal(expected, 'visible'); point('button-visible-unique');
      assert.equal(duplicates, 1, 'strict locator rejects missing or duplicate signer controls');
      assert(state.route === '/settings/preferences#local-library' && state.section, 'signer control is hidden outside Settings or under closed native details');
    },
    async click() {
      point('button-click'); assert.equal(duplicates, 1); inManagement(); assert(state.section);
      assert.equal(scope.deniedConnect, true, 'only an explicitly denied synthetic selection may execute');
      if (skipSignerCall) return;
      point('signer-get-public-key');
      try {
        await vm.runInContext('signerCall', scope)({ page: signerSourcePage ?? expectedPage,
          frame: { url: () => (signerFrameOrigin ?? origin) + '/settings/preferences#local-library' } }, 'getPublicKey');
        state.notice = 'Signer connected. No library data sent.';
      } catch (cause) { state.notice = cause.message; }
    },
  };
  const summary = (key, event) => ({
    locator(selector) { assert.equal(selector, '..'); return { async getAttribute(name) { assert.equal(name, 'open'); return state[key] ? '' : null; } }; },
    async click() { point(event); inManagement(); state[key] = true; },
  });
  const panel = {
    async waitFor({ state: expected }) { assert.equal(expected, 'visible'); point('panel-visible'); inManagement(); },
    async count() { point('panel-visible-unique'); return panelDuplicates; },
    locator(selector) {
      assert.equal(selector, 'summary');
      return { filter({ hasText }) {
        const matches = [[publicationTitle, 'section', 'publication-summary-click'], [advancedTitle, 'advanced', 'advanced-summary-click']].filter(([title]) => hasText.test(title));
        assert.equal(matches.length, 1, 'disclosure lookup must be exact and unique');
        const [title, key, event] = matches[0]; assert(!hasText.test('prefix ' + title)); return summary(key, event);
      } };
    },
    getByRole(role, options) { assert.equal(role, 'button'); assert.equal(options.name, buttonName); assert.equal(options.exact, true); return button; },
    getByText(text, options) {
      assert.equal(text, 'Synthetic signer selection denied'); assert.equal(options.exact, false);
      return { async waitFor({ state: expected }) { assert.equal(expected, 'visible'); point('denial-visible'); assert.equal(state.notice, text); inManagement(); assert(state.section); } };
    },
  };
  const expectedPage = {
    url() { return origin + state.route; },
    locator(selector) { assert.equal(selector, 'section[data-local-library-panel]'); return panel; },
    getByRole(role, options) {
      assert.equal(role, 'link'); assert.equal(options.exact, true);
      assert(['My Library', 'Manage library'].includes(options.name)); return { name: options.name };
    },
  };
  const ownedPages = new Set([expectedPage]);
  if (approveSignerSourcePage) { assert(signerSourcePage); ownedPages.add(signerSourcePage); }
  scope = vm.createContext({
    deniedConnect: false, page: expectedPage, ownedPages, origin, URL, owner: 'a'.repeat(64),
    async spaClick(link, accepts) {
      point(link.name === 'My Library' ? 'navigate-library' : 'navigate-management');
      if (link.name === 'Manage library') assert.equal(state.route, '/library');
      state.route = link.name === 'My Library' ? '/library' : '/settings/preferences#local-library'; state.section = false; state.advanced = false;
      assert(accepts(new URL(origin + state.route)), 'actual navigation predicate must accept the visible target');
      assert.equal(accepts(new URL(origin + '/title/wrong')), false);
    },
    async idle(target) { assert.equal(target, panel); point('operation-settled'); },
    async jobs() { point('outbox-read'); return Array(jobCount).fill({}); },
    insist(condition, message) { assert(condition, message); },
  });
  vm.runInContext('const library = ' + librarySource + '; const expand = ' + expandSource + '; const section = ' + sectionSource + ';' + managementSource + prepareSource + signerSource, scope);
  return { state, events, button, panel, scope, denial: vm.runInContext('(' + denialSource + ')', scope), prepare: vm.runInContext('prepareSignerControls', scope) };
}

test('production-composed Settings owns management while the root retains only renderless lifecycle', () => {
  const frontend = fileURLToPath(new URL('../../frontend/', import.meta.url));
  const plugin = createFrontendOverlays(frontend);
  const compose = relative => { const file = path.join(frontend, relative), original = fs.readFileSync(file, 'utf8'); return plugin.transform.handler(original, file)?.code ?? original; };
  const root = compose('src/routes/+layout.svelte'), app = compose('src/routes/(app)/+layout.svelte');
  assert.match(root, /import LibraryRuntime from "\$localLibrary\/LibraryRuntime\.svelte"/);
  assert.match(root, /<LibraryRuntime \/>/);
  for (const shell of [root, app]) assert.doesNotMatch(shell, /(?:import|<)(?:\s+)?(?:LocalLibraryPanel|ProtocolIdentityPanel)/);
  const settings = compose('src/routes/(app)/settings/preferences/+page.svelte');
  assert.match(settings, /import LocalLibraryPanel from "\$localLibrary\/LocalLibraryPanel\.svelte"/);
  assert.match(settings, /\{\/if\}\s*<div[^>]*>\s*<LocalLibraryPanel \/>\s*<ProtocolIdentityPanel \/>/);
  assert(settings.lastIndexOf('</form>') < settings.indexOf('<LocalLibraryPanel />'), 'management remains outside the native preference form');
  const library = compose('src/routes/(app)/library/+layout.svelte');
  assert.match(library, /<Button\b[^>]*href="\/settings\/preferences#local-library"[^>]*>Manage library<\/Button>\s*<\/div>\s*<Protected>/);
  const runtime = fs.readFileSync(new URL('../../platform-integrations/local-library/LibraryRuntime.svelte', import.meta.url), 'utf8');
  assert.match(runtime, /void openLibrary\(\)/);
  assert.match(runtime, /window\.addEventListener\('weebdex-library-error', reportError\)/);
  assert.match(runtime, /window\.removeEventListener\('weebdex-library-error', reportError\)/);
  assert.doesNotMatch(runtime.replace(/<script\b[^>]*>[\s\S]*?<\/script>/, ''), /\S/, 'persistent lifecycle has no visible management chrome');
});

test('real management panel is a normal Settings section with closed optional disclosures', () => {
  const tag = panelSource.match(/<section\b[^>]*\bdata-local-library-panel[^>]*>/)?.[0];
  assert(tag, 'management must be a normal section'); assert.match(tag, /\bid="local-library"/);
  assert.doesNotMatch(panelSource, /<details\b[^>]*\bdata-local-library-panel/);
  for (const title of ['Public sync &amp; recovery', advancedTitle]) {
    const details = panelSource.match(new RegExp('<details\\b([^>]*)>\\s*<summary\\b[^>]*>' + title + '<\\/summary>'));
    assert(details, 'reviewed optional disclosure must exist: ' + title); assert.doesNotMatch(details[1], /\bopen(?:\s|=|$)/);
  }
  const identity = fs.readFileSync(new URL('../../platform-integrations/local-library/ProtocolIdentityPanel.svelte', import.meta.url), 'utf8');
  assert.match(identity, /<section\b[^>]*data-protocol-identity-panel/);
  assert.match(identity, /<summary\b[^>]*>Signing identity &amp; instances<\/summary>/);
  assert.match(source, /await management\(\); await expand\(identity\(\)\); await section\(identity\(\), "Signing identity & instances"\)/);
});

test('actual signer denial reestablishes its visible controls after failed title follow', async () => {
  const value = fixture(), report = {}, check = createJourneyCheck(report);
  await assert.rejects(value.button.waitFor({ state: 'visible' }), /hidden outside Settings or under closed native details/);
  value.events.length = 0;
  await assert.rejects(check('signed-library-mapped-follow-trigger-visible-unique', async () => { throw Error('original title failure'); }), /original title failure/);
  await check('signed-library-signer-selection-denied', value.denial);
  assert.deepEqual(value.events, ['navigate-library', 'navigate-management', 'panel-visible', 'panel-visible-unique', 'advanced-summary-click', 'publication-summary-click', 'button-visible-unique', 'button-click', 'signer-get-public-key', 'operation-settled', 'denial-visible', 'outbox-read']);
  assert.deepEqual(report.journey_checks, [
    { check_id: 'signed-library-mapped-follow-trigger-visible-unique', status: 'failed' },
    { check_id: 'signed-library-signer-selection-denied', status: 'passed' },
  ], 'Independent signer readiness must never erase the preceding title failure');
});

test('every actual setup interruption prevents signer selection and preserves the error', async () => {
  for (const failAt of ['navigate-library', 'navigate-management', 'panel-visible', 'panel-visible-unique', 'advanced-summary-click', 'publication-summary-click', 'button-visible-unique']) {
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

test('missing or ambiguous normal management sections fail before signer selection', async () => {
  for (const panelDuplicates of [0, 2]) {
    const value = fixture({ panelDuplicates });
    await assert.rejects(value.denial(), /Expected one route-owned management or conversation surface/);
    assert(!value.events.includes('button-click')); assert.equal(value.scope.deniedConnect, false);
  }
});

test('navigation and expanded controls cannot falsely pass without the actual refusal', async () => {
  const value = fixture({ skipSignerCall: true });
  await assert.rejects(value.denial(), /Synthetic signer selection denied/);
  assert.equal(value.state.route, '/settings/preferences#local-library');
  assert(value.state.advanced && value.state.section);
  assert(!value.events.includes('signer-get-public-key'));
  assert(!value.events.includes('outbox-read'));
});

test('actual synthetic signer refuses another page or frame origin before the denial branch', async () => {
  for (const options of [{ signerSourcePage: {} }, { signerFrameOrigin: 'http://127.0.0.1:4001' }]) {
    const value = fixture(options);
    await assert.rejects(value.denial(), /Synthetic signer selection denied/);
    assert.match(value.state.notice, /Synthetic signer called outside its explicitly selected fixture page/);
    assert(value.events.includes('signer-get-public-key'));
    assert(!value.events.includes('outbox-read'));
  }
});

test('explicitly owned source-moderation second page may reach the same denied signer branch', async () => {
  const value = fixture({ signerSourcePage: {}, approveSignerSourcePage: true });
  await value.denial(); assert.equal(value.state.notice, 'Synthetic signer selection denied');
  assert(value.events.includes('outbox-read'));
  assert.match(source, /const snapshotPage = page, conversationPage = await context\.newPage\(\); ownedPages\.add\(conversationPage\); page = conversationPage/);
});

test('denied selection must still leave the actual outbox empty', async () => {
  const value = fixture({ jobCount: 1 });
  await assert.rejects(value.denial(), /Denied signer selection queued a snapshot/);
  assert(value.events.includes('denial-visible'));
});

test('normal management section and empty-string open disclosures stay open on repeated readiness', async () => {
  const value = fixture();
  const expand = vm.runInContext('expand', value.scope), section = vm.runInContext('section', value.scope);
  await value.prepare(); value.events.length = 0;
  await expand(value.panel); await section(value.panel, publicationTitle); await value.prepare();
  assert.deepEqual(value.events, ['panel-visible', 'panel-visible-unique', 'panel-visible', 'panel-visible-unique', 'button-visible-unique']);
  assert(value.state.advanced && value.state.section);
});
