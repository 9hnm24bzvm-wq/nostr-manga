#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays, frontendApprovedReaderControlExceptions } from '../../platform-integrations/frontend-overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const plugin = createFrontendOverlays(frontend);
const expected = {
  'src/lib/components/reader/reader-page-selector.svelte': 5,
  'src/lib/components/reader/reader-chapter-selector.svelte': 5,
  'src/lib/components/reader/reader-pagination.svelte': 6,
  'src/lib/components/reader/reader-panel.svelte': 3,
  'src/lib/components/comments/comments-votes.svelte': 2,
  'src/lib/components/reader/reader-page-image.svelte': 2,
  'src/lib/components/reader/reader-settings.svelte': 2,
  'src/lib/components/reader/reader-last-page.svelte': 2
};
const transformed = relative => {
  const file = path.join(frontend, relative), source = fs.readFileSync(file, 'utf8');
  return plugin.transform.handler(source, file).code;
};

test('reader accessibility and narrow-phone changes have exact bounded markup scope', () => {
  assert.equal(frontendApprovedReaderControlExceptions.length, 27);
  for (const [relative, count] of Object.entries(expected)) {
    const patches = frontendApprovedReaderControlExceptions.filter(row => row.path === relative);
    assert.equal(patches.length, count, relative);
    const source = fs.readFileSync(path.join(frontend, relative), 'utf8');
    const executed = transformed(relative);
    for (const patch of patches) {
      assert.equal(patch.count, 1);
      assert.equal(source.split(patch.before).length - 1, 1);
      assert.equal(executed.split(patch.after).length - 1, 1);
    }
  }
});

test('page and chapter icon names follow their actual RTL and LTR actions', () => {
  for (const [relative, noun] of [['reader-page-selector.svelte', 'page'], ['reader-chapter-selector.svelte', 'chapter']]) {
    const code = transformed('src/lib/components/reader/' + relative);
    const labels = [...code.matchAll(/aria-label=\{([^}]+)\}/g)].map(match => new Function('$readerPrefs', 'Constants', 'return ' + match[1]));
    assert.equal(labels.length, 4);
    const constants = { Reader: { Direction: { LTR: 0, RTL: 1 } } };
    assert.deepEqual(labels.map(fn => fn({ direction: 0 }, constants)), ['First', 'Previous', 'Next', 'Last'].map(label => label + ' ' + noun));
    assert.deepEqual(labels.map(fn => fn({ direction: 1 }, constants)), ['Last', 'Next', 'Previous', 'First'].map(label => label + ' ' + noun));
    assert(code.includes('aria-label="Select ' + noun + '"'));
  }
});

test('narrow Direction fix only changes <=360px spacing, text and icon size', () => {
  const patches = frontendApprovedReaderControlExceptions.filter(row => row.approved_reader_control_exception === 'reader_narrow_direction_fit');
  assert.equal(patches.length, 2);
  const suffix = ' max-[360px]:gap-1 max-[360px]:px-1 max-[360px]:text-xs max-[360px]:[&>svg]:size-4';
  for (const patch of patches) {
    assert.equal(patch.after.replace(suffix, ''), patch.before);
    assert(!patch.after.includes('h-') && !patch.after.includes('height') && !patch.after.includes('display'));
  }
});

test('original source hash fences reject drift and earlier markup guards stay exact', () => {
  const file = path.join(frontend, 'src/lib/components/reader/reader-panel.svelte');
  assert.throws(() => plugin.transform.handler(fs.readFileSync(file, 'utf8') + '\n', file), /Overlay input differs/);
  const prepare = fs.readFileSync(path.join(root, 'scripts/prepare-frontend-integration.mjs'), 'utf8');
  assert.match(prepare, /frontendApprovedReaderControlExceptions\.length !== 27/);
  assert.match(prepare, /frontendApprovedPanelExceptions\.length !== approvedPanels\.length/);
  assert.match(prepare, /retainedMarkup\(original\) !== retainedMarkup\(normalizedResult\)/);
  assert.match(prepare, /hash\(original\) !== exception\.sha256/);
});

 test('Stretch Width alone expands the image wrapper without changing aspect-ratio rules', () => {
  const patches = frontendApprovedReaderControlExceptions.filter(row => row.approved_reader_control_exception === 'reader_stretch_width_fit');
  assert.equal(patches.length, 1);
  assert.equal(patches[0].after.replace(' class:w-full={$readerPrefs.scale === Constants.Reader.Scale.StretchWidth}', ''), patches[0].before);
  const code = transformed('src/lib/components/reader/reader-page-image.svelte');
  assert(code.includes('style="aspect-ratio: {width}/{height}"'));
  assert(code.includes('class="object-contain"'));
  assert(code.includes('class:h-auto={[Constants.Reader.Scale.LimitWidth, Constants.Reader.Scale.StretchWidth].includes($readerPrefs.scale)}'));
});
