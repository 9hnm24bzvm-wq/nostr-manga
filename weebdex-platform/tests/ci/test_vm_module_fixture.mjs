import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import {createVmModuleLoader, createPublicApiModuleLoader} from './vm-module-fixture.mjs';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),directory=path.join(root,'platform-integrations/public-api');
const makeContext=()=>vm.createContext({URL,URLSearchParams,TextEncoder,AbortController,console});

function sourceLoader(sources, context) {
  const created = new Map();
  const loader = createVmModuleLoader({
    resolve(specifier) {
      assert.match(specifier, /^\.\/[a-z-]+$/, 'Unexpected control import');
      return specifier.slice(2);
    },
    createModule(name, importModuleDynamically) {
      assert.ok(Object.hasOwn(sources, name), 'Unexpected control module: ' + name);
      created.set(name, (created.get(name) ?? 0) + 1);
      return new vm.SourceTextModule(sources[name], {context, identifier: name, importModuleDynamically});
    }
  });
  return {...loader, created};
}

test('cached link/evaluation promises handle concurrent diamond roots and await actual completion', {timeout: 5000}, async () => {
  let release, began;
  const ready = new Promise(resolve => { release = resolve; });
  const started = new Promise(resolve => { began = resolve; });
  const counts = {shared: 0, left: 0, right: 0, root: 0};
  const context = vm.createContext({ready, began, counts});
  const loader = sourceLoader({
    shared: 'counts.shared++; began(); await ready; export const identity = {};',
    left: "import {identity} from './shared'; counts.left++; export {identity};",
    right: "import {identity} from './shared'; counts.right++; export {identity};",
    root: "import {identity as left} from './left'; import {identity as right} from './right'; counts.root++; export {left, right};"
  }, context);
  const linking = loader.load('root');
  assert.equal(loader.load('root'), linking);
  const pending = loader.evaluate('root');
  assert.equal(loader.evaluate('root'), pending);
  const concurrent = [pending, loader.evaluate('left'), loader.evaluate('right'), loader.evaluate('shared')];
  let settled = 0;
  const all = Promise.all(concurrent.map(value => value.then(module => { settled++; return module; })));
  await started;
  assert.equal(settled, 0, 'top-level await must not expose a premature namespace');
  release();
  const [rootModule, left, right, shared] = await all;
  assert.equal(await linking, rootModule);
  assert.equal(rootModule.namespace.left, shared.namespace.identity);
  assert.equal(rootModule.namespace.right, shared.namespace.identity);
  assert.equal(left.namespace.identity, right.namespace.identity);
  assert.equal(await loader.evaluate('root'), rootModule);
  assert.equal(await loader.namespace('shared'), shared.namespace);
  assert.deepEqual(counts, {shared: 1, left: 1, right: 1, root: 1});
  assert.deepEqual([...loader.created.values()], [1, 1, 1, 1]);
});

test('dynamic imports share evaluations without deadlocking a root top-level await', {timeout: 5000}, async () => {
  const counts = {leaf: 0};
  const loader = sourceLoader({
    root: "export const first = await import('./leaf'); export const second = await import('./leaf');",
    leaf: 'counts.leaf++; export const identity = {};'
  }, vm.createContext({counts}));
  const rootModule = await loader.evaluate('root');
  const leaf = await loader.evaluate('leaf');
  assert.equal(rootModule.namespace.first, leaf.namespace);
  assert.equal(rootModule.namespace.second, leaf.namespace);
  assert.equal(counts.leaf, 1);
  assert.deepEqual([...loader.created.values()], [1, 1]);
});

test('failed links and evaluations stay rejected while unrelated roots remain usable', async () => {
  const failure = Error('Finite evaluation failure');
  const loader = sourceLoader({
    broken: "import 'unexpected-dependency';",
    throwing: 'throw failure;',
    good: 'export const value = 42;'
  }, vm.createContext({failure}));
  const broken = loader.load('broken');
  assert.equal(loader.load('broken'), broken);
  await assert.rejects(broken, /Unexpected control import/);
  await assert.rejects(loader.evaluate('broken'), /Unexpected control import/);
  const throwing = loader.evaluate('throwing');
  assert.equal(loader.evaluate('throwing'), throwing);
  await assert.rejects(throwing, error => error === failure);
  await assert.rejects(loader.evaluate('throwing'), error => error === failure);
  assert.equal((await loader.namespace('good')).value, 42);
  assert.deepEqual([...loader.created.values()], [1, 1, 1]);
});

test('actual public modules share the language/query diamond across repeated concurrent roots', async () => {
  const denied = Error('Finite native-session directory boundary');
  let access = 0;
  const selected = new URLSearchParams({ name: 'Reader', limit: '20' }), controller = new AbortController();
  const loader = createPublicApiModuleLoader({context: makeContext(), boundAccountAccess() { assert.fail('Ordinary directory must not use a signing account'); }, sessionUserDirectory(query, signal) { access++; assert.equal(query, selected); assert.equal(signal, controller.signal); throw denied; }});
  const pending = loader.evaluate('site-client');
  assert.equal(loader.evaluate('site-client'), pending);
  const [site, query, language, popularity, directoryModule, repeated] = await Promise.all([
    pending, loader.evaluate('query'), loader.evaluate('language'), loader.evaluate('popularity'),
    loader.evaluate('user-directory'), loader.evaluate('site-client')
  ]);
  assert.equal(site, repeated);
  assert.equal((await loader.load('language')), language);
  assert.equal(await loader.namespace('query'), query.namespace);
  assert.equal(typeof directoryModule.namespace.createUserDirectorySearch, 'function');
  assert.equal(language.namespace.publicWriteLanguage('ta'), 'ta');
  assert.equal(language.namespace.publicSearchLanguage('TA'), true);
  assert.throws(() => language.namespace.publicWriteLanguage('TA'), error => error.status === 422);
  assert.equal(query.namespace.publicQuery('manga', new URLSearchParams({lang: 'ta'})).get('originalLanguage[]'), 'ta');
  assert.equal(popularity.namespace.popularityQuery('manga', new URLSearchParams({lang: 'ta'})).query.get('originalLanguage[]'), 'ta');
  assert.throws(() => popularity.namespace.popularityQuery('manga', new URLSearchParams({lang: 'ta!'})), error => error.status === 422);
  const bridge = new site.namespace.RetainedCatalogueClient(new Proxy({}, {get() { throw Error('Unexpected anonymous HTTP'); }}));
  await assert.rejects(() => bridge.read('user.search', { query: selected, signal: controller.signal }), error => error === denied);
  assert.equal(access, 1, 'the actual dynamic directory import reaches only the explicit finite native-session seam');
});

test('public fixture refuses unknown imports, path escapes and unstubbed account dependencies', async () => {
  const loader = createPublicApiModuleLoader({context: makeContext()});
  for (const name of ['node:fs', '../local-library/publication', '/tmp/fixture', './language']) {
    await assert.rejects(loader.load(name), /Unexpected public API fixture module/);
  }
  await assert.rejects(loader.load('missing-fixture-module'), error => error.code === 'ENOENT');
  await assert.rejects(loader.evaluate('bound-account-request'), /Unexpected public API fixture import: \.\.\/local-library\/publication/);
  await assert.rejects(loader.evaluate('session-user-directory'), /Unexpected public API fixture import: svelte\/store/);
  assert.equal((await loader.namespace('language')).publicWriteLanguage('ta'), 'ta');
});

// Exact pre-correction loader bodies from 312dee79. Only their enclosing scope
// is fresh, so a failure proves the old loading algorithm against current real
// sources, not a hand-written replacement graph or a forced error.
function legacyPopularityLoader() {
  const context = makeContext(), modules = new Map();
  async function load(name){if(modules.has(name))return modules.get(name);const file=name==='error'?path.join(root,'frontend/src/lib/constants/Error.ts'):path.join(directory,name+'.ts'),module=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform'}),{context,identifier:file});modules.set(name,module);await module.link((specifier,importer)=>load(specifier==='$lib/constants/Error'?'error':path.relative(directory,path.resolve(path.dirname(importer.identifier),specifier)).replace(/\.ts$/,'')));return module;}
  async function namespace(name){const module=await load(name);if(module.status!=='evaluated')await module.evaluate();return module.namespace;}
  return namespace;
}
function legacyDirectoryLoader() {
  const context = makeContext(), cache = new Map();
  const defaultAccess = () => { throw Error('Unexpected real account access'); };
  async function load(name){if(cache.has(name))return cache.get(name);let module;if(name==='$lib/constants/Error'){module=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(path.join(root,'frontend/src/lib/constants/Error.ts'),'utf8'),{mode:'transform'}),{context});}else if(name==='bound-account-request'){module=new vm.SyntheticModule(['boundAccountAccess'],function(){this.setExport('boundAccountAccess',(...args)=>defaultAccess(...args));},{context});}else{module=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(path.join(directory,name+'.ts'),'utf8'),{mode:'transform'}),{context,importModuleDynamically:async specifier=>{const value=await load(specifier.slice(2));if(value.status!=='evaluated')await value.evaluate();return value;}});}cache.set(name,module);await module.link(specifier=>load(specifier.startsWith('./')?specifier.slice(2):specifier));return module;}
  return load;
}
const oldFailure = error => error.code === 'ERR_VM_MODULE_LINK_FAILURE' && /\.\/language/.test(error.message);
test('exact old popularity loader fails at the actual shared language edge', async () => {
  const namespace = legacyPopularityLoader();
  await namespace('client');
  await assert.rejects(namespace('site-client'), oldFailure);
});
test('exact old directory loader fails at the actual shared language edge', async () => {
  const load = legacyDirectoryLoader();
  const directoryModule = await load('user-directory');
  await directoryModule.evaluate();
  await assert.rejects(load('site-client'), oldFailure);
});
