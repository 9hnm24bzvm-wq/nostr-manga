import {localPageDriverPath,localPageDriverAssociationSha256,readLocalPageDriverAssociation,restoreLocalPageDriverSource} from './reviewed-local-page-driver-source-successor.mjs';
import {restoreIntegratedSource} from './reviewed-integration-source-successors.mjs';
import {restoreHistorySource} from './reviewed-history-source-successors.mjs';
// Authored comparison/graph controls. No browser, service or product substitution.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {localPageAssociationSha256, readLocalPageSourceAssociation, restoreLocalPageSource, restoreReviewedLanguageSource} from './reviewed-local-page-source-successors.mjs';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const project = path.join(root, 'weebdex-platform');
const digest = value => createHash('sha256').update(value).digest('hex');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const metadata = fs.readFileSync(path.join(project, 'docs/fixtures/signed-local-page-source-associations.json'));
const data = readLocalPageSourceAssociation();
const encoded = value => Buffer.from(JSON.stringify(value, null, 2) + '\n');
function actual(row) {
  let value = read(row.path);
  if (row.kind === 'adapterFixturePrefix') {
    const start = value.indexOf('const context ='), end = value.indexOf('async function ready(');
    assert.ok(start > 0 && end > start);
    value = value.slice(start, end);
  }
  return value;
}
function exactFile(row) {
  const bytes = fs.readFileSync(path.join(root, row.path));
  assert.equal(bytes.length, row.bytes, row.path);
  assert.equal(digest(bytes), row.sha256, row.path);
  return bytes.toString('utf8');
}

// Current physical-source identities for two reviewed chapter-page provenance
// successors. Historical local-page metadata remains immutable and is checked
// independently; do not reconstruct historical bytes for current graph parsing.
const currentSourceSuccessors = Object.freeze([
  {
    "path": "weebdex-platform/platform-integrations/public-api/types.ts",
    "previousBytes": 4439,
    "previousSha256": "ddb5209dc67047998af0f1c8f4d7ac117b3857b48ce798bfe79c978e093555db",
    "currentBytes": 4485,
    "currentSha256": "b1d86a39ec64540c867a866ffb6f5ada371917de141f5097c6c19d0c840ec741"
  },
  {
    "path": "weebdex-platform/backend/src/public_api/catalogue.rs",
    "previousBytes": 51383,
    "previousSha256": "2980072e4fef4bc9145dfd445bcd06a62ca45a500590668ff9f4f5e56e2740ec",
    "currentBytes": 54715,
    "currentSha256": "415731320e6f5d9e19d6dd5219db9ad0a38c5f6889f10508e2d2eeb611dbdfab"
  }
].map(row => Object.freeze(row)));
function exactCurrentFile(row) {
  const successor = currentSourceSuccessors.find(value => value.path === row.path);
  if (!successor) return exactFile(row);
  assert.equal(row.bytes, successor.previousBytes, row.path + ' historical length');
  assert.equal(row.sha256, successor.previousSha256, row.path + ' historical digest');
  return exactFile({...row, bytes: successor.currentBytes, sha256: successor.currentSha256});
}

test('immutable local-page association declares three complete identities and all 21 exact seams', () => {
  assert.equal(digest(metadata), localPageAssociationSha256);
  assert.equal(data.baseline_revision, 'f7c598cb57f9a2afbecc107c1d81bb4b30ac4102');
  assert.equal(data.reviewed_product_revision, '7e931c74903f3969f26c2299a6d7cbfe91a3ad8c');
  assert.equal(data.reviewed_fixture_revision, '017d47e9af8c6097139b7f48866c5f85a9722b91');
  assert.equal(data.browser_executed, false); assert.equal(data.backend_executed, false);
  assert.match(data.scope, /never runtime\/build substitution/);
  assert.deepEqual(data.rows.map(row => [row.kind, row.literal_reversals.length]), [['uploadClient', 13], ['chapterEditor', 6], ['adapterFixturePrefix', 2]]);
  assert.equal(data.rows.reduce((sum, row) => sum + row.literal_reversals.length, 0), data.seam_count);
  assert.equal(data.seam_count, 21);
});
for (const row of data.rows) {
  test(row.kind + ': actual function verifies unique seams and complete predecessor bytes without modifying current source', () => {
    const source = actual(row);
    assert.equal(Buffer.byteLength(source), row.current_bytes); assert.equal(digest(source), row.current_sha256);
    let proof = source;
    for (const edit of row.literal_reversals) {
      assert.notEqual(edit.current, ''); assert.equal(source.split(edit.current).length, 2);
      assert.equal(proof.split(edit.current).length, 2);
      proof = proof.replace(edit.current, () => edit.previous);
    }
    const predecessor = restoreLocalPageSource(source, row.kind);
    assert.equal(predecessor, proof);
    assert.equal(Buffer.byteLength(predecessor), row.previous_bytes);
    assert.equal(digest(predecessor), row.previous_sha256);
    assert.equal(actual(row), source); assert.notEqual(predecessor, source);
  });
  test(row.kind + ': old, altered, missing, partial and duplicate current source are refused', () => {
    const source = actual(row), old = restoreLocalPageSource(source, row.kind);
    for (const value of [old, source + '\n', source.slice(1), '', undefined, null, 1, {}, Buffer.from(source)]) {
      assert.throws(() => restoreLocalPageSource(value, row.kind), /Unknown reviewed local page source/);
    }
    for (const edit of row.literal_reversals) {
      for (const value of [source.replace(edit.current, ''), source.replace(edit.current, () => edit.previous), source + edit.current]) {
        assert.throws(() => restoreLocalPageSource(value, row.kind), /Unknown reviewed local page source/);
      }
    }
  });
  test(row.kind + ': exact outer layer composes with unchanged historical language/scheduling helpers', () => {
    const historical = {uploadClient: 'c8d3e55319a2f93ae1f9b4e11875c2d4711a1d8d985e4dc7df5ab722f26dca8f', chapterEditor: 'b368c2419b3eb1babe92bc3c13d274265249cf7c2200f63fb43415467689d328', adapterFixturePrefix: 'ab243b74e652d5766e41f3e10af337ac16bbbba9df04c4af5246dfa6a05f48a0'};
    assert.equal(digest(restoreReviewedLanguageSource(actual(row), row.kind)), historical[row.kind]);
    assert.throws(() => restoreReviewedLanguageSource(restoreLocalPageSource(actual(row), row.kind), row.kind));
  });
}
test('unknown source kinds and any changed metadata encoding, field, row, seam or graph are refused', () => {
  for (const kind of ['unknown', '__proto__', 'constructor', '', undefined, null, 1]) assert.throws(() => restoreLocalPageSource(actual(data.rows[0]), kind));
  const candidates = [metadata.toString(), Buffer.concat([metadata, Buffer.from(' ')]), encoded(Object.fromEntries(Object.entries(data).reverse()))];
  for (const key of Object.keys(data)) for (const operation of ['delete', 'change']) {
    const value = structuredClone(data);
    if (operation === 'delete') delete value[key]; else value[key] = 'unknown';
    candidates.push(encoded(value));
  }
  for (let i = 0; i < data.rows.length; i++) {
    for (const key of Object.keys(data.rows[i])) { const value = structuredClone(data); delete value.rows[i][key]; candidates.push(encoded(value)); }
    for (let j = 0; j < data.rows[i].literal_reversals.length; j++) for (const key of ['current', 'previous']) {
      const value = structuredClone(data); value.rows[i].literal_reversals[j][key] += ' '; candidates.push(encoded(value));
    }
  }
  for (const mutate of [value => value.rows.reverse(), value => value.rows.push(value.rows[0]), value => value.rows.pop(), value => value.bounded_current_source_graph[0].imports.pop()]) {
    const value = structuredClone(data); mutate(value); candidates.push(encoded(value));
  }
  for (const value of candidates) {
    assert.throws(() => readLocalPageSourceAssociation(value));
    assert.throws(() => restoreLocalPageSource(actual(data.rows[0]), data.rows[0].kind, value));
  }
});
test('historical helper bytes, full metadata and 40-file historical pinset remain unchanged', () => {
  assert.equal(data.preserved_historical_artifacts.length, 4);
  for (const row of data.preserved_historical_artifacts) exactFile(row);
  const pins = JSON.parse(exactFile(data.preserved_historical_artifacts.find(row => row.path.endsWith('/source-pins.json'))));
  assert.equal(pins.sources.length, 40); assert.equal(pins.browser_executed, false); assert.equal(pins.hosted_executed, false);
  const editor = data.rows.find(row => row.kind === 'chapterEditor');
  assert.equal(pins.sources.find(row => row.path === editor.path).sha256, editor.previous_sha256);
});
test('bounded current graph uses actual parsed imports, not reconstructed comparison or mounted stubs', () => {
  const require = createRequire(path.join(process.env.WEEBDEX_FRONTEND_ROOT ?? path.join(project, 'frontend'), 'package.json'));
  const ts = require('typescript');
  assert.equal(data.bounded_current_source_graph.length, 9);
  for (const row of data.bounded_current_source_graph) {
    let source = exactCurrentFile(row);
    if (row.path.endsWith('.svelte')) {
      const scripts = [...source.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)];
      assert.equal(scripts.length, 1); source = scripts[0][1];
    }
    const tree = ts.createSourceFile(row.path, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    assert.deepEqual(tree.parseDiagnostics, []);
    const imports = tree.statements.filter(ts.isImportDeclaration).map(node => node.moduleSpecifier.text);
    assert.deepEqual(imports, row.imports, row.path);
  }
  const byName = name => data.bounded_current_source_graph.find(row => row.path.endsWith('/' + name));
  for (const [name, dependencies] of [
    ['PublicChapterEditor.svelte', ['./PublicChapterPagesEditor.svelte', './upload-client', './normalize', './chapter-access', './types']],
    ['PublicChapterPagesEditor.svelte', ['./chapter-pages-controller', './upload-client', './chapter-access', '$lib/Context', '$lib', '$app/navigation', 'svelte', '$lib/components/upload/UploadChapterPages.svelte']],
    ['chapter-pages-controller.ts', ['./upload-client']], ['chapter-access.ts', ['./types']],
    ['normalize.ts', ['./types', './validation']], ['test_public_chapter_removal_dom.mjs', ['./signed-chapter-route-fixture.mjs']],
  ]) for (const dependency of dependencies) assert.ok(byName(name).imports.includes(dependency), name + ' -> ' + dependency);
  assert.equal(data.native_page_boundary.specifier, '$lib/components/upload/UploadChapterPages.svelte');
  assert.match(data.native_page_boundary.qualification, /require separate strict\/browser qualification/);
});
test('source-bound local eligibility is distinct from immutable protocol originals and is not full write authority', () => {
  for (const row of data.backend_source_bindings) exactCurrentFile(row);
  const api = 'weebdex-platform/platform-integrations/public-api/';
  assert.match(read(api + 'chapter-access.ts'), /chapter\.attributes\.localPageSource==='local'/);
  assert.match(read(api + 'chapter-access.ts'), /canEditPages:pageSource==='local'&&has\('self.chapter.update'\)&&canUpdate/);
  assert.match(read(api + 'normalize.ts'), /public_api:\{resource:row\}/);
  assert.match(read(api + 'types.ts'), /attributes: A/);
  assert.match(read(api + 'PublicChapterPagesEditor.svelte'), /access\?\.pageSource === 'protocol'/);
  assert.match(read(api + 'PublicChapterPagesEditor.svelte'), /These pages belong to the original publication and cannot be edited or replaced/);
  assert.match(read(api + 'chapter-pages-controller.ts'), /await owner\.chapterAccess\(version\)/);
  assert.match(read('weebdex-platform/backend/src/public_api/catalogue.rs'), /catalog_public_chapter_json\(c\.id\) \|\| jsonb_build_object\('_local_page_source_mutable', c\.protocol_event_id IS NULL\)/);
  assert.match(read('weebdex-platform/backend/src/public_api/catalogue_dto.rs'), /value\["_local_page_source_mutable"\]\.as_bool\(\)/);
  assert.match(read('weebdex-platform/backend/src/public_api/catalogue_dto.rs'), /a\["localPageSource"\] = json!\(if local \{ "local" \} else \{ "protocol" \}\)/);
});


const driverMetadata = fs.readFileSync(path.join(project, 'docs/fixtures/signed-local-page-driver-source-association.json'));
const driverData = readLocalPageDriverAssociation();
const driverSource = () => fs.readFileSync(path.join(root, localPageDriverPath));

test('separate driver record shares the exact Python metadata without rewriting the old association', () => {
  assert.equal(localPageDriverAssociationSha256, '2764693600cde177426c8e948ba0dea2f7f7d97cc5f6ade7ab472078d9c87148');
  assert.equal(digest(driverMetadata), localPageDriverAssociationSha256);
  assert.equal(driverData.row.path, localPageDriverPath);
  assert.equal(driverData.row.kind, 'signedFeatureSourcePinDriver');
  assert.equal(driverData.comparison_only, true);
  assert.equal(driverData.source_commit, '9c221a556ed4dbe3fcbda889d67b3f403c5a37d7');
  assert.equal(driverData.previous_source_commit, 'f7c598cb57f9a2afbecc107c1d81bb4b30ac4102');
  assert.deepEqual(driverData.preserved_local_page_metadata, {
    path: 'weebdex-platform/docs/fixtures/signed-local-page-source-associations.json',
    bytes: 73175, sha256: 'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c',
  });
  assert.equal(digest(metadata), driverData.preserved_local_page_metadata.sha256);
  const old = data.consumer_baseline_artifacts.find(row => row.path === localPageDriverPath);
  assert.deepEqual([old.bytes, old.sha256], [driverData.row.previous_bytes, driverData.row.previous_sha256]);
  assert.ok(read('weebdex-platform/tests/ci/reviewed_local_page_source_successors.py').includes(
    "DRIVER_METADATA_SHA256 = '" + localPageDriverAssociationSha256 + "'"));
});

test('driver Buffer and string inputs reconstruct two unique literals and the unchanged integration/history chain', () => {
  const physical = driverSource(), row = driverData.row;
  assert.deepEqual([physical.length, digest(physical)], [2055, 'a2851efe16c88d1fd14ce48493e24aa415f8c8ac57a397779539822bbd3a3a85']);
  assert.equal(row.literal_reversals.length, 2);
  let proof = physical.toString('utf8');
  for (const edit of row.literal_reversals) {
    assert.notEqual(edit.current, ''); assert.equal(edit.previous, '');
    assert.equal(physical.toString('utf8').split(edit.current).length, 2);
    assert.equal(proof.split(edit.current).length, 2);
    proof = proof.replace(edit.current, () => edit.previous);
  }
  for (const source of [physical, physical.toString('utf8')]) {
    const prior = restoreLocalPageDriverSource(localPageDriverPath, source);
    assert.equal(Buffer.isBuffer(prior), Buffer.isBuffer(source));
    assert.equal(prior.toString(), proof);
    assert.deepEqual([Buffer.byteLength(prior), digest(prior)], [1834, '5e603d716a87caf9f143ee423738985aa4e51e5137600c6707857908fdfad263']);
    const integrated = restoreIntegratedSource(localPageDriverPath, prior);
    assert.deepEqual([Buffer.byteLength(integrated), digest(integrated)], [1617, '7e4179b1fab7dfd0a7915878747ef733b44a5788162ddad911df652de92f6476']);
    const historical = restoreHistorySource(localPageDriverPath, integrated);
    assert.deepEqual([Buffer.byteLength(historical), digest(historical)], [1432, '660ec6d7478eb2d50d368910aa6719868bebc6e4fcd74174aee453c0e27250fd']);
    const pins = JSON.parse(exactFile(data.preserved_historical_artifacts.find(row => row.path.endsWith('/source-pins.json'))));
    const pin = pins.sources.find(row => row.path === localPageDriverPath);
    assert.deepEqual([Buffer.byteLength(historical), digest(historical)], [pin.bytes, pin.sha256]);
  }
  assert.deepEqual(driverSource(), physical);
});

test('driver helper refuses unknown paths/types and old partial duplicate double or altered bytes', () => {
  const physical = driverSource(), text = physical.toString('utf8');
  const prior = restoreLocalPageDriverSource(localPageDriverPath, physical);
  const integrated = restoreIntegratedSource(localPageDriverPath, prior);
  const historical = restoreHistorySource(localPageDriverPath, integrated);
  for (const file of ['', '__proto__', 'constructor', undefined, null, 1, {}, [], new String(localPageDriverPath), localPageDriverPath + ' ', '../' + localPageDriverPath,
                      'weebdex-platform/platform-integrations/public-api/PublicChapterEditor.svelte']) {
    assert.throws(() => restoreLocalPageDriverSource(file, physical));
  }
  const sameLength = Buffer.from(physical); sameLength[0] ^= 1;
  const candidates = [null, undefined, 1, {}, [], new String(text), new Uint8Array(physical),
    prior, prior.toString(), integrated, historical, sameLength, physical.subarray(1),
    Buffer.concat([physical, Buffer.from(' ')]), Buffer.concat([physical, physical]), text + ' ', text.slice(1)];
  for (const edit of driverData.row.literal_reversals) {
    const partial = text.replace(edit.current, () => edit.previous);
    candidates.push(partial, Buffer.from(partial), text + edit.current,
      text.replace(edit.current, () => edit.current + edit.current));
  }
  for (const candidate of candidates) assert.throws(() => restoreLocalPageDriverSource(localPageDriverPath, candidate));
  assert.throws(() => restoreLocalPageDriverSource(localPageDriverPath, restoreLocalPageDriverSource(localPageDriverPath, physical)));
  assert.deepEqual(driverSource(), physical);
});

test('driver complete metadata hash rejects changed missing unknown reordered partial double or swapped reversals', () => {
  const candidates = [null, driverMetadata.toString(), new Uint8Array(driverMetadata), Buffer.from('{}'),
    Buffer.concat([driverMetadata, Buffer.from(' ')]), Buffer.from(JSON.stringify(driverData)),
    encoded(Object.fromEntries(Object.entries(driverData).reverse()))];
  for (const location of [null, 'row', 'preserved_local_page_metadata']) {
    const original = location === null ? driverData : driverData[location];
    for (const key of [...Object.keys(original), 'unreviewed']) for (const operation of ['delete', 'change']) {
      const value = structuredClone(driverData), target = location === null ? value : value[location];
      if (operation === 'delete') { if (!Object.hasOwn(target, key)) continue; delete target[key]; }
      else target[key] = 'unknown';
      candidates.push(encoded(value));
    }
  }
  for (let i = 0; i < driverData.row.literal_reversals.length; i++) for (const key of ['current', 'previous']) {
    for (const operation of ['delete', 'change']) {
      const value = structuredClone(driverData), edit = value.row.literal_reversals[i];
      if (operation === 'delete') delete edit[key]; else edit[key] += ' ';
      candidates.push(encoded(value));
    }
  }
  for (const mutate of [
    v => v.row.literal_reversals.reverse(), v => v.row.literal_reversals.pop(),
    v => v.row.literal_reversals.splice(0), v => v.row.literal_reversals.push(...structuredClone(v.row.literal_reversals)),
    v => v.row.literal_reversals.forEach(edit => { [edit.current, edit.previous] = [edit.previous, edit.current]; }),
    v => v.row.literal_reversals[0].unknown = true,
    v => v.row = Object.fromEntries(Object.entries(v.row).reverse()),
  ]) { const value = structuredClone(driverData); mutate(value); candidates.push(encoded(value)); }
  const physical = driverSource();
  for (const candidate of candidates) {
    assert.throws(() => readLocalPageDriverAssociation(candidate));
    assert.throws(() => restoreLocalPageDriverSource(localPageDriverPath, physical, candidate));
  }
  assert.throws(() => readLocalPageDriverAssociation(metadata));
  assert.throws(() => readLocalPageSourceAssociation(driverMetadata));
});

test('driver layer cannot be skipped doubled or substituted for old integration/history guards', () => {
  const physical = driverSource(), prior = restoreLocalPageDriverSource(localPageDriverPath, physical);
  assert.throws(() => restoreIntegratedSource(localPageDriverPath, physical));
  assert.throws(() => restoreHistorySource(localPageDriverPath, physical));
  assert.throws(() => restoreHistorySource(localPageDriverPath, prior));
  assert.throws(() => restoreLocalPageDriverSource(localPageDriverPath, restoreIntegratedSource(localPageDriverPath, prior)));
  assert.deepEqual(driverSource(), physical);
});

test('both history callers retain the exact reader-library dispatch alongside the driver-only stage', () => {
  for (const extension of ['mjs', 'py']) {
    const source = read('weebdex-platform/tests/ci/test_reviewed_history_source_successors.' + extension);
    assert.ok(source.includes('bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts'));
    assert.ok(source.includes('weebdex-platform/platform-integrations/local-library/library.ts'));
    if (extension === 'mjs') {
      assert.ok(source.includes('row.path===localPageDriverPath?restoreLocalPageDriverSource(row.path,physical):physical'));
      assert.ok(source.includes('restoreIntegratedSource(row.path,associated)'));
      assert.ok(source.includes('restoreBoundedReaderSource('));
    } else {
      assert.ok(source.includes("restore_local_page_source(row['path'],physical) if row['path']==DRIVER else physical"));
      assert.ok(source.includes("restore_integrated_source(row['path'],current)"));
      assert.ok(source.includes('restore_bounded_reader_source('));
    }
  }
  for (const extension of ['mjs', 'py']) {
    const source = read('weebdex-platform/tests/ci/test_reviewed_integration_source_successors.' + extension);
    assert.ok(extension === 'mjs'
      ? source.includes('row.path===localPageDriverPath?restoreLocalPageDriverSource(row.path,physical):physical')
      : source.includes("restore_local_page_source(row['path'],physical) if row['path']==DRIVER else physical"));
  }
});
