#!/usr/bin/env node
// Execute the actual relocation step and management helpers with explicit driver
// stand-ins. Callback DOM values are synthetic: no browser, screenshots, visual
// acceptance, application, network, service, database or downloads are involved.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const source = fs.readFileSync(new URL('../../scripts/browser-journeys.mjs', import.meta.url), 'utf8');
const stepStart = source.indexOf("\t\tawait step('relocated native Library");
const stepEnd = source.indexOf('\n\t\tconst check =', stepStart);
const helperStart = source.indexOf('\tconst panel = () =>');
const helperEnd = source.indexOf('\n\tconst follow =', helperStart);
assert(stepStart > 0 && stepEnd > stepStart && helperStart > 0 && helperEnd > helperStart, 'Actual relocation step/helper boundaries must remain explicit');
const actualSource = source.slice(helperStart, helperEnd) + '\n' + source.slice(stepStart, stepEnd);
const AsyncFunction = Object.getPrototypeOf(async function() {}).constructor;
const origin = 'http://127.0.0.1:1', manga = 'SyntheticManga', chapter = 'SyntheticChapter';
const nativeRoutes = ['/', '/search', `/title/${manga}`, `/chapter/${chapter}/1`];
const tabs = { Preferences: '/settings/preferences', Library: '/settings/library', 'Public identity': '/settings/identity', Account: '/settings/account', 'API Clients': '/settings/client' };
const tools = { library: { 'Manage lists': 'lists', 'Set up public sync': 'sync', 'Library recovery': 'recovery' }, identity: { 'Edit public profile': 'profile', 'Manage follows': 'follows', 'Saved public actions': 'actions' } };

async function probe(fault, { initialReaderOpen } = {}) {
  const events = [], report = { screenshots: [] }, original = Error('Original stand-in driver failure: ' + fault);
  let url = new URL('/', origin), viewport = { width: 1440, height: 1000 }, shown = false, expanded = false, readerSidebar = false, reloads = 0, activeTool, closedTool = false, result;
  const isSettings = () => url.pathname.startsWith('/settings/');
  const atReader = () => url.pathname === `/chapter/${chapter}/1`;
  const owner = () => url.pathname === '/settings/library' ? 'library' : url.pathname === '/settings/identity' ? 'identity' : undefined;
  const visible = async predicate => { if (!predicate()) throw original; };
  const navigate = path => { url = new URL(path, origin); shown = false; expanded = false; readerSidebar = atReader() ? initialReaderOpen ?? viewport.width >= 768 : false; activeTool = undefined; closedTool = false; };
  const box = (kind, label) => {
    events.push(['frame', kind, label, viewport.width]);
    const offscreen = fault === 'offscreen-arrival' && kind === 'heading' && label === 'Library' || fault === 'offscreen-identity' && kind === 'heading' && label === 'Public identity' || fault === 'offscreen-context' && kind === 'context' || fault === 'oversize-dialog' && kind === 'dialog';
    return { width: 250, height: offscreen ? viewport.height + 1 : 50, left: 10, right: 260, top: 10, bottom: offscreen ? viewport.height + 11 : 60 };
  };
  const evaluateBox = (operation, kind, label) => {
    const node = { getBoundingClientRect: () => box(kind, label), contains: () => false };
    return vm.runInNewContext('(' + operation.toString() + ')(node)', { node, innerWidth: viewport.width, innerHeight: viewport.height, document: { elementFromPoint: () => fault === 'occluded-arrival' && kind === 'heading' && label === 'Library' ? {} : node } });
  };
  const countPanel = kind => {
    if (owner() === kind) return kind === 'library' && fault === 'duplicate-library' ? 2 : kind === 'identity' && fault === 'missing-identity' ? 0 : 1;
    return fault === 'cross-mounted-panel' && owner() || fault === 'preferences-panel' && isSettings() && !owner() ? 1 : 0;
  };
  const section = kind => ({
    async count() { return countPanel(kind); },
    async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); await visible(() => owner() === kind); },
    getByRole(role, options) {
      assert.equal(options.exact, true); assert.equal(owner(), kind);
      if (role === 'heading') {
        assert.equal(options.name, kind === 'library' ? 'Library' : 'Public identity');
        return { async evaluate(operation) { return evaluateBox(operation, 'heading', options.name); } };
      }
      assert.equal(role, 'button');
      if (options.name === 'Verify current catalogue scope') return { async count() { return fault === 'scope-controls-main' ? 1 : 0; } };
      if (options.name === 'Connect external signer') return { async waitFor(value) { assert.deepEqual(value, { state: 'visible' }); assert.equal(kind, 'identity'); events.push(['signer-control', viewport.width]); } };
      const tool = tools[kind][options.name]; assert(tool);
      return { async click() { assert(!activeTool); activeTool = { kind, tool }; closedTool = false; events.push(['tool-open', kind, tool, viewport.width]); } };
    },
  });
  const conversation = selector => {
    const match = selector.match(/^section\[data-source-conversation\]\[data-resource-kind="(chapter|manga)"\]\[data-resource-id="([^"]+)"\]$/);
    assert(match, 'Unexpected conversation selector: ' + selector);
    const [, kind, id] = match;
    const bound = () => kind === 'chapter'
      ? atReader() && shown && id === chapter && fault !== 'wrong-chapter'
      : url.pathname === `/title/${manga}` && url.searchParams.get('tab') === 'comments' && id === manga && fault !== 'wrong-manga';
    return {
      async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); events.push(['conversation', kind, id, viewport.width]); await visible(bound); },
      getByRole(role, options) {
        assert.equal(options.exact, true);
        if (role === 'button' && options.name === 'Comments from other sites') return {
          async getAttribute(name) { assert.equal(name, 'aria-expanded'); assert(bound()); return expanded && fault !== 'context-not-expanded' || fault === 'expanded-comments' ? 'true' : 'false'; },
          async click() { assert(bound()); expanded = true; events.push(['signed-comments', kind, viewport.width]); },
        };
        assert.equal(role, 'button'); assert.equal(options.name, 'Load verified public conversation');
        return {
          async count() {
            assert(bound() && expanded);
            if (fault === 'source-loader-driver') throw original;
            events.push(['source-loader', kind, viewport.width]);
            return fault === 'missing-source-loader' ? 0 : 1;
          },
          async scrollIntoViewIfNeeded() { assert(bound() && expanded); events.push(['scroll-context', kind, viewport.width]); },
          async evaluate(operation) { assert(bound() && expanded); return evaluateBox(operation, 'context', kind); },
        };
      },
      getByLabel(label, options) {
        assert.equal(options.exact, true); assert(['Current instance catalogue ID', 'Target type'].includes(label));
        return { async count() { assert(bound()); events.push(['target-input-check', label]); return fault === 'generic-target-input' ? 1 : 0; } };
      },
    };
  };
  const dialog = () => ({
    async count() { return fault === 'preopened-tool' && !activeTool ? 1 : activeTool ? fault === 'duplicate-dialog' ? 2 : 1 : 0; },
    locator(selector) {
      const match = selector.match(/^section\[data-(library|public-identity)-tool="([a-z]+)"\]$/); assert(match);
      return {
        async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); await visible(() => activeTool?.kind === (match[1] === 'public-identity' ? 'identity' : match[1]) && activeTool?.tool === match[2]); },
        async count() { return fault === 'duplicate-tool' ? 2 : activeTool?.kind === (match[1] === 'public-identity' ? 'identity' : match[1]) && activeTool?.tool === match[2] ? 1 : 0; },
        locator(query) { assert.equal(query, 'details, [data-slot^="accordion"]'); return { async count() { return fault === 'tool-accordion' ? 1 : 0; } }; },
      };
    },
    async evaluate(operation) { assert(activeTool); return evaluateBox(operation, 'dialog', activeTool.tool); },
    getByRole(role, options) {
      assert.equal(role, 'button'); assert.deepEqual(options, { name: 'Close', exact: true });
      return { async click() { assert(activeTool); events.push(['tool-close', activeTool.kind, activeTool.tool, viewport.width]); activeTool = undefined; closedTool = true; } };
    },
    async waitFor(options) { assert.deepEqual(options, { state: 'hidden' }); assert.equal(activeTool, undefined); assert.equal(readerSidebar, false); },
  });
  const navigation = {
    async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); assert(isSettings()); events.push(['native-navigation', url.pathname, viewport.width]); },
    getByRole(role, options) {
      assert.equal(role, 'link'); assert.equal(options.exact, true); const path = tabs[options.name]; assert(path);
      return {
        async count() { if (['Account', 'API Clients'].includes(options.name)) return fault === 'protected-tab' ? 1 : 0; return fault === 'missing-tab' && options.name === 'Public identity' ? 0 : fault === 'duplicate-tab' ? 2 : 1; },
        async getAttribute(attribute) {
          if (attribute === 'href') return fault === 'wrong-tab-href' ? '/auth/login' : path;
          assert.equal(attribute, 'aria-current'); return fault === 'wrong-active-tab' ? 'page' : url.pathname === path ? 'page' : null;
        },
        async click() { events.push(['tab-click', options.name, viewport.width]); navigate(path); },
      };
    },
  };
  const page = {
    async setViewportSize(value) { viewport = { ...value }; events.push(['viewport', value.width, value.height]); },
    async screenshot(options) {
      assert.match(options.path, /^\/stand-in-only\/test-evidence\/frontend\/browser\/local-library-standin\/relocation-(desktop|mobile)-[a-z-]+\.png$/);
      assert.equal(typeof options.fullPage, 'boolean');
      if (fault === 'capture-failure') throw original;
      events.push(['capture-stand-in', options.path, options.fullPage]);
    },
    async evaluate(operation) { return vm.runInNewContext('(' + operation.toString() + ')()', { innerWidth: viewport.width, document: { documentElement: { scrollWidth: viewport.width + (fault === 'overflow' ? 2 : 0) } } }); },
    locator(selector) {
      if (selector === 'nav button:has(svg.lucide-settings)') return { first() { return { async click() { assert(atReader() && !readerSidebar, 'Opening an already-open reader sidebar would close it'); if (fault === 'reader-toggle-driver') throw original; readerSidebar = true; events.push(['reader-settings-open', viewport.width]); } }; } };
      if (selector === '[data-slot="sidebar"][data-state="expanded"], [data-slot="sidebar"][data-mobile="true"]') return {
        getByRole(role, options) {
          assert.equal(role, 'link'); assert.deepEqual(options, { name: 'Manage library', exact: true });
          return {
            async isVisible() { assert(atReader()); events.push(['reader-settings-visible', readerSidebar, viewport.width]); return readerSidebar; },
            async waitFor(options) {
              assert(['visible', 'hidden'].includes(options.state)); assert(atReader());
              assert.equal(readerSidebar, options.state === 'visible', 'Active reader Manage visibility must follow native sidebar state');
              events.push(['reader-manage-wait', options.state, viewport.width]);
            },
            async getAttribute(attribute) { assert.equal(attribute, 'href'); assert(atReader() && readerSidebar); return fault === 'wrong-manage-href' ? '/settings/preferences#local-library' : '/settings/library'; },
          };
        },
        locator(close) {
          assert.equal(close, '[data-slot="sidebar-header"] button:has(svg.lucide-x)');
          // Mobile has two legitimate X controls: the reader header toggle and
          // the Sheet primitive's sibling close. Only the header belongs here.
          const nativeCloseControls = [{ owner: 'sidebar-header' }, ...(viewport.width < 768 ? [{ owner: 'sheet-content' }] : [])];
          const selected = nativeCloseControls.filter(control => control.owner === 'sidebar-header');
          events.push(['reader-close-candidates', nativeCloseControls.length, selected.length, viewport.width]);
          return { async click() { assert.equal(selected.length, 1, 'Strict reader-header close must have one owner'); assert(atReader() && readerSidebar); if (fault === 'reader-close-driver') throw original; if (fault !== 'reader-close-retains-panel') readerSidebar = false; events.push(['reader-settings-close', viewport.width]); } };
        },
      };
      if (selector === '[data-local-library-panel], [data-protocol-identity-panel]') return { async count() { assert(!isSettings()); events.push(['global-check', url.pathname]); return fault === 'global-mount' ? 1 : 0; } };
      if (selector === 'img[src^="blob:"]') return { async evaluateAll(operation) {
        assert(atReader()); events.push(['image-callback', viewport.width]);
        const image = { complete: fault !== 'undecoded-image', naturalWidth: fault === 'undecoded-image' ? 0 : 600,
          getBoundingClientRect: () => ({ width: 200, height: 300, left: 0, right: 200, top: fault === 'offscreen-image' ? viewport.height + 1 : 10, bottom: fault === 'offscreen-image' ? viewport.height + 301 : 310 }) };
        return vm.runInNewContext('(' + operation.toString() + ')(images)', { images: [image], innerWidth: viewport.width, innerHeight: viewport.height });
      } };
      if (selector.startsWith('section[data-source-conversation]')) return conversation(selector);
      if (selector === 'section[data-local-library-panel]') return section('library');
      if (selector === 'section[data-protocol-identity-panel]') return section('identity');
      if (selector.startsWith('main section[')) return { async count() { assert(owner()); return fault === 'outside-main' ? 0 : 1; } };
      if (selector === '[data-source-conversation]') return { async count() { assert(isSettings()); return fault === 'settings-conversation' ? 1 : 0; } };
      if (selector === 'details, [data-slot^="accordion"]') return { async count() { assert(isSettings()); return fault === 'accordion' ? 1 : 0; } };
      if (selector === '[data-library-tool], [data-public-identity-tool]') return { async count() { return fault === 'preopened-tool' || fault === 'tool-remains' && closedTool ? 1 : activeTool ? 1 : 0; } };
      assert.equal(selector, 'input[name="email"], input[name="username"], input[name="password"]');
      return { async count() { assert.equal(url.pathname, '/settings/preferences'); return fault === 'protected-controls' ? 1 : 0; } };
    },
    getByRole(role, options) {
      if (role === 'dialog') { assert.equal(options, undefined); return dialog(); }
      assert.equal(options.exact, true);
      if (role === 'navigation') { assert.equal(options.name, 'Settings'); return navigation; }
      if (role === 'button') {
        if (options.name === 'Add New Client') return { async count() { return fault === 'api-controls' ? 1 : 0; } };
        assert.equal(options.name, 'Show Comments');
        return { async isVisible() { assert(atReader()); return true; }, async click() { assert(atReader()); shown = true; events.push(['show-comments', viewport.width]); } };
      }
      assert.equal(role, 'link'); assert.equal(options.name, 'Manage library');
      return {
        async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); assert(atReader() && readerSidebar); },
        async getAttribute(attribute) { assert.equal(attribute, 'href'); assert(atReader() && readerSidebar || url.pathname === '/library'); return fault === 'wrong-manage-href' || fault === 'wrong-library-manage-href' && url.pathname === '/library' ? '/settings/preferences#local-library' : '/settings/library'; },
        async click() { assert.equal(url.pathname, '/library'); if (fault === 'missing-manage-link') throw original; events.push(['manage-link', viewport.width]); navigate('/settings/library'); },
      };
    },
    async waitForURL(predicate) { assert(predicate(url), 'Actual native route or protected navigation must reach its expected destination'); },
    async reload(options) { assert.deepEqual(options, { waitUntil: 'domcontentloaded' }); assert.equal(url.pathname, '/settings/library'); reloads++; if (fault === 'reload-route') url.pathname = '/settings/preferences'; if (fault === 'reload-hash') url.hash = '#local-library'; events.push(['reload', viewport.width]); },
    url() { return url.href; },
  };
  const globals = {
    page, phase: 'local-library', manga, chapter, report, journeyRootUrl: new URL('file:///stand-in-only/'), fileURLToPath, URL,
    process: { env: { BROWSER_RUN_ID: fault === 'unsafe-run-id' ? '../unsafe' : 'standin' } },
    async api(route) { assert.equal(route, '/auth/check'); events.push(['auth', reloads]); return { status: fault === 'authenticated' || fault === 'reload-account' && reloads > 0 || fault === 'protected-account-authority' && events.some(row => row[0] === 'open' && row[1] === '/settings/account') ? 200 : 401 }; },
    async open(route) {
      events.push(['open', route, viewport.width]);
      navigate(['/settings/account', '/settings/client'].includes(route) && fault !== 'unguarded-route' ? '/settings/preferences' : route);
    },
    async until(read, valid, message) { const value = await read(); assert(valid(value), message); return value; },
    insist(value, message) { assert(value, message); },
    async step(label, operation) { assert.match(label, /^relocated native Library/); result = await operation(); },
  };
  let error;
  try { await new AsyncFunction(...Object.keys(globals), actualSource)(...Object.values(globals)); } catch (cause) { error = cause; }
  return { error, original, events, report, result };
}

test('actual step visits both native layouts, bound expanded contexts, native dialogs and direct protected routes with stand-ins', async () => {
  const value = await probe(); assert.equal(value.error, undefined);
  assert.deepEqual(value.events.filter(row => row[0] === 'viewport'), [['viewport', 1440, 1000], ['viewport', 390, 844], ['viewport', 1440, 1000]]);
  for (const width of [1440, 390]) {
    assert.deepEqual(value.events.filter(row => row[0] === 'open' && row[2] === width).map(row => row[1]), [...nativeRoutes, `/title/${manga}?tab=comments`, '/library', '/settings/account', '/settings/client']);
    for (const kind of ['chapter', 'manga']) {
      assert(value.events.some(row => row[0] === 'conversation' && row[1] === kind && row[3] === width));
      assert(value.events.some(row => row[0] === 'signed-comments' && row[1] === kind && row[2] === width));
      assert(value.events.some(row => row[0] === 'scroll-context' && row[1] === kind && row[2] === width));
    }
    assert.equal(value.events.filter(row => row[0] === 'image-callback' && row[1] === width).length, 2, 'Execute both actual image callbacks');
    const label = width === 1440 ? 'desktop' : 'mobile';
    const captures = value.events.filter(row => row[0] === 'capture-stand-in' && row[1].includes('/relocation-' + label + '-'));
    assert.deepEqual(captures.map(row => [row[1].split('/relocation-' + label + '-')[1], row[2]]), [['home.png', true], ['search.png', true], ['title.png', true], ['reader.png', false], ['reader-discussion.png', false], ['title-comments.png', false], ['library-settings.png', false], ['public-identity.png', false]]);
    const readerVisible = value.events.findIndex(row => row[0] === 'reader-manage-wait' && row[1] === 'visible' && row[2] === width), readerClose = value.events.findIndex(row => row[0] === 'reader-settings-close' && row[1] === width);
    const readerHidden = value.events.findIndex(row => row[0] === 'reader-manage-wait' && row[1] === 'hidden' && row[2] === width), comments = value.events.findIndex(row => row[0] === 'show-comments' && row[1] === width);
    assert(value.events.indexOf(captures[3]) < readerVisible && readerVisible < readerClose && readerClose < readerHidden && readerHidden < comments, 'Initial reader capture precedes real settings inspection, confirmed dismissal and contextual comments');
    assert.equal(value.events.filter(row => row[0] === 'reader-settings-open' && row[1] === width).length, width === 1440 ? 0 : 1, 'Default desktop sidebar starts open while mobile starts closed');
    const arrival = value.events.findIndex(row => row[0] === 'frame' && row[1] === 'heading' && row[2] === 'Library' && row[3] === width);
    const manage = value.events.findIndex(row => row[0] === 'manage-link' && row[1] === width);
    assert(manage < arrival && arrival < value.events.indexOf(captures[6]), 'Actual Manage arrival is measured before the Library capture');
    assert.equal(value.events.slice(manage + 1, arrival).some(row => row[0].startsWith('scroll')), false, 'No scroll may repair the actual Manage arrival before measuring it');
    for (const [kind, definitions] of Object.entries(tools)) {
      assert.deepEqual(value.events.filter(row => row[0] === 'tool-open' && row[1] === kind && row[3] === width).map(row => row[2]), Object.values(definitions));
      assert.deepEqual(value.events.filter(row => row[0] === 'tool-close' && row[1] === kind && row[3] === width).map(row => row[2]), Object.values(definitions));
    }
    assert.deepEqual(value.events.filter(row => row[0] === 'tab-click' && row[2] === width).map(row => row[1]), ['Public identity', 'Library', 'Preferences']);
  }
  assert.equal(value.events.filter(row => row[0] === 'manage-link').length, 2);
  assert.equal(value.events.filter(row => row[0] === 'source-loader').length, 4);
  assert.equal(value.events.filter(row => row[0] === 'reload').length, 2);
  assert.equal(value.events.filter(row => row[0] === 'auth').length, 7);
  assert.equal(value.events.filter(row => row[0] === 'target-input-check').length, 8);
  assert.equal(value.report.screenshots.length, 16); assert.equal(new Set(value.report.screenshots).size, 16);
  assert.deepEqual(value.result.screenshots, value.report.screenshots);
  assert.equal(value.result.observations.length, 16);
  assert(value.result.observations.every(row => !row.horizontal_overflow));
  assert.deepEqual(value.result.manage_arrivals_before_scroll.map(row => row.viewport), ['desktop', 'mobile']);
  assert.equal(value.result.native_settings_tabs, true); assert.equal(value.result.protected_settings_direct_navigation, true);
  assert.equal(value.result.screenshots_require_separate_visual_inspection, true);
});

for (const initialReaderOpen of [true, false]) test('actual reader settings inspection handles initially ' + (initialReaderOpen ? 'open' : 'closed') + ' native sidebars', async () => {
  const value = await probe(undefined, { initialReaderOpen }); assert.equal(value.error, undefined);
  for (const width of [1440, 390]) {
    assert.equal(value.events.filter(row => row[0] === 'reader-settings-open' && row[1] === width).length, initialReaderOpen ? 0 : 1, 'Toggle only a genuinely closed active sidebar');
    assert.equal(value.events.filter(row => row[0] === 'reader-settings-close' && row[1] === width).length, 1);
    assert(value.events.some(row => row[0] === 'reader-manage-wait' && row[1] === 'hidden' && row[2] === width), 'Close must hide the active Manage link even on desktop');
  }
  assert.equal(value.report.screenshots.length, 16);
});

test('actual reader close must hide Manage rather than merely dismiss a dialog', async () => {
  const value = await probe('reader-close-retains-panel');
  assert.match(value.error?.message ?? '', /Active reader Manage visibility must follow native sidebar state/);
  assert.equal(value.result, undefined);
  assert.equal(value.events.some(row => row[0] === 'show-comments'), false);
});

test('actual reader close selects its header toggle despite the legitimate mobile Sheet X', async () => {
  const value = await probe(); assert.equal(value.error, undefined);
  assert(value.events.some(row => row[0] === 'reader-close-candidates' && row[1] === 2 && row[2] === 1 && row[3] === 390));
  const reader = fs.readFileSync(new URL('../../frontend/src/lib/components/reader/reader-panel.svelte', import.meta.url), 'utf8');
  const header = reader.slice(reader.indexOf('<Sidebar.Header'), reader.indexOf('</Sidebar.Header>'));
  assert.match(header, /<Button[^>]*onclick=\{sidebar.toggle\}>\s*<XIcon/);
  assert.match(fs.readFileSync(new URL('../../frontend/src/lib/components/ui/sidebar/sidebar-header.svelte', import.meta.url), 'utf8'), /data-slot="sidebar-header"/);
});

for (const [fault, message] of [
  ['authenticated', /requires an anonymous browser/], ['unsafe-run-id', /Unsafe relocation evidence/],
  ['global-mount', /remain mounted outside Settings/], ['overflow', /overflows the viewport/],
  ['undecoded-image', /did not decode genuine local media/], ['offscreen-image', /outside the initial unobstructed viewport/],
  ['expanded-comments', /expanded before a contextual user action/], ['context-not-expanded', /did not expand/], ['missing-source-loader', /no read-only loading control/],
  ['generic-target-input', /editing a different resource target/], ['offscreen-context', /source comment controls is outside/],
  ['wrong-manage-href', /Reader Manage library link missed its native/], ['wrong-library-manage-href', /Manage library does not point to its native Settings tab/],
  ['duplicate-library', /exactly one route-specific management surface/], ['missing-identity', /exactly one route-specific management surface/],
  ['cross-mounted-panel', /exactly one route-specific management surface/], ['preferences-panel', /exactly one route-specific management surface/],
  ['settings-conversation', /exactly one route-specific management surface/], ['outside-main', /outside the native Settings main/],
  ['missing-tab', /tab is missing, duplicated or incorrectly selected/], ['duplicate-tab', /tab is missing, duplicated or incorrectly selected/],
  ['wrong-tab-href', /tab is missing, duplicated or incorrectly selected/], ['wrong-active-tab', /tab is missing, duplicated or incorrectly selected/],
  ['protected-tab', /exposed protected account tabs/], ['accordion', /still use accordions/], ['preopened-tool', /Technical tools mounted before/],
  ['scope-controls-main', /scope controls leaked onto Library main/],
  ['offscreen-arrival', /Manage library arrival heading is outside/], ['occluded-arrival', /Manage library arrival heading is outside/], ['offscreen-identity', /Public identity arrival heading is outside/],
  ['duplicate-dialog', /exactly one accordion-free dialog/], ['duplicate-tool', /exactly one accordion-free dialog/], ['tool-accordion', /exactly one accordion-free dialog/],
  ['oversize-dialog', /dialog is outside/], ['tool-remains', /Dismissed native tool remained mounted/],
  ['reload-hash', /reload lost its native route/], ['reload-account', /created account authority/],
  ['unguarded-route', /protected navigation must reach its expected destination/],
  ['protected-account-authority', /Direct protected Settings navigation created account authority/],
  ['protected-controls', /exposed account or API client controls/], ['api-controls', /exposed account or API client controls/],
]) test('actual relocation gate rejects ' + fault, async () => {
  const value = await probe(fault); assert.match(value.error?.message ?? '', message); assert.equal(value.result, undefined);
});


test('failed Manage arrival retains exact coordinates and DOM hit-test before any screenshot scroll', async () => {
  for (const fault of ['offscreen-arrival', 'occluded-arrival']) {
    const value = await probe(fault);
    assert.match(value.error?.message ?? '', /Manage library arrival heading is outside/);
    const saved = value.report.relocation_geometry.find(row => row.surface === 'desktop Manage library arrival heading');
    assert(saved, 'Failure evidence must survive the step throwing');
    assert.equal(saved.viewport_width, 1440); assert.equal(saved.viewport_height, 1000);
    assert.equal(saved.unobstructed, fault !== 'occluded-arrival');
    if (fault === 'offscreen-arrival') assert.equal(saved.bottom, 1011);
    assert.equal(value.events.some(row => row[0] === 'tool-open'), false, 'No later tool action may run after failed arrival');
  }
});

test('actual reload waits for the real Library owner rather than accepting a different route', async () => {
  const value = await probe('reload-route'); assert.equal(value.error, value.original); assert.equal(value.result, undefined);
});

for (const fault of ['wrong-chapter', 'wrong-manga', 'source-loader-driver', 'missing-manage-link', 'reader-toggle-driver', 'reader-close-driver', 'capture-failure']) test('actual step preserves original driver error for ' + fault, async () => {
  const value = await probe(fault); assert.equal(value.error, value.original); assert.equal(value.result, undefined);
});
