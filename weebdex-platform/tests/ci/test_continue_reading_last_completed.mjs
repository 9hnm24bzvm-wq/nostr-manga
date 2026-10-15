#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays, frontendOverlayManifest } from '../../platform-integrations/frontend-overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const target = path.join(frontend, 'src/routes/(app)/title/MangaActions.svelte');
const original = fs.readFileSync(target, 'utf8');
const transformed = createFrontendOverlays(frontend).transform.handler(original, target).code;
// Exercise the actual source-bound production selection block, rather than a
// separately implemented algorithm. API aggregate order is newest to oldest.
const start = transformed.indexOf('\t\t\t\t\tif (history) {');
const end = transformed.indexOf('\t\t\t\t\tif (targetAggregatedChapter) {', start);
assert(start > 0 && end > start, 'Production Continue Reading selection block missing');
const select = new Function('aggregated', 'history', 'console',
  'let targetAggregatedChapter;\n' + transformed.slice(start, end) + '\nreturn targetAggregatedChapter;');
const silent = { info() {} };
const chapter = (volume, number, ...ids) => ({ volume, chapter: number, entries: Object.fromEntries(ids.map(id => [id, {}])) });
const newest = chapter('2', '1', 'v2c1');
const middle = chapter('1', '10', 'v1c10');
const oldest = chapter('1', '2', 'v1c2en', 'v1c2ja');
const aggregated = { chapters: [newest, middle, oldest] };
const choose = history => select(aggregated, history, silent);

test('all-read device and account history continue at latest completed chapter', () => {
  for (const mode of ['device', 'account']) {
    const history = { v2c1: null, v1c10: null, v1c2en: null };
    assert.equal(choose(history), newest, mode + ' all-read must not restart at oldest chapter');
  }
});

test('no history still starts at oldest chapter', () => {
  assert.equal(choose(undefined), oldest);
  assert.equal(choose({}), oldest);
});

test('partially read history still chooses oldest unread chapter', () => {
  assert.equal(choose({ v1c2en: null }), middle);
  assert.equal(choose({ v1c2en: null, v1c10: null }), newest);
  assert.equal(choose({ v2c1: null }), oldest);
});

test('alternate edition completion and native volume/chapter ordering are preserved', () => {
  assert.equal(choose({ v1c2ja: null }), middle);
  assert.equal(choose({ v1c2ja: null, v1c10: null, v2c1: null }), newest);
  assert.deepEqual(aggregated.chapters, [newest, middle, oldest]);
  assert.equal(select({ chapters: [oldest] }, { v1c2en: null }, silent), oldest);
});

test('only the all-read fallback changes and existing authority fences remain', () => {
  const manifest = frontendOverlayManifest.filter(row => row.label === 'all-read Continue Reading stays at newest completed aggregate chapter');
  assert.equal(manifest.length, 1);
  assert.equal(manifest[0].count, 1);
  assert.equal(manifest[0].path, 'src/routes/(app)/title/MangaActions.svelte');
  assert.match(transformed, /targetAggregatedChapter = filtered\[filtered\.length - 1\]/);
  assert.match(transformed, /else targetAggregatedChapter = aggregated\.chapters\[aggregated\.chapters\.length - 1\]/);
  assert.match(transformed, /readDisposed/);
  assert.match(transformed, /generation !== readGeneration/);
  assert.match(transformed, /context !== readContext\(\)/);
  assert.equal(fs.readFileSync(target, 'utf8'), original, 'Preserved frontend source must remain unchanged');
});
