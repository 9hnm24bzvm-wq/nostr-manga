// Execute the actual four-stage composed reader script. Reader methods, DOM
// targets and animation scheduling are finite seams; no browser is launched.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import test from 'node:test';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
import {createReaderOverlays} from '../../platform-integrations/frontend-reader-overlays.mjs';
import {createLocalLibraryOverlays} from '../../platform-integrations/local-library/overlays.mjs';
import {createPublicApiOverlays} from '../../platform-integrations/public-api/overlays.mjs';

const frontend = path.resolve(new URL('../../frontend', import.meta.url).pathname);
const file = path.join(frontend, 'src/lib/components/reader/reader-keyboard-listener.svelte');
const original = fs.readFileSync(file, 'utf8');
function composed(enabled) {
  let code = original;
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, {enabled})]) code = plugin.transform.handler(code, file)?.code ?? code;
  return code;
}
const sources = [composed(false), composed(true)];
function keyboard(direction, code) {
  const body = code.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, '');
  class Element { closest() { return null; } }
  class HTMLElement extends Element { isContentEditable = false; }
  const prefs = {layout: 'paged', direction, keyboard_navigation: true, scroll: 10};
  const actions = [], cancelled = [], scheduled = [];
  const reader = {prefs, scroll: {locked: false},
    gotoPreviousPage() { actions.push('previous-page'); }, gotoNextPage() { actions.push('next-page'); },
    gotoPreviousChapter() { actions.push('previous-chapter'); }, gotoNextChapter() { actions.push('next-chapter'); }};
  const handler = vm.runInNewContext(stripTypeScriptTypes(body, {mode: 'transform'}) + '\nonKeyDown;', {
    Element, HTMLElement, $readerPrefs: prefs, useReader: () => reader, getIsOpen: () => false,
    Constants: {Reader: {Layout: {Scroll: 'scroll'}, Direction: {RTL: 'rtl', LTR: 'ltr'}}},
    onMount() {}, onDestroy() {}, requestAnimationFrame(callback) { scheduled.push(callback); return scheduled.length; },
    window: {cancelAnimationFrame(id) { cancelled.push(id); }, scrollBy() { actions.push('scroll'); }}
  });
  function press(code, extra = {}) {
    const target = new Element(), before = actions.length;
    const result = {prevented: false, propagationStopped: false, immediateStopped: false};
    handler({code, target, ctrlKey: false, altKey: false, metaKey: false, isComposing: false, defaultPrevented: false,
      composedPath: () => [target], preventDefault() { result.prevented = true; }, stopPropagation() { result.propagationStopped = true; }, stopImmediatePropagation() { result.immediateStopped = true; }, ...extra});
    return {...result, actions: actions.slice(before)};
  }
  return {press, scheduled, cancelled};
}

for (const flag of ['altKey', 'metaKey', 'ctrlKey', 'isComposing']) test('actual composed reader yields ' + flag + ' without consuming navigation', () => {
  for (const code of sources) for (const direction of ['ltr', 'rtl']) {
    const reader = keyboard(direction, code);
    for (const key of ['ArrowLeft', 'ArrowRight', 'BracketLeft', 'BracketRight']) assert.deepEqual(reader.press(key, {[flag]: true}), {prevented: false, propagationStopped: false, immediateStopped: false, actions: []}, direction + ' ' + flag + ' ' + key);
  }
});

test('plain LTR and RTL page/chapter shortcuts retain their actual navigation and event ownership', () => {
  const ltr = {'ArrowLeft': 'previous-page', 'ArrowRight': 'next-page', 'BracketLeft': 'previous-chapter', 'BracketRight': 'next-chapter'};
  const rtl = {'ArrowLeft': 'next-page', 'ArrowRight': 'previous-page', 'BracketLeft': 'next-chapter', 'BracketRight': 'previous-chapter'};
  for (const code of sources) for (const [direction, expected] of [['ltr', ltr], ['rtl', rtl]]) {
    const reader = keyboard(direction, code);
    for (const [key, action] of Object.entries(expected)) assert.deepEqual(reader.press(key), {prevented: true, propagationStopped: true, immediateStopped: true, actions: [action]}, direction + ' ' + key);
  }
});

test('yielding a modified or composition event also stops an already held reader-scroll frame', () => {
  for (const code of sources) for (const flag of ['altKey', 'metaKey', 'ctrlKey', 'isComposing']) {
    const reader = keyboard('ltr', code);
    reader.press('ArrowDown'); assert.equal(reader.scheduled.length, 1);
    assert.equal(reader.press('ArrowLeft', {[flag]: true}).prevented, false);
    assert.deepEqual(reader.cancelled, [1]);
  }
});

test('composition preserves canonical reader bytes and all markup outside its script', () => {
  const markup = value => value.replace(/<script[^>]*>[\s\S]*?<\/script>/, '<script></script>');
  for (const code of sources) assert.equal(markup(code), markup(original));
  assert.equal(fs.readFileSync(file, 'utf8'), original);
});
