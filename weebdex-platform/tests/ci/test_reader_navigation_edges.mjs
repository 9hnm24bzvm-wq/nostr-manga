#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const plugin = createFrontendOverlays(frontend);
const transformed = name => { const file = path.join(frontend, 'src/lib/components/reader', name); return plugin.transform.handler(fs.readFileSync(file, 'utf8'), file).code; };
const provider = transformed('reader-provider.svelte');
const selection = provider.match(/const requestedPage = ([\s\S]*?)reader\.currentPage = reader\.pages\[([^\r\n]+)\];/);
assert(selection, 'Actual production bounded page initialization must be present');
const choose = new Function('reader', 'page', ('const requestedPage = ' + selection[1] + 'reader.currentPage = reader.pages[' + selection[2] + '];').replace('(page.state as any)', 'page.state'));
const pages = [0, 1, 2].map(index => ({ index }));
const resolve = (raw, state) => { const reader = { pages }; choose(reader, { state: state === undefined ? {} : { page: state }, params: { page: raw } }); return reader.currentPage?.index; };

test('malformed and non-finite page URLs show a real first page', () => {
  for (const raw of ['abc', 'NaN', 'Infinity', '-Infinity', '', undefined]) assert.equal(resolve(raw), 0, String(raw));
});
test('valid page, clamp bounds, decimal coercion and history-state priority remain coherent', () => {
  assert.equal(resolve('2'), 1);
  assert.equal(resolve('3'), 2);
  assert.equal(resolve('0'), 0);
  assert.equal(resolve('-1'), 0);
  assert.equal(resolve('999'), 2);
  assert.equal(resolve('1.5'), 0);
  assert.equal(resolve('2.9'), 1);
  assert.equal(resolve('1', 3), 2);
  assert.equal(resolve('3', 'NaN'), 0);
});
const context = transformed('context.svelte.ts');
const method = context.slice(context.indexOf('async changePage(idx: number) {'), context.indexOf('\tasync gotoPreviousPage()'));
assert(method.startsWith('async changePage'));
const AsyncFunction = Object.getPrototypeOf(async function() {}).constructor;
const changePage = new AsyncFunction('idx', method.slice(method.indexOf('{') + 1, method.lastIndexOf('}')));
const state = (end = true) => { const value = { pages, currentPage: pages[2], isViewingLastPage: end, scrolls: 0, mutex: { exec: operation => operation() }, async scrollIntoView(page) { assert(this.pages.includes(page)); this.scrolls++; } }; return value; };
test('Last page control leaves end card even when selected raw index is unchanged', async () => {
  const reader = state(); await changePage.call(reader, 2);
  assert.equal(reader.currentPage, pages[2]); assert.equal(reader.isViewingLastPage, false); assert.equal(reader.scrolls, 1);
});
test('ordinary same-page no-op and invalid index guards are retained', async () => {
  const normal = state(false); await changePage.call(normal, 2); assert.equal(normal.scrolls, 0);
  for (const index of [-1, 3]) { const reader = state(); await changePage.call(reader, index); assert.equal(reader.isViewingLastPage, true); assert.equal(reader.currentPage, pages[2]); assert.equal(reader.scrolls, 0); }
  const changed = state(); await changePage.call(changed, 1); assert.equal(changed.currentPage, pages[1]); assert.equal(changed.isViewingLastPage, false);
});
