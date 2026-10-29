// Only the public search parser's mode-selection dependency is replaced here.
// The actual parser, public validators and query/client code remain unchanged.
// This seam is not an account store, identity, persistence or HTTP simulator.
import assert from 'node:assert/strict';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const parser = path.join(root, 'platform-integrations/public-api/manga-search-query.ts');
const storeId = 'fixture:public-search-mode-store', libraryId = 'fixture:public-search-mode-library';
export function createPublicSearchModeFixture(context) {
  let mode = 'device';
  const localLibraryState = Object.freeze({ subscribe(callback) { callback(Object.freeze({ mode })); return () => {}; } });
  const get = store => { assert.equal(store, localLibraryState, 'Only the parser mode store may be read'); let value; const off = store.subscribe(next => { value = next; }); off(); return value; };
  return {
    setMode(value) { assert.ok(value === 'device' || value === 'account', 'Finite library mode'); mode = value; },
    resolve(specifier, importer) {
      if (importer.identifier !== parser) return undefined;
      if (specifier === 'svelte/store') return storeId;
      if (specifier === '../local-library/library') return libraryId;
      return undefined;
    },
    module(identifier) {
      if (identifier === storeId) return new vm.SyntheticModule(['get'], function () { this.setExport('get', get); }, { context, identifier });
      if (identifier === libraryId) return new vm.SyntheticModule(['localLibraryState'], function () { this.setExport('localLibraryState', localLibraryState); }, { context, identifier });
      return undefined;
    }
  };
}
