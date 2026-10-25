// Finite actual-composition type regression, not a whole-application or DOM pass.
// Uses existing pinned dependencies only; no writes, subprocesses or runtime Git.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
import { createPublicUploadOverlays } from '../../platform-integrations/public-api/upload-overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT;
assert.ok(dependencies, 'Use the existing verified frontend dependency stage');
const require = createRequire(path.join(dependencies, 'package.json'));
const ts = require('typescript');
const svelte = require('svelte/compiler');
const hash = value => createHash('sha256').update(value).digest('hex');
const uploadRelative = 'src/lib/components/upload/UploadChapterMetadata.svelte';
const inputRelative = 'src/lib/components/ui/input/input.svelte';
const read = relative => fs.readFileSync(path.join(frontend, relative), 'utf8');

function one(values, description) {
  assert.equal(values.length, 1, description);
  return values[0];
}

function script(source, filename) {
  const match = one([...source.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)], filename + ' has one script');
  const parsed = ts.createSourceFile(filename + '.ts', match[1], ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  assert.deepEqual(parsed.parseDiagnostics, [], filename + ' parses without recovery');
  return parsed;
}

function alias(source, name) {
  return one(source.statements.filter(node => ts.isTypeAliasDeclaration(node) && node.name.text === name),
    'Use the actual unique ' + name + ' declaration').getText(source);
}

function walk(node, visit) {
  if (!node || typeof node !== 'object') return;
  if (Array.isArray(node)) { for (const child of node) walk(child, visit); return; }
  visit(node);
  for (const value of Object.values(node)) walk(value, visit);
}

function languageInput(source) {
  const parsed = svelte.parse(source, { filename: uploadRelative, modern: true });
  const matches = [];
  walk(parsed.fragment, node => {
    if (node.type !== 'Component' || node.name !== 'Input') return;
    if (node.attributes.some(attribute => attribute.type === 'BindDirective' && attribute.name === 'value'
      && attribute.expression?.type === 'MemberExpression' && !attribute.expression.computed
      && attribute.expression.object.type === 'Identifier' && attribute.expression.object.name === 'draft'
      && attribute.expression.property.type === 'Identifier' && attribute.expression.property.name === 'language')) matches.push(node);
  });
  const component = one(matches, 'The composed language binding uses exactly one actual Input');
  return {
    attribute: name => one(component.attributes.filter(node => node.type === 'Attribute' && node.name === name),
      'The language Input has exactly one ' + name + ' attribute')
  };
}

// Preserve Svelte's distinction between quoted text and a numeric expression.
// Do not coerce strings with Number(), parseInt(), or a fabricated permissive prop.
function literal(attribute) {
  if (Array.isArray(attribute.value)) {
    const text = one(attribute.value, 'A quoted prop has one literal text node');
    assert.equal(text.type, 'Text');
    return JSON.stringify(text.data);
  }
  assert.equal(attribute.value.type, 'ExpressionTag');
  assert.equal(attribute.value.expression.type, 'Literal');
  assert.equal(typeof attribute.value.expression.value, 'number');
  return JSON.stringify(attribute.value.expression.value);
}

function checkProps(declarations, component) {
  // Project only the implicated literal props into the *complete, unchanged*
  // canonical Props alias. This is a focused typed composition, not svelte-check.
  const source = declarations + '\nconst uploadLanguageInput: Props = {\n'
    + '  type: ' + literal(component.attribute('type')) + ',\n'
    + '  maxlength: ' + literal(component.attribute('maxlength')) + '\n};\n';
  const filename = path.join(dependencies, '__weebdex_upload_language_input_type__.ts');
  assert.equal(fs.existsSync(filename), false, 'The typed projection is virtual, never a dependency-stage write');
  const options = {
    strict: true, skipLibCheck: false, noEmit: true,
    target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    lib: ['lib.esnext.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'], types: []
  };
  const host = ts.createCompilerHost(options);
  const originalGetSourceFile = host.getSourceFile.bind(host);
  const originalFileExists = host.fileExists.bind(host);
  const originalReadFile = host.readFile.bind(host);
  const virtual = file => path.resolve(file) === filename;
  host.getSourceFile = (file, languageVersion, onError, shouldCreateNewSourceFile) => virtual(file)
    ? ts.createSourceFile(file, source, languageVersion, true, ts.ScriptKind.TS)
    : originalGetSourceFile(file, languageVersion, onError, shouldCreateNewSourceFile);
  host.fileExists = file => virtual(file) || originalFileExists(file);
  host.readFile = file => virtual(file) ? source : originalReadFile(file);
  const resolved = ts.resolveModuleName('svelte/elements', filename, options, host).resolvedModule;
  assert.ok(resolved, 'Resolve the real installed Svelte element declarations');
  const elements = path.join(path.dirname(require.resolve('svelte/package.json')), 'elements.d.ts');
  assert.equal(fs.realpathSync(resolved.resolvedFileName), fs.realpathSync(elements));
  const program = ts.createProgram([filename], options, host);
  const declaration = program.getSourceFile(resolved.resolvedFileName);
  assert.ok(declaration, 'The semantic program actually includes svelte/elements');
  assert.equal(declaration.text, fs.readFileSync(elements, 'utf8'));
  const diagnostics = ts.getPreEmitDiagnostics(program);
  return { filename, source, diagnostics };
}

test('actual public upload numeric maxlength satisfies real Input Props; exact old string fails TS2322', () => {
  assert.equal(ts.version, '5.9.3');
  assert.equal(require('svelte/package.json').version, '5.55.1');
  assert.equal(svelte.VERSION, '5.55.1');
  const source = read(uploadRelative);
  assert.equal(hash(source), 'c94de7465c23c046f6277dae37266b845b62ba8c5097980a3629014f6f3d7ff9');
  const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend),
    createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled: true }),
    createPublicUploadOverlays(frontend, { enabled: true })];
  const filename = path.join(frontend, uploadRelative);
  let composed = source;
  for (const [index, plugin] of plugins.entries()) {
    if (index === 4) assert.equal(hash(composed), 'e34402aebd6fae2085c3dedcaf650a0bc242c5d921ac2217c9c46515eaed1879');
    composed = plugin.transform.handler(composed, filename)?.code ?? composed;
  }
  assert.equal(hash(composed), 'ec72abd0e8bb8ccc8925fcf45e65c963c0f1ce53b2fdc7f964bd4c8fb13bde63');

  const uploadScript = script(composed, uploadRelative);
  const inputImport = one(uploadScript.statements.filter(node => ts.isImportDeclaration(node)
    && node.moduleSpecifier.text === '$lib/components/ui/input'), 'Use the canonical Input import');
  assert.ok(inputImport.importClause.namedBindings.elements.some(node => node.name.text === 'Input'
    && (!node.propertyName || node.propertyName.text === 'Input')));
  const index = read('src/lib/components/ui/input/index.ts');
  assert.match(index, /import Root from "\.\/input\.svelte";/);
  assert.match(index, /Root as Input/);

  // These declarations are copied verbatim from canonical sources, not restated
  // or widened in a test fixture. Source-parity hashes guard their provenance.
  const parity = JSON.parse(fs.readFileSync(path.join(root, 'test-evidence/frontend/source-parity.json'), 'utf8'));
  for (const relative of [inputRelative, 'src/lib/Utils.ts', 'src/lib/components/ui/input/index.ts']) {
    const row = one(parity.files.filter(entry => entry.path === relative), 'Canonical source parity: ' + relative);
    assert.equal(hash(read(relative)), row.source_sha256);
  }
  const inputScript = script(read(inputRelative), inputRelative);
  const elementsImport = one(inputScript.statements.filter(node => ts.isImportDeclaration(node)
    && node.moduleSpecifier.text === 'svelte/elements'), 'Use the real Svelte element type import');
  assert.equal(elementsImport.importClause.isTypeOnly, true);
  const utils = ts.createSourceFile('Utils.ts', read('src/lib/Utils.ts'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  assert.deepEqual(utils.parseDiagnostics, [], 'Canonical utility declarations parse without recovery');
  const propsBinding = inputScript.statements.filter(ts.isVariableStatement)
    .flatMap(node => [...node.declarationList.declarations])
    .filter(node => node.initializer && ts.isCallExpression(node.initializer) && ts.isIdentifier(node.initializer.expression)
      && node.initializer.expression.text === '$props');
  assert.equal(one(propsBinding, 'The real Input props binding').type.getText(inputScript), 'Props');
  const declarations = [elementsImport.getText(inputScript), alias(utils, 'WithElementRef'),
    alias(inputScript, 'InputType'), alias(inputScript, 'Props')].join('\n');

  const current = languageInput(composed);
  const maxlength = current.attribute('maxlength');
  assert.equal(composed.slice(maxlength.start, maxlength.end), 'maxlength={5}');
  assert.equal(literal(maxlength), '5');
  const corrected = checkProps(declarations, current);
  assert.deepEqual(corrected.diagnostics.map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')), []);

  // A one-attribute reversion must reproduce the *whole* failed production
  // snapshot's independently verified digest, not merely a lookalike snippet.
  const previous = composed.slice(0, maxlength.start) + 'maxlength="5"' + composed.slice(maxlength.end);
  assert.equal(hash(previous), 'b654ffad8acfb5c019d68cfa6b2df62da573224639c1025b37f9ae35c314c2cc');
  const oldInput = languageInput(previous);
  assert.equal(literal(oldInput.attribute('maxlength')), '"5"');
  const rejected = checkProps(declarations, oldInput);
  const diagnostic = one(rejected.diagnostics, 'The old source fails for exactly one real type error');
  assert.equal(diagnostic.category, ts.DiagnosticCategory.Error);
  assert.equal(diagnostic.code, 2322);
  assert.equal(diagnostic.file.fileName, rejected.filename);
  assert.equal(rejected.source.slice(diagnostic.start, diagnostic.start + diagnostic.length), 'maxlength');
  assert.match(ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'), /Type 'string' is not assignable to type 'number/);
});
