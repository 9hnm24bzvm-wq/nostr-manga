// Exercise the actual composed chapter chooser with finite reader state.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
const frontend = path.resolve(new URL('../../frontend', import.meta.url).pathname), file = path.join(frontend, 'src/lib/components/reader/context.svelte.ts');
const original = fs.readFileSync(file, 'utf8');
function choose(enabled) {
  let code = original;
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled })]) code = plugin.transform.handler(code, file)?.code ?? code;
  const method = code.slice(code.indexOf('findChapter(idx: number)'), code.indexOf('\n\tasync changeChapter'));
  return new Function('return ' + stripTypeScriptTypes('function ' + method, { mode: 'transform' }))();
}
function state(currentGroups, groups, entries = { unrelated: { language: 0, groups: [0] }, same: { language: 0, groups: [1] } }) {
  return { chapter: { id: 'current', relationships: { groups: currentGroups } }, aggregated: { chapters: [{ volume: '1', chapter: '2.5', entries }], languages: ['en', 'fr'], groups } };
}
for (const enabled of [false, true]) {
  const mode = enabled ? 'public' : 'native', findChapter = choose(enabled);
  test(mode + ' reader prefers the current group identity over an earlier edition with the same name', () => {
    const own = { id: 'group-own', name: 'Shared name' }, unrelated = { id: 'group-unrelated', name: 'Shared name' };
    const value = findChapter.call(state([own], [unrelated, own]), 0);
    assert.equal(value.id, 'same'); assert.equal(value.language, 'en'); assert.deepEqual(value.groups, ['Shared name']); assert.equal(value.chapter, '2.5');
  });
  test(mode + ' current group ID is stable when an edition carries a different group display name', () => {
    const value = findChapter.call(state([{ id: 'group-own', name: 'Previous name' }], [{ id: 'other', name: 'Previous name' }, { id: 'group-own', name: 'Renamed group' }]), 0);
    assert.equal(value.id, 'same'); assert.deepEqual(value.groups, ['Renamed group']);
  });
  test(mode + ' group-name fallback remains when either legacy metadata side has no ID', () => {
    for (const [current, candidate] of [[{ name: 'Current' }, { name: 'Current' }], [{ id: 'current', name: 'Current' }, { name: 'Current' }], [{ name: 'Current' }, { id: 'current', name: 'Current' }]]) {
      assert.equal(findChapter.call(state([current], [{ id: 'unrelated', name: 'Other' }, candidate]), 0).id, 'same');
    }
  });
  test(mode + ' no-group ties retain original edition ordering and actual language metadata', () => {
    const entries = { first: { language: 1, groups: [] }, second: { language: 0, groups: [] } };
    for (const current of [undefined, []]) { const value = findChapter.call(state(current, [], entries), 0); assert.equal(value.id, 'first'); assert.equal(value.language, 'fr'); assert.deepEqual(value.groups, []); }
  });
}
test('canonical reader context is unchanged', () => { assert.equal(fs.readFileSync(file, 'utf8'), original); });
