"""Actual plugin-emitted graph to actual Python admission, synthetic chunks only.

Rollup4.60.1 lists included dynamic import expressions, which can resolve to one
target more than once. No Rollup/Vite build, transport, browser or service runs.
The hosted failure's exact refused predicate conjunct remains unknown.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import traceback
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('qa_adjacency_admission', ROOT / 'scripts/ci/public-upload-qa-build.py')
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)

# This runs only the real plugin hooks, over explicit synthetic Rollup output.
# Its baseline reconstruction is accepted only if byte-identical to frozen6a3.
NODE_FIXTURE = r"""
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const source=process.argv[2], root=process.argv[3], baseline=process.argv[4]==='baseline', shape=process.argv[5];
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const paths=['scripts/public-upload-refusal-qa.mjs','scripts/vite.platform.config.mjs',
  'platform-integrations/public-api/nostr-upload-request.ts','platform-integrations/public-api/upload-client.ts'];
for(const name of paths){fs.mkdirSync(path.dirname(path.join(root,name)),{recursive:true});fs.copyFileSync(path.join(source,name),path.join(root,name));}
const producer=path.join(root,paths[0]);
if(baseline){
  const current=fs.readFileSync(producer,'utf8'), before='dynamic_imports: [...row.dynamicImports]', after='dynamic_imports: [...new Set(row.dynamicImports)]';
  assert.equal(current.split(after).length,2);
  const old=current.replace(after,before);
  assert.equal(sha(old),'b67b9611b7d6e75a334440d5a8963023eaabb8a49c8f2f7613dd2ead538c5b5a');
  fs.writeFileSync(producer,old);
}
const api=await import(pathToFileURL(producer));
const plugin=api.createUploadRefusalQaPlugin(root,{enabled:true,publicUploadEnabled:true}), requested=[];
plugin.configResolved({build:{ssr:false}});
plugin.buildStart.call({emitFile(row){requested.push(row);}});
assert.equal(requested.length,1);
const transport=path.join(root,paths[2]), client=path.join(root,paths[3]);
plugin.transform(fs.readFileSync(transport,'utf8'),transport);
const target='_app/immutable/chunks/transport.js', app='_app/immutable/chunks/client.js';
const entry='_app/immutable/entry/app.js', neighbour='_app/immutable/chunks/neighbour.js';
function chunk(fileName,modules,imports=[],extra={}){return {type:'chunk',fileName,
  modules:Object.fromEntries(modules.map(id=>[id,{}])),imports,dynamicImports:[],exports:[],
  facadeModuleId:null,isEntry:false,code:'// finite synthetic chunk '+fileName+'\n',...extra};}
const raw=shape==='single'?[app]:shape==='ordered'?[app,neighbour,app]:
  Array(shape==='raw129'?129:shape==='raw128'?128:2).fill(app);
const bundle={
  [target]:chunk(target,[transport]),[app]:chunk(app,[client],[target]),
  [entry]:chunk(entry,['finite-entry'],[],{isEntry:true,dynamicImports:raw}),
  [neighbour]:chunk(neighbour,['finite-neighbour']),
  [api.uploadRefusalReaderFile]:chunk(api.uploadRefusalReaderFile,[],[target],
    {isEntry:true,exports:['readUploadRequestRefusals'],facadeModuleId:'\0virtual:public-upload-refusal-reader'}),
};
const emitted=[];
try{plugin.generateBundle.handler.call({emitFile(row){emitted.push(row);}}, {},bundle);}
catch(error){assert.equal(error.message,'QA chunk imports exceed their finite bound');
  console.log(JSON.stringify({refused:true,raw_count:raw.length}));process.exit(0);}
assert.deepEqual(bundle[entry].dynamicImports,raw);
const ui=path.join(root,'artifacts/frontend');
for(const row of Object.values(bundle)){const name=path.join(ui,row.fileName);fs.mkdirSync(path.dirname(name),{recursive:true});fs.writeFileSync(name,row.code);}
assert.equal(emitted.length,1);assert.equal(emitted[0].fileName,api.uploadRefusalGraphFile);
const graph=JSON.parse(emitted[0].source), graphPath=path.join(ui,emitted[0].fileName);
fs.mkdirSync(path.dirname(graphPath),{recursive:true});fs.writeFileSync(graphPath,emitted[0].source);
console.log(JSON.stringify({refused:false,raw_dynamic_imports:raw,
  emitted_dynamic_imports:graph.chunks.find(row=>row.file_name===entry).dynamic_imports,
  producer_sha256:sha(fs.readFileSync(producer)),graph_sha256:sha(emitted[0].source)}));
"""


class PublicUploadQaAdjacency(unittest.TestCase):
    def emit(self, root, shape='repeat', baseline=False):
        result = subprocess.run(['node', '--input-type=module', '-e', NODE_FIXTURE,
                                 'qa-adjacency-fixture', str(ROOT), str(root),
                                 'baseline' if baseline else 'current', shape],
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_repeated_targets_emit_one_adjacency_and_actual_python_admission_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.emit(root)
            self.assertEqual(len(result['raw_dynamic_imports']), 2)
            self.assertEqual(result['emitted_dynamic_imports'], result['raw_dynamic_imports'][:1])
            self.assertEqual(qa.verify(root, root / 'artifacts/frontend', True)['graph_sha256'], result['graph_sha256'])

    def test_exact_6a3_counterfactual_emits_duplicates_and_names_rejects_them(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.emit(root, baseline=True)
            self.assertEqual(result['producer_sha256'], 'b67b9611b7d6e75a334440d5a8963023eaabb8a49c8f2f7613dd2ead538c5b5a')
            self.assertEqual(result['emitted_dynamic_imports'], result['raw_dynamic_imports'])
            try:
                qa.verify(root, root / 'artifacts/frontend', True)
            except RuntimeError as error:
                # Original admission delegates to insist(); the independently
                # prepared fixed-code diagnostic raises directly from names().
                self.assertIn([row.name for row in traceback.extract_tb(error.__traceback__)][-2:],
                              [['names', 'insist'], ['verify', 'names']])
            else:
                self.fail('Exact baseline duplicate adjacency unexpectedly passed strict admission')

    def test_unique_input_remains_admitted_by_both_exact_producers(self):
        for baseline in [False, True]:
            with self.subTest(baseline=baseline), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                result = self.emit(root, shape='single', baseline=baseline)
                self.assertEqual(result['emitted_dynamic_imports'], result['raw_dynamic_imports'])
                qa.verify(root, root / 'artifacts/frontend', True)

    def test_first_seen_order_and_raw_source_vector_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.emit(root, shape='ordered')
            self.assertEqual(result['raw_dynamic_imports'][0], result['raw_dynamic_imports'][2])
            self.assertEqual(result['emitted_dynamic_imports'], result['raw_dynamic_imports'][:2])
            qa.verify(root, root / 'artifacts/frontend', True)

    def test_raw_128_bound_remains_before_canonicalization_and_129_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.emit(root, shape='raw128')
            self.assertEqual(len(result['raw_dynamic_imports']), 128)
            self.assertEqual(len(result['emitted_dynamic_imports']), 1)
            qa.verify(root, root / 'artifacts/frontend', True)
        for baseline in [False, True]:
            with self.subTest(baseline=baseline), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.assertEqual(self.emit(root, shape='raw129', baseline=baseline), {'refused': True, 'raw_count': 129})
                self.assertFalse((root / 'artifacts/frontend' / qa.GRAPH).exists())

    def test_python_uniqueness_and_actual_chunk_hash_guards_are_unchanged(self):
        for kind in ['duplicate', 'changed-chunk']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.emit(root)
                ui = root / 'artifacts/frontend'
                graph = json.loads((ui / qa.GRAPH).read_bytes())
                entry = next(row for row in graph['chunks'] if row['file_name'] == '_app/immutable/entry/app.js')
                if kind == 'duplicate':
                    entry['dynamic_imports'] *= 2
                    (ui / qa.GRAPH).write_text(json.dumps(graph))
                else:
                    path = ui / entry['dynamic_imports'][0]
                    path.write_bytes(path.read_bytes() + b'changed bytes')
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)


if __name__ == '__main__':
    unittest.main()
