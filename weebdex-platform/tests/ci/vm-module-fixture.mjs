// Test-only VM loading. Link callbacks instantiate dependencies; Node links the
// complete graph, including diamonds, rather than competing recursive link calls.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';

export function createVmModuleLoader({createModule, resolve}) {
  const modules = new Map(), links = new Map(), evaluations = new Map();
  let linkQueue = Promise.resolve();
  const dynamicImport = (specifier, importer) => evaluate(resolve(specifier, importer));
  function instantiate(name) {
    if (!modules.has(name)) modules.set(name, createModule(name, dynamicImport));
    return modules.get(name);
  }
  function load(name) {
    if (!links.has(name)) {
      // Different requested roots may share still-linking dependencies. Serialize
      // only root linking, not evaluation (which can itself await a dynamic import).
      const pending = linkQueue.then(async () => {
        const module = instantiate(name);
        if (module.status === 'unlinked') {
          await module.link((specifier, importer) => instantiate(resolve(specifier, importer)));
        }
        if (module.status === 'errored') throw module.error;
        return module;
      });
      links.set(name, pending);
      // A failed root remains rejected in links without poisoning unrelated roots.
      linkQueue = pending.then(() => undefined, () => undefined);
    }
    return links.get(name);
  }
  function evaluate(name) {
    if (!evaluations.has(name)) evaluations.set(name, load(name).then(async module => {
      // Even an 'evaluated' module can have an outstanding top-level await.
      // Await the VM's evaluation promise, rather than treating status as completion.
      await module.evaluate();
      return module;
    }));
    return evaluations.get(name);
  }
  async function namespace(name) { return (await evaluate(name)).namespace; }
  return {load, evaluate, namespace};
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const directory = path.join(root, 'platform-integrations/public-api');
const localName = /^[a-z][a-z0-9-]*$/;
export function createPublicApiModuleLoader({context, boundAccountAccess}) {
  return createVmModuleLoader({
    resolve(specifier) {
      if (specifier === '$lib/constants/Error') return 'error';
      if (specifier.startsWith('./') && localName.test(specifier.slice(2))) return specifier.slice(2);
      throw Error('Unexpected public API fixture import: ' + specifier);
    },
    createModule(name, importModuleDynamically) {
      if (!localName.test(name)) throw Error('Unexpected public API fixture module: ' + name);
      if (name === 'bound-account-request' && boundAccountAccess) {
        return new vm.SyntheticModule(['boundAccountAccess'], function () {
          this.setExport('boundAccountAccess', boundAccountAccess);
        }, {context, identifier: 'fixture:bound-account-request'});
      }
      const file = name === 'error' ? path.join(root, 'frontend/src/lib/constants/Error.ts') : path.join(directory, name + '.ts');
      return new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), {mode: 'transform'}), {
        context, identifier: file, importModuleDynamically
      });
    }
  });
}
