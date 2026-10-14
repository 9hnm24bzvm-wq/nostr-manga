#!/usr/bin/env node
// Execute the actual relocation step and management helper with explicit driver
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
const summaries = ['Advanced library settings', 'Public sync & recovery', 'Signing identity & instances', 'Public profile and follows', 'Saved public actions'];
const nativeRoutes = ['/', '/search', `/title/${manga}`, `/chapter/${chapter}/1`];

async function probe(fault) {
  const events = [], report = { screenshots: [] }, original = Error('Original stand-in driver failure: ' + fault);
  let url = new URL('/', origin), viewport = { width: 1440, height: 1000 }, shown = false, expanded = false, reloads = 0, result;
  const isSettings = () => url.pathname === '/settings/preferences';
  const atReader = () => url.pathname === `/chapter/${chapter}/1`;
  const visible = async predicate => { if (!predicate()) throw original; };
  const dom = operation => vm.runInNewContext('(' + operation.toString() + ')()', {
    innerWidth: viewport.width, innerHeight: viewport.height,
    document: { documentElement: { scrollWidth: viewport.width + (fault === 'overflow' ? 2 : 0) } },
  });
  const section = kind => ({
    async count() { return isSettings() ? kind === 'library' && fault === 'duplicate-library' ? 2 : kind === 'identity' && fault === 'missing-identity' ? 0 : 1 : 0; },
    async waitFor(options) { assert.deepEqual(options, { state: 'visible' }); await visible(isSettings); },
    async scrollIntoViewIfNeeded() { assert(isSettings()); events.push(['scroll', kind]); },
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
        if (role === 'button' && options.name === 'Signed comments') return {
          async getAttribute(name) { assert.equal(name, 'aria-expanded'); assert(bound()); return expanded || fault === 'expanded-comments' ? 'true' : 'false'; },
          async click() { assert(bound()); expanded = true; events.push(['signed-comments', viewport.width]); },
        };
        assert.equal(role, 'link'); assert.equal(options.name, 'Manage public identity');
        return { async getAttribute(name) {
          assert.equal(name, 'href'); assert(bound() && expanded);
          if (fault === 'missing-prompt') throw original;
          events.push(['identity-prompt', viewport.width]);
          return fault === 'wrong-identity-link' ? '/auth/login' : '/settings/preferences#public-identity';
        } };
      },
      getByLabel(label, options) {
        assert.equal(options.exact, true); assert(['Current instance catalogue ID', 'Target type'].includes(label));
        return { async count() { assert(bound()); events.push(['target-input-check', label]); return fault === 'generic-target-input' ? 1 : 0; } };
      },
    };
  };
  const page = {
    async setViewportSize(value) { viewport = { ...value }; events.push(['viewport', value.width, value.height]); },
    async screenshot(options) {
      assert.match(options.path, /^\/stand-in-only\/test-evidence\/frontend\/browser\/local-library-standin\/relocation-(desktop|mobile)-[a-z-]+\.png$/);
      assert.equal(typeof options.fullPage, 'boolean');
      if (fault === 'capture-failure') throw original;
      events.push(['capture-stand-in', options.path, options.fullPage]);
    },
    async evaluate(operation) { return dom(operation); },
    locator(selector) {
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
      if (selector === '[data-source-conversation]') return { async count() { assert(isSettings()); return fault === 'settings-conversation' ? 1 : 0; } };
      if (selector === '#public-identity') return section('identity');
      assert.equal(selector, 'summary');
      return { filter({ hasText }) {
        const name = summaries.find(value => hasText.test(value)); assert(name, 'Expected exact known disclosure matcher');
        assert(!hasText.test(name + ' extra'), 'Disclosure matcher must be exact');
        return { locator(parent) {
          assert.equal(parent, '..');
          return { async count() { assert(isSettings()); events.push(['disclosure', name, viewport.width]); return fault === 'missing-disclosure' ? 0 : fault === 'duplicate-disclosure' ? 2 : 1; },
            async getAttribute(attribute) { assert.equal(attribute, 'open'); return fault === 'open-disclosure' ? '' : null; } };
        } };
      } };
    },
    getByRole(role, options) {
      assert.equal(options.exact, true);
      if (role === 'button') {
        assert.equal(options.name, 'Show Comments');
        return { async isVisible() { assert(atReader()); return true; }, async click() { assert(atReader()); shown = true; events.push(['show-comments', viewport.width]); } };
      }
      assert.equal(role, 'link'); assert.equal(options.name, 'Manage library');
      return { async click() { assert.equal(url.pathname, '/library'); if (fault === 'missing-manage-link') throw original; events.push(['manage-link', viewport.width]); url = new URL('/settings/preferences#local-library', origin); } };
    },
    async waitForURL(predicate) { assert(predicate(url), 'Actual management helper must verify its settings deep link'); },
    async reload(options) { assert.deepEqual(options, { waitUntil: 'domcontentloaded' }); assert(isSettings()); reloads++; if (fault === 'reload-hash') url.hash = ''; events.push(['reload', viewport.width]); },
    url() { return url.href; },
  };
  const globals = {
    page, phase: 'local-library', manga, chapter, report, journeyRootUrl: new URL('file:///stand-in-only/'), fileURLToPath, URL,
    process: { env: { BROWSER_RUN_ID: fault === 'unsafe-run-id' ? '../unsafe' : 'standin' } },
    async api(route) { assert.equal(route, '/auth/check'); events.push(['auth', reloads]); return { status: fault === 'authenticated' || fault === 'reload-account' && reloads > 0 ? 200 : 401 }; },
    async open(route) { events.push(['open', route, viewport.width]); url = new URL(route, origin); shown = false; expanded = false; },
    async until(read, valid, message) { const value = await read(); assert(valid(value), message); return value; },
    insist(value, message) { assert(value, message); },
    async step(label, operation) { assert.match(label, /^relocated native Library/); result = await operation(); },
  };
  let error;
  try { await new AsyncFunction(...Object.keys(globals), actualSource)(...Object.values(globals)); } catch (cause) { error = cause; }
  return { error, original, events, report, result };
}

test('actual step visits both configured native layouts, contextual actions and unique settings with capture stand-ins', async () => {
  const value = await probe(); assert.equal(value.error, undefined);
  assert.deepEqual(value.events.filter(row => row[0] === 'viewport'), [['viewport', 1440, 1000], ['viewport', 390, 844], ['viewport', 1440, 1000]]);
  for (const width of [1440, 390]) {
    assert.deepEqual(value.events.filter(row => row[0] === 'open' && row[2] === width).map(row => row[1]), [...nativeRoutes, `/title/${manga}?tab=comments`, '/library']);
    for (const kind of ['chapter', 'manga']) assert(value.events.some(row => row[0] === 'conversation' && row[1] === kind && row[3] === width));
    assert.equal(value.events.filter(row => row[0] === 'disclosure' && row[2] === width).length, summaries.length);
    assert.equal(value.events.filter(row => row[0] === 'image-callback' && row[1] === width).length, 2, 'Execute both actual image callbacks');
    const label = width === 1440 ? 'desktop' : 'mobile';
    const captures = value.events.filter(row => row[0] === 'capture-stand-in' && row[1].includes('/relocation-' + label + '-'));
    assert.deepEqual(captures.map(row => [row[1].split('/relocation-' + label + '-')[1], row[2]]), [['home.png', true], ['search.png', true], ['title.png', true], ['reader.png', false], ['reader-discussion.png', false], ['title-comments.png', false], ['library-settings.png', false], ['public-identity.png', false]]);
    assert(value.events.indexOf(captures[3]) < value.events.findIndex(row => row[0] === 'show-comments' && row[1] === width), 'Initial reader capture precedes contextual comments');
  }
  assert.equal(value.events.filter(row => row[0] === 'manage-link').length, 2);
  assert.equal(value.events.filter(row => row[0] === 'identity-prompt').length, 2);
  assert.equal(value.events.filter(row => row[0] === 'reload').length, 2);
  assert.equal(value.events.filter(row => row[0] === 'auth').length, 3);
  assert.equal(value.events.filter(row => row[0] === 'target-input-check').length, 4);
  assert.equal(value.report.screenshots.length, 16); assert.equal(new Set(value.report.screenshots).size, 16);
  assert.deepEqual(value.result.screenshots, value.report.screenshots);
  assert.equal(value.result.observations.length, 16);
  assert(value.result.observations.every(row => !row.horizontal_overflow));
  assert.equal(value.result.screenshots_require_separate_visual_inspection, true);
});

for (const [fault, message] of [
  ['authenticated', /requires an anonymous browser/], ['unsafe-run-id', /Unsafe relocation evidence/],
  ['global-mount', /remain mounted outside Settings/], ['overflow', /overflows the viewport/],
  ['undecoded-image', /did not decode genuine local media/], ['offscreen-image', /outside the initial unobstructed viewport/],
  ['expanded-comments', /expanded before a contextual user action/], ['wrong-identity-link', /no anonymous signer Settings path/],
  ['generic-target-input', /editing a different resource target/], ['duplicate-library', /exactly one management surface/],
  ['missing-identity', /exactly one management surface/], ['settings-conversation', /exactly one management surface/],
  ['missing-disclosure', /disclosure is missing, duplicated or open/], ['duplicate-disclosure', /disclosure is missing, duplicated or open/],
  ['open-disclosure', /disclosure is missing, duplicated or open/], ['reload-hash', /reload lost its deep link/], ['reload-account', /created account authority/],
]) test('actual relocation gate rejects ' + fault, async () => {
  const value = await probe(fault); assert.match(value.error?.message ?? '', message); assert.equal(value.result, undefined);
});

for (const fault of ['wrong-chapter', 'wrong-manga', 'missing-prompt', 'missing-manage-link', 'capture-failure']) test('actual step preserves original driver error for ' + fault, async () => {
  const value = await probe(fault); assert.equal(value.error, value.original); assert.equal(value.result, undefined);
});
