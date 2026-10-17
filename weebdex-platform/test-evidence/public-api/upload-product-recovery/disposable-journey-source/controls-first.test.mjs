#!/usr/bin/env node
// Finite pure seams: real controls plus source-derived DOM/locator stand-ins.
// No browser, DOM package, application, fixture, socket, process or build starts.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { createPublicUploadRecoveryControls, uploadPanel, uploadPageGrid, uploadPageCards, uploadPageCard, uploadPanelButton } from '../../scripts/public-upload-recovery-controls.mjs';

const read = path => fs.readFileSync(new URL('../../' + path, import.meta.url), 'utf8');
const original = path => read('frontend/src/lib/components/' + path);
const names = ['01-page.png', '02-page.png', '03-page.png'];
const reordered = [names[2], names[0], names[1]];
const origin = 'http://127.0.0.1:31415', pathname = '/title/synthetic/upload';
const PANEL = 'section:has(> header > h2:text-is("Upload Chapter"))';
const GRID = ':scope > form > div.space-y-4 > div.grid[style*="--columns"]';
const CARDS = ':scope > div.relative[class~="!visible"]';
const SHADOW = 'data-is-dnd-shadow-item-internal', ORIGINAL = 'data-is-dnd-original-dragged-item', CLONE = 'dnd-action-dragged-el';

// The pure tests read, but never import or compile, preserved application source.
test('scopes and exact preview-loading topology are present in pinned preserved source and signed overlay', () => {
  const anchors = JSON.parse(read('platform-integrations/public-api/upload-anchors.json'));
  for (const file of anchors.files.filter(file => file.path.startsWith('src/lib/components/upload/'))) {
    const source = read('frontend/' + file.path);
    assert.equal(createHash('sha256').update(source).digest('hex'), file.source_sha256);
  }
  const chapter = original('upload/UploadChapter.svelte');
  assert.match(chapter, /<section[^>]*>\s*<header[^>]*>\s*<h2[^>]*>Upload Chapter<\/h2>\s*<\/header>\s*<form/);
  const pages = original('upload/UploadChapterPages.svelte');
  assert.match(pages, /<div class="space-y-4">/);
  assert.match(pages, /class="grid grid-cols-\[repeat\(var\(--columns\),minmax\(0,1fr\)\)\] gap-8 select-none"/);
  assert.match(pages, /flipDurationMs: 100/);
  assert.match(pages, /class="!visible relative !border-none !ring-0"\s*class:dragging=\{isDragging\}/);
  assert.match(pages, /\{\/each\}\s*<!--[\s\S]*?<div>\s*<AspectRatio ratio=\{10 \/ 14\}>/);
});

// Minimal read-only nodes implement only selectors exercised by domSnapshot.
// They do not implement mutation APIs or synthesize any browser events.
function card(name, extra = {}) { return { card: true, names: name === null ? [] : [name], images: name === null ? [] : [{ alt: name, decoded: true }], loading: 0, shadow: false, dragging: false, ...extra }; }
const add = () => ({ card: false, add: true, names: [], images: [], loading: 0, shadow: false, dragging: false });
function frame(order = names, extra = {}) { return { children: [...order.map(name => card(name)), add()], clones: 0, originals: 0, shadows: 0, ...extra }; }
function shadowFrame(order = [null, names[0], names[1]], extra = {}) {
  const value = frame(order, { clones: 1, originals: 1, shadows: 1, ...extra });
  value.children[order.indexOf(null)] = card(null, { shadow: true, dragging: true });
  return value;
}
function nodeFor(row) {
  return {
    matches(selector) {
      if (selector === 'div.relative[class~="!visible"]') return row.card;
      assert.equal(selector, 'div'); return true;
    },
    hasAttribute(name) { assert.equal(name, SHADOW); return row.shadow; },
    classList: { contains(name) { assert.equal(name, 'dragging'); return row.dragging; } },
    querySelectorAll(selector) {
      if (selector === 'h3') return row.names.map(textContent => ({ textContent }));
      if (selector === 'img') return row.images.map(image => ({ getAttribute(name) { assert.equal(name, 'alt'); return image.alt; }, complete: image.decoded, naturalWidth: image.decoded ? 32 : 0, naturalHeight: image.decoded ? 48 : 0 }));
      if (selector === 'button[data-slot="button"]') return row.add ? [{}] : [];
      assert.equal(selector, '[data-slot="empty"] > [role="status"][aria-label="Loading"]'); return Array(row.loading).fill({});
    },
  };
}
function gridFor(value) {
  return { children: value.children.map(nodeFor), ownerDocument: { querySelectorAll(selector) {
    const key = selector === '[id="' + CLONE + '"]' ? 'clones' : selector === '[' + ORIGINAL + ']' ? 'originals' : selector === '[' + SHADOW + ']' ? 'shadows' : assert.fail('Unknown global selector: ' + selector);
    return Array(value[key]).fill({});
  } } };
}
function harness(options = {}) {
  const h = { time: 0, url: origin + pathname, phase: 'initial', events: [], selectors: [], frames: [], panelCount: 1, gridCount: 1, pageNames: names, healthCalls: 0, moves: 0, after: options.after ?? (() => {}), ...options };
  h.snapshot = options.snapshot ?? (() => {
    if (h.phase === 'drag-target') return shadowFrame();
    if (h.phase === 'released') return h.frames.length ? h.frames.shift() : frame(reordered);
    return frame(h.pageNames);
  });
  const operation = async (name, result) => { h.events.push(name); await h.after(name, h); return typeof result === 'function' ? result() : result; };
  const locator = (kind, value) => ({
    locator(selector) {
      h.selectors.push(selector);
      if (kind === 'panel' && selector === GRID) return locator('grid');
      if (kind === 'grid' && selector === CARDS) return locator('cards');
      if (kind === 'panel' && selector === ':scope > form button[data-slot="button"]:visible') return locator('buttons');
      assert.fail('Unknown scoped selector: ' + kind + '/' + selector);
    },
    filter({ has, hasText }) {
      if (kind === 'cards') { assert.ok(has?.heading); return locator('card', has.heading); }
      assert.equal(kind, 'buttons'); assert.ok(hasText instanceof RegExp); return locator('button', hasText);
    },
    async count() {
      return operation(kind + '-count', () => kind === 'panel' ? h.panelCount : kind === 'grid' ? h.gridCount : kind === 'card' ? h.pageNames.filter(name => name === value).length : kind === 'button' ? (h.buttonNames ?? ['Not affiliated with any group']).filter(name => value.test(name)).length : h.pageNames.length);
    },
    async evaluate(callback, markers) { assert.equal(kind, 'grid'); return operation('snapshot', () => callback(gridFor(h.snapshot()), markers)); },
    async scrollIntoViewIfNeeded({ timeout }) { assert.equal(kind, 'card'); assert.ok(timeout > 0 && timeout <= 1000); return operation('scroll'); },
    async boundingBox() { assert.equal(kind, 'card'); return operation('box-' + value, () => ({ x: names.indexOf(value) * 150 + 20, y: 100, width: 100, height: 140 })); },
    async click({ timeout }) { assert.equal(kind, 'button'); assert.ok(timeout > 0 && timeout <= 1000); return operation('click'); },
  });
  h.page = {
    url: () => h.url,
    viewportSize: () => ({ width: 1000, height: 800 }),
    locator(selector) {
      h.selectors.push(selector);
      if (selector === PANEL) return locator('panel');
      if (selector.startsWith('h3:text-is(') && selector.endsWith(')')) return { heading: JSON.parse(selector.slice(11, -1)) };
      assert.fail('Unknown root selector: ' + selector);
    },
    mouse: {
      async move(x, y, options) {
        assert.ok(Number.isFinite(x) && Number.isFinite(y));
        h.moves++; const number = h.moves;
        return operation('move-' + number, () => { if (number === 2) h.phase = 'drag-start'; if (number === 3) { assert.deepEqual(options, { steps: 12 }); h.phase = 'drag-target'; } });
      },
      async down(options) { assert.deepEqual(options, { button: 'left' }); h.pressed = true; return operation('down'); },
      async up(options) { assert.deepEqual(options, { button: 'left' }); return operation('up', () => { h.pressed = false; h.phase = 'released'; }); },
    },
  };
  h.controls = createPublicUploadRecoveryControls(h.page, {
    origin, pathname, timeoutMs: 1000, pollMs: 10, now: () => h.time,
    async assertHealthy() { h.healthCalls++; await h.after('health', h); },
    async sleep(ms) { h.events.push('sleep'); assert.ok(ms > 0 && ms <= 10); h.time += ms; await h.after('sleep', h); },
  });
  return h;
}

// This test makes the source-backed preview checks explicit rather than using a
// generic missing-heading retry. Spinner/Empty source is only read as bytes.
test('the exact loader branch has no filename heading and the signed placeholder uses the same Empty/Spinner', () => {
  const entry = original('upload/UploadChapterPagesEntry.svelte');
  assert.match(entry, /<img[^>]*alt=\{page.original_file_name\}/);
  assert.match(entry, /<h3[^>]*>\s*\{page.original_file_name\}\s*<\/h3>/);
  assert.match(entry, /\{#snippet loader\(\)\}\s*<Empty.Root class="size-full">\s*<Spinner class="size-10" \/>\s*<\/Empty.Root>/);
  const loader = original('ui/ImageLoader.svelte');
  assert.match(loader, /\{#if isLoaded\}\s*\{@render child\(\{ src, width, height \}\)\}\s*\{:else if loader\}\s*\{@render loader\?\.\(\)\}/);
  assert.match(loader, /img.addEventListener\("load",/);
  assert.match(original('ui/spinner/spinner.svelte'), /<Loader2Icon role="status" aria-label="Loading"/);
  assert.match(original('ui/empty/empty.svelte'), /data-slot="empty"/);
  const anchors = JSON.parse(read('platform-integrations/public-api/upload-anchors.json'));
  const preview = anchors.patches.find(patch => patch.label === 'late signed URL loads in its own actual image component');
  assert.ok(preview);
  assert.match(preview.after, /<Empty.Root class="size-full"><Spinner class="size-10" \/><\/Empty.Root>/);
  const source = read('scripts/public-upload-recovery-controls.mjs');
  assert.doesNotMatch(source, /dispatchEvent|\.dragTo\(|\.first\(|\.nth\(|localStorage|indexedDB|fetch\(|XMLHttpRequest|\.sort\(/);
  assert.match(source, /pending: 'dnd-settling'/);
  assert.match(source, /pending: 'preview-loading'/);
});

test('locators remain exact and direct, excluding global filename and first-match fallback', () => {
  const h = harness();
  uploadPanel(h.page); uploadPageGrid(h.page); uploadPageCards(h.page); uploadPageCard(h.page, names[2]); uploadPanelButton(h.page, 'A|B');
  assert.ok(h.selectors.includes(PANEL)); assert.ok(h.selectors.includes(GRID)); assert.ok(h.selectors.includes(CARDS));
  assert.ok(h.selectors.includes('h3:text-is("03-page.png")'));
});

test('strict settled filenames and decoded images are returned without mutation', async () => {
  const h = harness();
  assert.deepEqual(await h.controls.readSettledOrder(names), names);
  assert.ok(h.healthCalls >= 7); assert.equal(h.events.includes('down'), false); assert.equal(h.events.includes('sleep'), false);
});

for (const [label, setup, message] of [
  ['missing page', h => { h.pageNames = names.slice(0, 2); }, /card count/],
  ['duplicate page', h => { h.pageNames = [names[0], names[0], names[2]]; }, /Duplicate/],
  ['unknown page', h => { h.pageNames = [names[0], 'unknown.png', names[2]]; }, /Unknown/],
  ['wrong order', h => { h.pageNames = reordered; }, /order differs/],
  ['missing heading', h => { const value = frame(); value.children[1] = card(null); h.snapshot = () => value; }, /one exact filename/],
  ['conflicting image', h => { const value = frame(); value.children[1].images[0].alt = names[0]; h.snapshot = () => value; }, /Conflicting/],
  ['undecoded image', h => { const value = frame(); value.children[1].images[0].decoded = false; h.snapshot = () => value; }, /has not decoded/],
  ['extra page', h => { const value = frame(); value.children.splice(2, 0, card('extra.png')); h.snapshot = () => value; }, /card count/],
  ['unknown child', h => { const value = frame(); value.children.splice(1, 0, add()); h.snapshot = () => value; }, /child topology/],
  ['duplicate panel', h => { h.panelCount = 2; }, /exactly one upload panel/],
  ['missing panel', h => { h.panelCount = 0; }, /exactly one upload panel/],
  ['duplicate grid', h => { h.gridCount = 2; }, /exactly one upload grid/],
]) test('permanent ' + label + ' is a failure, never pending', async () => {
  const h = harness(); setup(h);
  await assert.rejects(h.controls.readSettledOrder(names), message);
  assert.equal(h.events.includes('sleep'), false);
  assert.equal(h.controls.cleanupSnapshot().fenced, true);
  const count = h.events.length;
  await assert.rejects(h.controls.reorderLastToFirst(names), /fenced/);
  await assert.rejects(h.controls.clickButton('Not affiliated with any group'), /fenced/);
  assert.throws(() => h.controls.panel(), /fenced/); assert.throws(() => h.controls.grid(), /fenced/); assert.throws(() => h.controls.card(names[0]), /fenced/);
  assert.equal(h.events.length, count);
});

test('real pointer last-to-first waits for known clone/shadow cleanup then strictly verifies order', async () => {
  const h = harness();
  h.frames = [shadowFrame(), frame(reordered, { clones: 1 }), frame(reordered, { originals: 1 }), frame(reordered)];
  assert.deepEqual(await h.controls.reorderLastToFirst(names), reordered);
  assert.deepEqual(h.events.filter(name => /^move-|^(down|up)$/.test(name)), ['move-1', 'down', 'move-2', 'move-3', 'up']);
  assert.equal(h.time, 40);
  const cleanup = h.controls.cleanupSnapshot();
  assert.equal(cleanup.downAttempted, true); assert.equal(cleanup.releaseAttempted, true); assert.equal(cleanup.releaseSucceeded, true); assert.equal(cleanup.releaseError, null); assert.equal(cleanup.fenced, false);
});

test('known marker cleanup can settle on a standalone read', async () => {
  const values = [frame(names, { clones: 1 }), shadowFrame(), frame(names)];
  const h = harness({ snapshot: () => values.shift() ?? frame(names) });
  assert.deepEqual(await h.controls.readSettledOrder(names), names); assert.equal(h.time, 20);
});

test('source-backed preview placeholder after drag is separate bounded pending until exact decoded names', async () => {
  const h = harness(); const loading = frame(reordered); loading.children[0] = card(null, { loading: 1 });
  h.frames = [shadowFrame(), loading, frame(reordered)];
  assert.deepEqual(await h.controls.reorderLastToFirst(names), reordered); assert.equal(h.time, 30);
});

for (const phase of ['clone', 'shadow', 'preview']) test('permanent ' + phase + ' lifecycle reaches the same absolute deadline', async () => {
  const stuck = phase === 'clone' ? frame(names, { clones: 1 }) : phase === 'shadow' ? shadowFrame() : (() => { const value = frame(); value.children[1] = card(null, { loading: 1 }); return value; })();
  const h = harness({ snapshot: () => stuck });
  await assert.rejects(h.controls.readSettledOrder(names, { deadlineAt: 35 }), /absolute deadline/);
  assert.equal(h.time, 35); assert.equal(h.events.includes('down'), false);
});

for (const [label, value] of [
  ['unknown filename', (() => { const value = shadowFrame(); value.children[1] = card('wrong.png'); return value; })()],
  ['extra card', (() => { const value = shadowFrame(); value.children.splice(1, 0, card('extra.png')); return value; })()],
  ['duplicate filename', (() => { const value = frame(); value.children[0] = card(names[1], { loading: 1 }); return value; })()],
]) test('known spinner or DnD marker never excuses ' + label, async () => {
  const h = harness({ snapshot: () => value });
  await assert.rejects(h.controls.readSettledOrder(names), /Unknown|card count|Duplicate/);
  assert.equal(h.events.includes('sleep'), false);
});

test('a genuine snapshot locator rejection is preserved without pending retry', async () => {
  const failure = Error('actual driver failed');
  const h = harness({ after(name) { if (name === 'snapshot') throw failure; } });
  await assert.rejects(h.controls.readSettledOrder(names), error => error === failure);
  assert.equal(h.events.includes('sleep'), false);
});

test('late resolved await cannot succeed after the original absolute deadline', async () => {
  const h = harness({ after(name, h) { if (name === 'snapshot') h.time = 1001; } });
  await assert.rejects(h.controls.readSettledOrder(names), /absolute deadline/);
  assert.equal(h.events.includes('sleep'), false);
});

for (const point of ['panel-count', 'snapshot', 'move-2', 'up']) test('origin/route is rechecked after awaited ' + point + ' and release remains mandatory after down', async () => {
  const h = harness({ after(name, h) { if (name === point) h.url = 'http://unowned.invalid/elsewhere'; } });
  await assert.rejects(point === 'panel-count' || point === 'snapshot' ? h.controls.readSettledOrder(names) : h.controls.reorderLastToFirst(names), /exact owned origin or route/);
  assert.equal(h.events.includes('up'), ['move-2', 'up'].includes(point));
  assert.equal(h.controls.cleanupSnapshot().fenced, true);
});

test('external health is rechecked after awaited pointer movement and fences later action', async () => {
  const failure = Error('fixture external-health violation'); let unhealthy = false;
  const h = harness({ after(name) { if (name === 'move-2') unhealthy = true; if (name === 'health' && unhealthy) throw failure; } });
  await assert.rejects(h.controls.reorderLastToFirst(names), error => error === failure);
  assert.equal(h.events.includes('up'), true); assert.equal(h.events.includes('move-3'), false);
  assert.equal(h.controls.cleanupSnapshot().releaseSucceeded, true);
});

for (const rejectRelease of [false, true]) test('rejected pointer down still attempts up and preserves primary error, release rejection=' + rejectRelease, async () => {
  const primary = Error('down may have pressed before rejecting'), release = Error('up rejected');
  const h = harness({ after(name) { if (name === 'down') throw primary; if (name === 'up' && rejectRelease) throw release; } });
  await assert.rejects(h.controls.reorderLastToFirst(names), error => error === primary);
  const cleanup = h.controls.cleanupSnapshot();
  assert.equal(cleanup.downAttempted, true); assert.equal(cleanup.releaseAttempted, true); assert.equal(cleanup.releaseSucceeded, !rejectRelease); assert.equal(cleanup.releaseError, rejectRelease ? release : null); assert.equal(cleanup.primaryError, primary); assert.equal(cleanup.fenced, true);
  const count = h.events.length;
  await assert.rejects(h.controls.reorderLastToFirst(names), /fenced/); assert.equal(h.events.length, count);
});

test('release-only rejection is the primary failure and prevents final settlement reads', async () => {
  const release = Error('up failed'); const h = harness({ after(name) { if (name === 'up') throw release; } });
  await assert.rejects(h.controls.reorderLastToFirst(names), error => error === release);
  const tail = h.events.slice(h.events.indexOf('up') + 1); assert.deepEqual(tail, []);
  assert.equal(h.controls.cleanupSnapshot().releaseError, release);
});

test('deadline after down still releases and separately retains a release rejection', async () => {
  const release = Error('cleanup up failed');
  const h = harness({ after(name, h) { if (name === 'down') h.time = 1000; if (name === 'up') throw release; } });
  await assert.rejects(h.controls.reorderLastToFirst(names), /absolute deadline/);
  assert.equal(h.events.includes('up'), true); assert.equal(h.events.includes('move-2'), false);
  assert.equal(h.controls.cleanupSnapshot().releaseError, release);
});

test('route and health failures discovered after a rejected down never replace that primary error', async () => {
  const primary = Error('down failed'), health = Error('health failed'); let failed = false;
  const h = harness({ after(name, h) { if (name === 'down') { h.url = origin + '/other'; failed = true; throw primary; } if (name === 'health' && failed) throw health; } });
  await assert.rejects(h.controls.reorderLastToFirst(names), error => error === primary);
  assert.equal(h.events.includes('up'), true); assert.ok(h.controls.cleanupSnapshot().guardErrors.includes(health));
});

test('exact escaped button label has no regex-alternative fallback', async () => {
  const h = harness({ buttonNames: ['A', 'B', 'A|B'] });
  await h.controls.clickButton('A|B'); assert.equal(h.events.filter(name => name === 'click').length, 1);
  const missing = harness({ buttonNames: ['A', 'B'] });
  await assert.rejects(missing.controls.clickButton('A|B'), /exactly one upload button/);
});

for (const [label, value] of [
  ['multiple clones', frame(names, { clones: 2 })],
  ['multiple shadows', frame(names, { shadows: 2 })],
  ['foreign shadow', frame(names, { shadows: 1 })],
]) test('unexpected DnD marker topology fails without waiting: ' + label, async () => {
  const h = harness({ snapshot: () => value });
  await assert.rejects(h.controls.readSettledOrder(names), /Unexpected upload DnD/);
  assert.equal(h.events.includes('sleep'), false);
});

test('deadline expiring during release is checked after await before any final read', async () => {
  const h = harness({ after(name, h) { if (name === 'up') h.time = 1000; } });
  await assert.rejects(h.controls.reorderLastToFirst(names), /absolute deadline/);
  assert.equal(h.controls.cleanupSnapshot().releaseSucceeded, true);
  assert.equal(h.events.at(-1), 'up');
});

for (const suffix of ['?other=1', '#other']) for (const point of ['snapshot', 'health', 'move-2']) test('exact upload URL rejects ' + suffix + ' introduced during ' + point, async () => {
  const h = harness({ after(name, h) { if (name === point) h.url = origin + pathname + suffix; } });
  await assert.rejects(point === 'move-2' ? h.controls.reorderLastToFirst(names) : h.controls.readSettledOrder(names), /exact owned origin or route/);
  assert.equal(h.events.includes('up'), point === 'move-2');
  assert.equal(h.controls.cleanupSnapshot().fenced, true);
});

test('global clone marker cannot turn a genuinely broken non-shadow card into pending', async () => {
  const value = frame(names, { clones: 1 }); value.children[1] = card(null);
  const h = harness({ snapshot: () => value });
  await assert.rejects(h.controls.readSettledOrder(names), /one exact filename/);
  assert.equal(h.events.includes('sleep'), false);
});
