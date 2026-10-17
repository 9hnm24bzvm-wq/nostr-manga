#!/usr/bin/env node
// Source/strict-locator selector and assertion preservation seams. No browser,
// application, database, service, network or dependency installation is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import test from 'node:test';
import { searchRatingTrigger } from '../../scripts/browser-select-controls.mjs';
const root = new URL('../../', import.meta.url), read = value => fs.readFileSync(new URL(value, root), 'utf8');
const source = read('scripts/browser-journeys.mjs');
function pageFor(texts) {
  const events = [], form = {}, placeholder = {}, scope = {};
  const page = {
    getByPlaceholder(name, options) { assert.equal(name,'Search...'); assert.equal(options.exact,true); return placeholder; },
    locator(selector) { assert.equal(selector,'form'); return { filter({has}) { assert.equal(has,placeholder); events.push('unique-search-form'); return scope; } }; }
  };
  scope.locator = selector => { assert.equal(selector,'button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible'); return { filter({hasText}) { const rows = texts.filter(text => hasText.test(text)); return { async click() { assert.equal(rows.length,1,'strict unique rating trigger'); events.push(rows[0]); } }; } }; };
  return {page,events};
}
test('rating trigger follows real Select label/count topology and rejects ambiguous or wrong controls', async () => {
  const markup = read('frontend/src/lib/components/MangaSearchFilter.svelte');
  assert.match(markup, /<form[\s\S]*?placeholder="Search\.\.\."/);
  assert.match(markup, /<Select.Root type="multiple" bind:value=\{search.contentRating\}>[\s\S]*?<Select.Trigger[\s\S]*?ContentRatingText\[k\][\s\S]*?\+\{search.contentRating.length\}[\s\S]*?Content Rating/);
  for (const selected of ['Content Rating',' Safe +1 ','Safe, Suggestive, Erotica, Pornographic +4','\nSafe,\nSuggestive\n+2\n']) {
    const value = pageFor([selected,'Publication Status','Language','Sort: Last uploaded chapter']); await searchRatingTrigger(value.page).click(); assert.equal(value.events.at(-1), selected);
  }
  for (const texts of [[],['Safe'],['Content Rating extra'],['Content Rating','Content Rating']]) await assert.rejects(searchRatingTrigger(pageFor(texts).page).click(), /strict unique/);
  assert.doesNotMatch(source, /getByRole\('combobox'/);
  assert.match(source, /setListboxOptions\(page, searchRatingTrigger\(page\), \[\]\)/);
  assert.match(source, /setListboxOptions\(page, searchRatingTrigger\(page\), \['Safe'\]\)/);
});
test('upload language scope and popup use exact visible text instead of duplicated flag accessible name', () => {
  const parent = read('frontend/src/lib/components/upload/UploadChapter.svelte'), metadata = read('frontend/src/lib/components/upload/UploadChapterMetadata.svelte');
  assert.match(parent, /<form[\s\S]*?<UploadChapterMetadata[\s\S]*?id="terms"[\s\S]*?<\/form>/);
  assert.match(metadata, /<Combobox.Root type="single" bind:value=\{draft.language\}/);
  assert.match(metadata, /<span class="truncate">Select Language<\/span>/);
  assert.match(metadata, /<Combobox.Item value=\{key\}>[\s\S]*?alt=\{value\}[\s\S]*?\{value\}/);
  assert.match(source, /selectListboxTrigger\(page.locator\('form'\).filter\(\{ has: page.locator\("#terms"\) \}\)\).filter\(\{ hasText: \/\^\\s\*Select Language\\s\*\$\/ \}\).click\(\)\);[\s\S]*?clickListboxOption\(page, "English"\)/);
  assert.doesNotMatch(source, /getByText\("English"[^\n]*\.last\(\)/);
});
test('all98 pre-relocation behavioral assertions survive with exact title201 and strengthened native import contracts', () => {
  assert(process.env.WEEBDEX_FRONTEND_ROOT, 'Use the verified exact-lock dependency root');
  const require = createRequire(path.join(process.env.WEEBDEX_FRONTEND_ROOT, 'package.json')), ts = require('typescript');
  const ast = ts.createSourceFile('browser-journeys.mjs',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS), predicates=[];
  const welcome = 'Account welcome appeared before account creation';
  const additions = ['The fixed upload diagnostic recorder is required','Actual uploaded staging prerequisite failed; replacement was not attempted','Actual image upload/replacement prerequisites failed; publication was not attempted','The fixed editorial diagnostic recorder is required','Original PNG cover bytes and authoritative identity were not persisted', 'Manage library does not point to its native Settings tab', 'Library tool dialog must have exactly one owner', welcome];
  // The later signup-only welcome adds this exact guard (c0c71ca). Preserve the historical
  // 98-assertion fingerprint and bind the added guard independently, rather than
  // blessing a changed fingerprint or increasing its expected assertion count.
  const welcomeAssertions = [];
  function findWelcome(node) { if (ts.isCallExpression(node) && node.expression.getText(ast) === 'insist' && node.arguments[1]?.text === welcome) welcomeAssertions.push(node.arguments[0].getText(ast)); ts.forEachChild(node, findWelcome); }
  findWelcome(ast);
  assert.deepEqual(welcomeAssertions, ['await page.getByRole("heading", { name: "Your account is ready", exact: true }).count() === 0']);
  // This historical fingerprint predates the separately tested relocation step.
  // Its native-tab geometry/permission/dialog assertions have their own actual-
  // callback negative drivers; do not mistake those additions for a removed gate.
  const relocationStart = source.indexOf("\t\tawait step('relocated native Library"), relocationEnd = source.indexOf('\n\t\tconst check =', relocationStart);
  assert(relocationStart > 0 && relocationEnd > relocationStart);
  function walk(node) { if (ts.isCallExpression(node) && node.expression.getText(ast) === 'insist') { if (!(node.getStart(ast) >= relocationStart && node.getEnd() <= relocationEnd) && !additions.includes(node.arguments[1]?.text)) {
    const parts=node.arguments.map(arg => arg.getText(ast).replace(/\s+/g,' '));
    if(node.arguments[1]?.text==='Title creation did not persist') { assert.equal(parts[0],'response.status() === 201'); parts[0]='response.status() === 200'; }
    if(node.arguments[1]?.text==='Foreign native IDs could be merged or replaced silently') {
      assert.equal(parts[0], "await review.getByRole('button', { name: 'Merge reviewed import', exact: true }).count() === 0 && await review.getByRole('button', { name: 'Replace with reviewed import', exact: true }).count() === 0");
      parts[0] = "await target.getByRole('button', { name: 'Merge reviewed import', exact: true }).count() === 0";
      parts[1] = "'Foreign native IDs could be merged silently'";
    }
    predicates.push(parts.join(','));
  } } ts.forEachChild(node,walk); } walk(ast);
  assert.equal(predicates.length,98);
  assert.equal(createHash('sha256').update(JSON.stringify(predicates.sort())).digest('hex'),'18f03e9f77e369b0d7a85d8f1ec6cfaba88cb217ef1b27986d253653687e9d9a');
});
