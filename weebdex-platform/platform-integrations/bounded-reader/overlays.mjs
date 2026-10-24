// Source-only semantic-reader replacement. Immutable native source hashes and
// exact anchors remain mandatory in both explicitly distinct catalogue profiles.
import fs from 'node:fs';
import { badgePatches, applyBadgeOverlays, reverseBadgeOverlays, assertBadgeExceptionScope } from './badge-overlays.mjs';
import path from 'node:path';
import { createHash } from 'node:crypto';
const patches = JSON.parse(fs.readFileSync(new URL('./anchors.json', import.meta.url), 'utf8'));
const hash = value => createHash('sha256').update(value).digest('hex');
const expectedMarkup = new Map([
  ['src/lib/components/reader/reader-chapter-selector.svelte', ['replace aggregate-only ChapterSelector with actual bounded consumer']],
  ['src/lib/components/reader/reader-group-selector.svelte', ['replace aggregate-only EditionSelector with actual bounded consumer']],
  ['src/lib/components/reader/reader-page-selector.svelte', ['page selector highlights the current spread independently of chapter navigation', 'page selector highlights the current page independently of chapter navigation']],
  ['src/lib/components/reader/reader-pagination.svelte', ['pagination removes duplicate aggregate index selector']],
  ['src/lib/components/reader/reader-last-page.svelte', ['endcard prevChapter owned semantic control 1', 'endcard prevChapter owned semantic control 2', 'endcard nextChapter owned semantic control 1', 'endcard nextChapter owned semantic control 2', 'endcard pending error excluded and restart presentation']],
  ['src/routes/(app)/title/MangaActions.svelte', ['one mode-specific primary and explicit bounded first-chapter alternative', 'retain device-only aggregate chooser with current admission']]
]);
export const boundedReaderOverlayManifest = [...patches, ...badgePatches].map(({ before, after, ...metadata }) => metadata);
export const boundedReaderApprovedMarkupExceptions = [...patches, ...badgePatches].filter(patch => patch.markup);
export function assertBoundedReaderExceptionScope() {
  assertBadgeExceptionScope();
  const readerMarkup = patches.filter(patch => patch.markup);
  if (readerMarkup.length !== 12 || readerMarkup.some(patch => patch.count !== 1 || !expectedMarkup.get(patch.path)?.includes(patch.label)) || [...expectedMarkup].some(([relative, labels]) => labels.some(label => readerMarkup.filter(patch => patch.path === relative && patch.label === label).length !== 1))) throw Error('Unreviewed bounded reader markup extension');
}
const markup = code => code.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g, '<script></script>');
export function assertBoundedReaderMarkup(input, output, relative) {
  if (!relative.endsWith('.svelte')) return;
  let restored = reverseBadgeOverlays(output, relative);
  for (const patch of patches.filter(patch => patch.path === relative).reverse()) {
    // Reverse every exact patch so script-contained markup-like strings cannot
    // accidentally broaden the exception. No arbitrary output normalization.
    if (patch.after === '') continue;
    if (restored.split(patch.after).length - 1 !== patch.count) throw Error(`Bounded reader reverse anchor changed: ${patch.label}`);
    restored = restored.split(patch.after).join(patch.before);
  }
  // Deleted script-only ranges do not change retained markup. Whole-selector
  // replacement and the explicitly listed twelve markup patches are the reader-stage UI delta.
  if (markup(input) !== markup(restored)) throw Error(`Bounded reader altered unapproved markup: ${relative}`);
}
export function createBoundedReaderOverlays(frontend) {
  assertBoundedReaderExceptionScope(); const root = path.resolve(frontend);
  return { name: 'bounded-live-semantic-reader', enforce: 'pre', transform: { order: 'pre', handler(input, id) {
    if (id.includes('?')) return null;
    const relative = path.relative(root, id).split(path.sep).join('/'), entries = patches.filter(patch => patch.path === relative);
    if (!entries.length && !badgePatches.some(patch => patch.path === relative)) return null;
    const original = fs.readFileSync(path.join(root, relative));
    if (entries.length && hash(original) !== entries[0].sha256) throw Error(`Bounded reader original source changed: ${relative}`);
    let code = input;
    for (const patch of entries) {
      if (code.split(patch.before).length - 1 !== patch.count) throw Error(`Bounded reader exact anchor changed: ${patch.label}`);
      code = code.split(patch.before).join(patch.after);
    }
    code = applyBadgeOverlays(code, relative, original);
    assertBoundedReaderMarkup(input, code, relative);
    return { code, map: null };
  } } };
}
