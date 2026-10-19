// Execute the composed reader script and actual navigation/read ownership helpers.
// Tick and Svelte lifecycle scheduling are finite seams; no browser is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';

const root = path.resolve(new URL('../../', import.meta.url).pathname);
const frontend = path.join(root, 'frontend'), relative = 'src/lib/components/reader/reader-pages.svelte';
const file = path.join(frontend, relative), original = fs.readFileSync(file, 'utf8');
const executable = source => stripTypeScriptTypes(source.replace(/^\s*import[^;]+;\s*$/gm, '').replace(/\bexport /g, ''), { mode: 'transform' });
function composed(enabled) {
  let code = original;
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled })]) code = plugin.transform.handler(code, file)?.code ?? code;
  return code;
}
function fixture(enabled, url = 'https://fixture.invalid/chapter/current/1?tlang=en&filter=a%2Fb&filter=two#image-2') {
  const ticks = [], mounts = [], destroys = [], replacements = [];
  const selected = { index: 0 }, reader = { chapter: { id: 'current' }, currentPage: selected, prefs: {} };
  const environment = { URL, URLSearchParams, window: { location: new URL(url) }, page: { state: { retained: 'state', page: 1 } },
    $state: value => value, useReader: () => reader, onMount: callback => mounts.push(callback), onDestroy: callback => destroys.push(callback),
    tick: () => new Promise(resolve => ticks.push(resolve)), $effect: callback => { environment.effect = callback; },
    replaceState: (path, state) => { replacements.push({ path, state }); environment.window.location = new URL(path, environment.window.location); environment.page.state = state; }
  };
  const context = vm.createContext(environment);
  const navigation = fs.readFileSync(path.join(root, 'platform-integrations/frontend-behavior/navigation-authority.ts'), 'utf8');
  const owner = fs.readFileSync(path.join(root, 'platform-integrations/frontend-behavior/safe-read-owner.ts'), 'utf8');
  const script = composed(enabled).match(/<script[^>]*>([\s\S]*?)<\/script>/)[1];
  vm.runInContext(executable(navigation + '\n' + owner + '\n' + script) + '\nthis.invalidateNavigation = invalidateNavigationEpoch;', context);
  async function settle() { const pending = ticks.splice(0); for (const resolve of pending) resolve(); await Promise.resolve(); }
  async function mount() { for (const callback of mounts) callback(); await settle(); }
  return { reader, environment, replacements, mount, settle, effect: () => environment.effect(), destroy: () => destroys.forEach(callback => callback()) };
}
for (const enabled of [false, true]) {
  const mode = enabled ? 'public' : 'native';
  test(mode + ' page history preserves encoded query, repeated filters, hash and existing state', async () => {
    const f = fixture(enabled); await f.mount(); f.reader.currentPage = { index: 1 }; f.effect(); await f.settle();
    assert.deepEqual(f.replacements.map(row => row.path), ['/chapter/current/2?tlang=en&filter=a%2Fb&filter=two#image-2']);
    assert.deepEqual(JSON.parse(JSON.stringify(f.replacements[0].state)), { retained: 'state', page: 2 });
  });
  test(mode + ' newer selected page owns matching path and state while old tick is dropped', async () => {
    const f = fixture(enabled); await f.mount(); f.effect(); f.reader.currentPage = { index: 2 }; f.effect(); await f.settle();
    assert.equal(f.replacements.length, 1); assert.match(f.replacements[0].path, /^\/chapter\/current\/3\?/); assert.equal(f.replacements[0].state.page, 3);
  });
  for (const reason of ['navigation starts', 'route commits', 'hash changes', 'chapter changes', 'page changes', 'destroyed']) test(mode + ' pending reader history yields when ' + reason, async () => {
    const f = fixture(enabled); await f.mount(); f.effect();
    if (reason === 'navigation starts') f.environment.invalidateNavigation();
    if (reason === 'route commits') f.environment.window.location = new URL('https://fixture.invalid/title/other');
    if (reason === 'hash changes') f.environment.window.location.hash = '#new';
    if (reason === 'chapter changes') f.reader.chapter.id = 'other';
    if (reason === 'page changes') f.reader.currentPage = { index: 2 };
    if (reason === 'destroyed') f.destroy();
    await f.settle(); assert.deepEqual(f.replacements, []);
  });
  test(mode + ' a destroyed mount tick cannot admit later reader history', async () => {
    const f = fixture(enabled), mounting = f.mount(); f.destroy(); await mounting; f.effect(); await f.settle(); assert.deepEqual(f.replacements, []);
  });
  test(mode + ' composition preserves every canonical reader byte and all markup', () => {
    const markup = source => source.replace(/<script[^>]*>[\s\S]*?<\/script>/g, '<script></script>');
    assert.equal(markup(composed(enabled)), markup(original)); assert.equal(fs.readFileSync(file, 'utf8'), original);
  });
}
