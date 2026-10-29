#!/usr/bin/env node
// Actual signed-journey callbacks with explicit native Settings/task/Dialog
// stand-ins and the actual getPublicKey branch. Separate composition tests
// inspect product source. Neither establishes signed browser acceptance.
// No browser, cryptographic signing, network, service, or database is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
// Source-inspection override only; it does not integrate the product into this
// checkout or prove anything about this checkout's built application.
const productRoot = path.resolve(process.env.WEEBDEX_SETTINGS_SOURCE_ROOT ?? root);
const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const between = (first, last) => {
  const start = source.indexOf(first), end = source.indexOf(last, start);
  assert(start > 0 && end > start, 'Actual helper boundaries must remain explicit: ' + first);
  return source.slice(start, end);
};
const surfaceSource = between('  const libraryTools = new WeakMap()', '\n  const idle =');
const managementSource = between('  const leaveReader = async () => {', '\n  const prepareSignerControls =');
const prepareSource = between('  const prepareSignerControls = async () => {', '\n  const knownServiceKey =');
const signerSource = between('  const signerCall = async (source, operation, input) => {', '\n  const installSigner =');
const denialSource = source.match(/await inspect\('signer-selection-denied', (async \(\) => \{[^\n]+\})\);/)[1];
const buttonName = 'Connect external browser signer';
const syncTasks = { destinations: 'Choose destinations', publish: 'Publish library', 'public-recovery': 'Recover public library', attempts: 'Saved sync attempts' };

function fixture({ failAt, failOccurrence = 1, duplicates = 1, panelDuplicates = 1, taskDuplicates = 1, dialogDuplicates = 1,
  identityDuplicates = 0, jobCount = 0, signerSourcePage, signerFrameOrigin, approveSignerSourcePage = false,
  skipSignerCall = false, initialRoute = '/title/synthetic', initialTool, manageHref = '/settings/library' } = {}) {
  const events = [], occurrence = new Map(), origin = 'http://127.0.0.1:4000';
  const interruption = Error('synthetic setup interruption: ' + failAt);
  let scope;
  const point = name => {
    events.push(name); const count = (occurrence.get(name) ?? 0) + 1; occurrence.set(name, count);
    if (name === failAt && count === failOccurrence) throw interruption;
  };
  const states = new WeakMap(), active = () => states.get(scope.page);
  const inManagement = () => assert.equal(active().route, '/settings/library', 'controls belong to the native Library Settings tab');
  const inTask = tool => { inManagement(); assert.equal(active().tool, tool, 'controls must belong to the selected native task'); };
  const exact = options => assert.equal(options.exact, true, 'all actionable names must be exact');
  const button = {
    async waitFor({ state }) {
      assert.equal(state, 'visible'); point('signer-visible-unique');
      assert.equal(duplicates, 1, 'strict locator rejects missing or duplicate signer controls'); inTask('destinations');
    },
    async click() {
      point('signer-click'); assert.equal(duplicates, 1); inTask('destinations');
      assert.equal(scope.deniedConnect, true, 'only an explicitly denied synthetic selection may execute');
      if (skipSignerCall) return;
      point('signer-get-public-key');
      try {
        await vm.runInContext('signerCall', scope)({ page: signerSourcePage ?? scope.page,
          frame: { url: () => (signerFrameOrigin ?? origin) + '/settings/library' } }, 'getPublicKey');
        active().notice = 'Signer connected. No library data sent.';
      } catch (cause) { active().notice = cause.message; }
    },
  };
  const surface = (kind, tool) => ({
    async waitFor({ state }) { assert.equal(state, 'visible'); point(kind + '-visible'); inManagement(); if (tool) inTask(tool); },
    async count() {
      point(kind + '-unique');
      if (kind === 'identity') return identityDuplicates;
      return active().route !== '/settings/library' ? 0 : tool ? active().tool === tool ? taskDuplicates : 0 : panelDuplicates;
    },
    getByRole(role, options) {
      assert.equal(role, 'button'); exact(options);
      if (options.name === buttonName) { assert.equal(tool, 'destinations', 'signer is scoped to the destinations task'); return button; }
      const selected = { 'Manage lists': 'lists', 'Set up public sync': 'sync', 'Library recovery': 'recovery' }[options.name];
      if (selected) {
        assert.equal(kind, 'main', 'main task entry must use the native route-owned panel');
        return { async click() { point('open-' + selected); inManagement(); assert.equal(active().tool, undefined); active().tool = selected; } };
      }
      if (options.name === 'Back to public sync') {
        assert(Object.hasOwn(syncTasks, tool));
        return { async click() { point('back-to-sync'); inTask(tool); active().tool = 'sync'; } };
      }
      const task = Object.keys(syncTasks).find(key => syncTasks[key] === options.name);
      assert(task, 'Unexpected native task button: ' + options.name); assert.equal(tool, 'sync');
      return { async click() { point('open-' + task); inTask('sync'); active().tool = task; } };
    },
    getByText(text, options) {
      assert.equal(tool, 'destinations'); assert.equal(text, 'Synthetic signer selection denied'); assert.equal(options.exact, false);
      return { async waitFor({ state }) { assert.equal(state, 'visible'); point('denial-visible'); inTask('destinations'); assert.equal(active().notice, text); } };
    },
  });
  const dialog = {
    async count() { point('dialog-count'); return active().tool ? dialogDuplicates : 0; },
    getByRole(role, options) {
      assert.equal(role, 'button'); exact(options); assert.equal(options.name, 'Close', 'Close must not match Close account or Close task');
      return { async click() { point('dialog-close'); assert(active().tool); assert.equal(dialogDuplicates, 1); active().tool = undefined; } };
    },
    async waitFor({ state }) { assert.equal(state, 'hidden'); point('dialog-hidden'); assert.equal(active().tool, undefined); },
  };
  function newPage(route = initialRoute, tool = initialTool) {
    const page = {
      url() { return origin + states.get(page).route; },
      locator(selector) {
        if (selector === 'section[data-local-library-panel]') return surface('main');
        if (selector === 'section[data-protocol-identity-panel]') return surface('identity');
        const match = selector.match(/^section\[data-library-tool="([a-z-]+)"\]$/);
        if (match) return surface(match[1], match[1]);
        assert.equal(selector, 'nav[aria-label="Settings"]', 'Settings lookup must be scoped to native navigation');
        return { getByRole(role, options) { assert.equal(role, 'link'); exact(options); assert.equal(options.name, 'Library'); return { name: options.name }; } };
      },
      getByRole(role, options) {
        if (role === 'dialog') { assert.equal(options, undefined); return dialog; }
        assert.equal(role, 'link'); exact(options); assert(['My Library', 'Manage library'].includes(options.name));
        return { name: options.name, async getAttribute(attribute) { assert.equal(attribute, 'href'); point('management-href'); return manageHref; } };
      },
    };
    states.set(page, { route, tool, notice: '' }); return page;
  }
  const expectedPage = newPage(), ownedPages = new Set([expectedPage]);
  if (approveSignerSourcePage) { assert(signerSourcePage); ownedPages.add(signerSourcePage); }
  scope = vm.createContext({
    deniedConnect: false, page: expectedPage, ownedPages, origin, URL, owner: 'a'.repeat(64),
    async spaClick(link, accepts) {
      const target = link.name === 'My Library' ? '/library' : '/settings/library';
      point(link.name === 'My Library' ? 'navigate-library' : link.name === 'Library' ? 'navigate-library-tab' : 'navigate-management');
      if (link.name === 'Manage library') assert.equal(active().route, '/library');
      if (link.name === 'Library') assert(active().route.startsWith('/settings/'));
      active().route = target; active().tool = undefined;
      assert(accepts(new URL(origin + target)), 'actual navigation predicate must accept native target');
      for (const rejected of ['/title/wrong', '/settings/preferences', '/settings/identity']) assert.equal(accepts(new URL(origin + rejected)), false);
    },
    async idle(target) { assert.equal(typeof target.getByText, 'function'); inTask('destinations'); point('operation-settled'); },
    async jobs() { point('outbox-read'); return Array(jobCount).fill({}); },
    insist(condition, message) { assert(condition, message); },
  });
  vm.runInContext(surfaceSource + managementSource + prepareSource + signerSource, scope);
  if (initialTool) vm.runInContext(`libraryTools.set(page, ${JSON.stringify(initialTool)})`, scope);
  return { state: states.get(expectedPage), events, button, scope, newPage, interruption,
    denial: vm.runInContext('(' + denialSource + ')', scope), prepare: vm.runInContext('prepareSignerControls', scope),
    openTool: vm.runInContext('openLibraryTool', scope), closeTools: vm.runInContext('closeTools', scope) };
}

test('composition: dedicated native Settings tabs own management and root retains renderless lifecycle', async () => {
  const frontend = path.join(productRoot, 'frontend');
  const { createFrontendOverlays } = await import(pathToFileURL(path.join(productRoot, 'platform-integrations/frontend-overlays.mjs')).href);
  const plugin = createFrontendOverlays(frontend);
  const compose = relative => { const file = path.join(frontend, relative), original = fs.readFileSync(file, 'utf8'); return plugin.transform.handler(original, file)?.code ?? original; };
  const rootLayout = compose('src/routes/+layout.svelte'), app = compose('src/routes/(app)/+layout.svelte');
  assert.match(rootLayout, /import LibraryRuntime from "\$localLibrary\/LibraryRuntime\.svelte"/); assert.match(rootLayout, /<LibraryRuntime \/>/);
  for (const shell of [rootLayout, app, compose('src/routes/(app)/settings/preferences/+page.svelte')]) assert.doesNotMatch(shell, /(?:import|<)(?:\s+)?(?:LocalLibraryPanel|ProtocolIdentityPanel|PublicIdentitySettings)/);
  const library = compose('src/routes/(app)/settings/library/+page.svelte');
  assert.match(library, /import LocalLibraryPanel from "\$localLibrary\/LocalLibraryPanel\.svelte"/); assert.match(library, /<LocalLibraryPanel \/>/);
  assert.doesNotMatch(library, /(?:import|<)(?:\s+)?(?:ProtocolIdentityPanel|PublicIdentitySettings)/);
  assert.doesNotMatch(compose('src/routes/(app)/settings/library/+layout.svelte'), /Protected|localLibraryState|\$Session/);
  const tabs = compose('src/routes/(app)/settings/+layout.svelte');
  assert.match(tabs, /<nav\b[^>]*aria-label="Settings"/); assert.match(tabs, /aria-current=\{page\.url\.pathname === pathname \? "page" : undefined\}/);
  for (const name of ['library', 'identity']) {
    const entry = tabs.match(new RegExp('\\b' + name + ': \\{([^}]+)\\}'))?.[1];
    assert(entry, 'native Settings tab exists: ' + name); assert.doesNotMatch(entry, /hidden|Session/);
  }
  const { frontendRouteAdditions } = await import(pathToFileURL(path.join(productRoot, 'platform-integrations/frontend-route-additions.mjs')).href);
  const identityRoute = frontendRouteAdditions.find(item => item.target === 'src/routes/(app)/settings/identity/+page.svelte');
  assert(identityRoute, 'dedicated Public identity route must be part of actual route additions');
  const identity = fs.readFileSync(path.join(productRoot, identityRoute.source), 'utf8');
  assert.match(identity, /import PublicIdentitySettings from ['"]\$localLibrary\/PublicIdentitySettings\.svelte['"]/); assert.match(identity, /<PublicIdentitySettings \/>/);
  assert.doesNotMatch(identity, /LocalLibraryPanel|Protected/);
  assert.match(compose('src/routes/(app)/library/+layout.svelte'), /<Button\b[^>]*href="\/settings\/library"[^>]*>Manage library<\/Button>\s*<\/div>\s*<Protected>/);
  const runtime = fs.readFileSync(path.join(productRoot, 'platform-integrations/local-library/LibraryRuntime.svelte'), 'utf8');
  assert.match(runtime, /void openLibrary\(\)/); assert.match(runtime, /window\.addEventListener\('weebdex-library-error', reportError\)/); assert.match(runtime, /window\.removeEventListener\('weebdex-library-error', reportError\)/);
  assert.doesNotMatch(runtime.replace(/<script\b[^>]*>[\s\S]*?<\/script>/, ''), /\S/, 'persistent lifecycle has no visible management chrome');
});

test('composition: native Settings tasks use dialogs and identity hides only advanced connection options', () => {
  const read = name => fs.readFileSync(path.join(productRoot, 'platform-integrations/local-library', name), 'utf8');
  const panel = read('LocalLibraryPanel.svelte'), tools = read('LibraryManagementTools.svelte'), identity = read('PublicIdentitySettings.svelte');
  assert.match(panel, /<section\b[^>]*\bdata-local-library-panel/);
  for (const title of ['Manage lists', 'Set up public sync', 'Library recovery']) assert(panel.includes('>' + title + '</Button>'));
  assert.match(panel, /import \* as Dialog from ['"]\$lib\/components\/ui\/dialog['"]/); assert.match(panel, /<Dialog\.Root open=\{toolOpen\}/); assert.match(panel, /<LibraryManagementTools view=\{tool\}/);
  assert.match(tools, /<section\b[^>]*\bdata-library-tool=\{view\}/);
  assert.match(tools, /view === 'sync'[\s\S]*onTask\('destinations'\)[^>]*>Choose destinations<\/Button>/);
  assert.match(tools, /view === 'destinations'[\s\S]*>Back to public sync<\/Button>[\s\S]*>Connect external browser signer<\/Button>/);
  assert.match(identity, /<section\b[^>]*data-protocol-identity-panel/); assert.match(identity, /<Dialog\.Root \{open\}/); assert.match(identity, /<ProtocolIdentityPanel\b[^>]*settingsView=\{view\}/);
  assert.doesNotMatch(panel, /<(?:details|summary)\b/);
  assert.equal((tools.match(/<details>/g) ?? []).length, 2);
  assert.match(tools, /<summary>Unshared entry references for support<\/summary>/);
  assert.match(tools, /<summary>Title reference for support<\/summary>/);
  assert.equal((identity.match(/<details\b/g) ?? []).length, 2);
  assert.match(identity, /<details[^>]*data-advanced-identity[\s\S]*<summary[^>]*>Advanced identity tools<\/summary>/);
  assert.match(identity, /<summary[^>]*>Advanced connection options<\/summary>/);
  assert.match(identity, /onclick=\{\(\) => edit\('keys'\)\}/);
  assert.match(source, /await openIdentityTool\("connections"\)/);
});

test('mocked safety: actual signer denial reestablishes visible native task after failed title follow', async () => {
  const value = fixture(), report = {}, check = createJourneyCheck(report);
  await assert.rejects(value.button.waitFor({ state: 'visible' }), /controls belong to the native Library Settings tab/); value.events.length = 0;
  await assert.rejects(check('signed-library-mapped-follow-trigger-visible-unique', async () => { throw Error('original title failure'); }), /original title failure/);
  await check('signed-library-signer-selection-denied', value.denial);
  assert.deepEqual(value.events, ['dialog-count', 'navigate-library', 'management-href', 'navigate-management', 'main-visible', 'main-unique', 'identity-unique', 'open-sync', 'sync-visible', 'sync-unique', 'dialog-count', 'open-destinations', 'destinations-visible', 'destinations-unique', 'dialog-count', 'signer-visible-unique', 'signer-click', 'signer-get-public-key', 'operation-settled', 'denial-visible', 'outbox-read']);
  assert.deepEqual(report.journey_checks, [{ check_id: 'signed-library-mapped-follow-trigger-visible-unique', status: 'failed' }, { check_id: 'signed-library-signer-selection-denied', status: 'passed' }], 'Independent signer readiness must never erase preceding title failure');
});

test('mocked safety: every actual readiness interruption prevents selection and preserves original error', async () => {
  for (const initial of [{}, { initialRoute: '/settings/identity' }, { initialRoute: '/settings/library', initialTool: 'lists' },
    { initialRoute: '/settings/library', initialTool: 'publish' }, { initialRoute: '/settings/library', initialTool: 'destinations' }]) {
    const baseline = fixture(initial); await baseline.prepare(); const seen = new Map();
    for (const barrier of baseline.events) {
      const failOccurrence = (seen.get(barrier) ?? 0) + 1; seen.set(barrier, failOccurrence);
      const value = fixture({ ...initial, failAt: barrier, failOccurrence });
      await assert.rejects(value.denial(), cause => cause === value.interruption);
      assert.equal(value.events.at(-1), barrier); assert.equal(value.scope.deniedConnect, false); assert(!value.events.includes('signer-click')); assert(!value.events.includes('outbox-read'));
    }
  }
});

test('mocked safety: missing or ambiguous exact signer controls fail before clicking', async () => {
  for (const duplicates of [0, 2]) {
    const value = fixture({ duplicates }); await assert.rejects(value.denial(), /strict locator rejects missing or duplicate signer controls/);
    assert(!value.events.includes('signer-click')); assert.equal(value.scope.deniedConnect, false);
  }
});

test('mocked safety: missing or ambiguous native route/task/dialog surfaces fail before selection', async () => {
  for (const options of [{ panelDuplicates: 0 }, { panelDuplicates: 2 }, { taskDuplicates: 0 }, { taskDuplicates: 2 }, { dialogDuplicates: 0 }, { dialogDuplicates: 2 }, { identityDuplicates: 1 }]) {
    const value = fixture(options);
    await assert.rejects(value.denial(), /Expected one route-owned management or conversation surface|Library task is not the single native dialog|Library tab also mounted public-identity management/);
    assert(!value.events.includes('signer-click')); assert.equal(value.scope.deniedConnect, false);
  }
});

test('mocked safety: navigation and visible controls cannot falsely pass without actual refusal', async () => {
  const value = fixture({ skipSignerCall: true }); await assert.rejects(value.denial(), /Synthetic signer selection denied/);
  assert.equal(value.state.route, '/settings/library'); assert.equal(value.state.tool, 'destinations'); assert(!value.events.includes('signer-get-public-key')); assert(!value.events.includes('outbox-read'));
});

test('mocked safety: synthetic signer rejects another page or frame origin before denial branch', async () => {
  for (const options of [{ signerSourcePage: {} }, { signerFrameOrigin: 'http://127.0.0.1:4001' }, { signerFrameOrigin: 'https://127.0.0.1:4000' },
    { signerSourcePage: {}, approveSignerSourcePage: true, signerFrameOrigin: 'http://127.0.0.1:4001' }]) {
    const value = fixture(options); await assert.rejects(value.denial(), /Synthetic signer selection denied/);
    assert.match(value.state.notice, /Synthetic signer called outside its explicitly selected fixture page/); assert(value.events.includes('signer-get-public-key')); assert(!value.events.includes('outbox-read'));
  }
});

test('mocked safety: explicitly owned second page may reach same-origin denied signer branch', async () => {
  const value = fixture({ signerSourcePage: {}, approveSignerSourcePage: true }); await value.denial();
  assert.equal(value.state.notice, 'Synthetic signer selection denied'); assert(value.events.includes('outbox-read'));
  assert.match(source, /const snapshotPage = page, conversationPage = await context\.newPage\(\); ownedPages\.add\(conversationPage\); page = conversationPage/);
});

test('mocked safety: denied selection must still leave actual outbox empty', async () => {
  const value = fixture({ jobCount: 1 }); await assert.rejects(value.denial(), /Denied signer selection queued a snapshot/); assert(value.events.includes('denial-visible'));
});

test('mocked safety: repeated native destinations readiness is idempotent', async () => {
  const value = fixture(); await value.prepare(); value.events.length = 0; await value.prepare(); await value.prepare();
  assert.deepEqual(value.events, Array(2).fill(['destinations-visible', 'destinations-unique', 'dialog-count', 'signer-visible-unique']).flat()); assert.equal(value.state.tool, 'destinations'); assert.equal(value.state.route, '/settings/library');
});

test('mocked safety: other sync subtasks return through exact Back to public sync', async () => {
  for (const initialTool of ['publish', 'public-recovery', 'attempts']) {
    const value = fixture({ initialRoute: '/settings/library', initialTool }); await value.prepare();
    assert.deepEqual(value.events, ['back-to-sync', 'sync-visible', 'sync-unique', 'open-destinations', 'destinations-visible', 'destinations-unique', 'dialog-count', 'signer-visible-unique']); assert.equal(value.state.tool, 'destinations'); assert(!value.events.includes('dialog-close'));
  }
});

test('mocked safety: unrelated native task closes exactly before opening signer destinations', async () => {
  const value = fixture({ initialRoute: '/settings/library', initialTool: 'lists' }); await value.prepare();
  assert.deepEqual(value.events.slice(0, 5), ['dialog-count', 'dialog-count', 'dialog-close', 'dialog-hidden', 'main-visible']); assert.equal(value.state.tool, 'destinations');
  for (const failAt of ['dialog-count', 'dialog-close', 'dialog-hidden']) {
    const interrupted = fixture({ initialRoute: '/settings/library', initialTool: 'lists', failAt });
    await assert.rejects(interrupted.denial(), new RegExp('synthetic setup interruption: ' + failAt)); assert.equal(interrupted.scope.deniedConnect, false); assert(!interrupted.events.includes('signer-click'));
  }
});

test('mocked safety: native Settings navigation is exact and rejects legacy management href', async () => {
  const value = fixture({ initialRoute: '/settings/identity' }); await value.prepare();
  assert(value.events.includes('navigate-library-tab')); assert(!value.events.includes('navigate-library')); assert.equal(value.state.route, '/settings/library');
  for (const manageHref of ['/settings/preferences#local-library', '/settings/identity', '/settings/library#other']) {
    const wrong = fixture({ manageHref }); await assert.rejects(wrong.denial(), /Manage library does not target the native Settings tab/); assert.equal(wrong.scope.deniedConnect, false); assert(!wrong.events.includes('navigate-management'));
  }
});

test('mocked safety: tasks belong to each page and closing clears only that page', async () => {
  const value = fixture(); await value.prepare(); const original = value.scope.page;
  const other = value.newPage('/settings/library'); value.scope.page = other; value.events.length = 0;
  await value.prepare(); assert(value.events.includes('open-sync')); assert(!value.events.includes('dialog-close'));
  vm.runInContext('identityTools.set(page, "connections")', value.scope);
  await value.closeTools(); assert.equal(vm.runInContext('libraryTools.has(page) || identityTools.has(page)', value.scope), false);
  value.scope.page = original; value.events.length = 0; await value.prepare();
  assert.deepEqual(value.events, ['destinations-visible', 'destinations-unique', 'dialog-count', 'signer-visible-unique']);
});

test('mocked safety: optional known-key input uses its exact native owner despite nested label help', async () => {
  const helper = between('  const knownServiceKey = async value => {', '\n  const selectSigner =');
  const markup = fs.readFileSync(path.join(productRoot, 'platform-integrations/local-library/LibraryManagementTools.svelte'), 'utf8');
  assert.match(markup, /<label for="sync-instance-key">Expected service public key\s*<Input id="sync-instance-key"[\s\S]*?<span[^>]*>Optional\./);
  assert.match(helper, /locator\("input#sync-instance-key"\)/);
  assert.doesNotMatch(helper, /\.first\(|\.last\(|getByLabel/);
  for (const initiallyOpen of [false, true]) {
    let shown = initiallyOpen; const calls = [];
    const field = { async count() { return shown ? 1 : 0; }, async fill(value) { assert(shown); calls.push(['fill', value]); } };
    const library = () => ({ locator(selector) { assert.equal(selector, 'input#sync-instance-key'); return field; }, getByRole(role, options) {
      assert.equal(role, 'button'); assert.equal(options.name, 'Use a known service key'); assert.equal(options.exact, true);
      return { async click() { assert.equal(shown, false); shown = true; calls.push(['reveal']); } };
    } });
    const scope = vm.createContext({ library }); vm.runInContext(helper, scope);
    await vm.runInContext('knownServiceKey', scope)('0'.repeat(64));
    assert.deepEqual(calls, [...(initiallyOpen ? [] : [['reveal']]), ['fill', '0'.repeat(64)]]);
  }
});
