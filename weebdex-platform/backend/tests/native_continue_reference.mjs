// Test-only oracle. Never reimplement the old algorithm or numerically sort
// labels here: consume actual native aggregate SQL order and composed source.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const target = path.join(frontend, 'src/routes/(app)/title/MangaActions.svelte');
const original = fs.readFileSync(target, 'utf8');
const transformed = createFrontendOverlays(frontend).transform.handler(original, target).code;
const start = transformed.indexOf('\t\t\t\t\tif (history) {');
const end = transformed.indexOf('\t\t\t\t\tif (targetAggregatedChapter) {', start);
assert(start > 0 && end > start, 'Actual composed retained Continue block is required');
const block = transformed.slice(start, end);
assert.match(block, /else targetAggregatedChapter = aggregated\.chapters\[0\]/);
const select = new Function('aggregated', 'history', 'console', 'let targetAggregatedChapter;\n' + block + '\nreturn targetAggregatedChapter;');
const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const chosen = input.aggregate.chapters.length ? select(input.aggregate, input.history, { info() {} }) : undefined;
const entries = chosen ? Object.entries(chosen.entries) : [];
process.stdout.write(JSON.stringify({
  target: chosen ? { volume: chosen.volume, chapter: chosen.chapter, anchorChapterId: entries[0][0] } : null,
  winnerId: entries.length ? entries[0][0] : null,
  hasMultipleEditions: entries.length > 1,
}));
