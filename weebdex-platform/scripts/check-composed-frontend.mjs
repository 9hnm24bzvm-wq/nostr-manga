#!/usr/bin/env node
// Materialize the actual production transforms in an ignored, local-only tree.
// This command never starts the application, a browser, or a database.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { frontendRouteAdditions } from '../platform-integrations/frontend-route-additions.mjs';
import { createFrontendOverlays } from '../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../platform-integrations/local-library/overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const frontend = path.join(root, 'frontend');
const dependencies = fs.realpathSync(process.env.WEEBDEX_SEMANTIC_NODE_MODULES ?? path.join(frontend, 'node_modules'));
const require = createRequire(path.join(dependencies, '../package.json'));
const ts = require('typescript');
const MagicString = require('magic-string');
const { TraceMap, originalPositionFor } = require('@jridgewell/trace-mapping');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const relative = file => path.relative(root, file).split(path.sep).join('/');
const writeJSON = (file, value) => { fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n'); };
function files(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap(entry => {
    const file = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw Error(`Unexpected source symlink: ${file}`);
    return entry.isDirectory() ? files(file) : entry.isFile() ? [file] : [];
  });
}
const canonicalList = spawnSync('git', ['ls-files', '-z', '--', 'frontend'], { cwd: root, encoding: 'utf8' });
if (canonicalList.status !== 0) throw Error('Cannot inventory tracked immutable frontend sources');
const canonicalFiles = canonicalList.stdout.split('\0').filter(Boolean).map(file => path.join(root, file));
const sourceInventory = () => Object.fromEntries(canonicalFiles.map(file => [relative(file), hash(fs.readFileSync(file))]));
const before = sourceInventory();
const work = path.resolve(process.env.WEEBDEX_SEMANTIC_OUTPUT ?? path.join(root, '.build/composed-semantic-check'));
if (!work.startsWith(path.join(root, '.build') + path.sep)) throw Error('Projection output must be inside this worktree .build directory');
if (fs.existsSync(work)) throw Error('Refusing to overwrite an existing semantic-check generation; choose a fresh output');
const input = path.join(work, 'production-input');
const project = path.join(work, 'project');
const evidence = path.join(work, 'evidence');
fs.mkdirSync(evidence, { recursive: true });
const copyFrontend = destination => { for (const source of canonicalFiles) { const target = path.join(destination, path.relative(frontend, source)); fs.mkdirSync(path.dirname(target), { recursive: true }); fs.copyFileSync(source, target); } };
copyFrontend(input);
copyFrontend(project);
// Own the node_modules container so Vite's temporary config bundle stays local.
// Individual dependency entries remain read-only inputs through symlinks.
function linkDependencies(destination) {
  const container = path.join(destination, 'node_modules'); fs.mkdirSync(container);
  for (const entry of fs.readdirSync(dependencies)) if (!entry.startsWith('.')) fs.symlinkSync(path.join(dependencies, entry), path.join(container, entry));
}
linkDependencies(input);
linkDependencies(project);
fs.cpSync(path.join(root, 'platform-integrations'), path.join(project, 'platform-integrations'), { recursive: true });
fs.cpSync(path.join(root, 'platform-integrations'), path.join(work, 'production-composed/platform-integrations'), { recursive: true });

// Reconstructed routes are installed on the production build input by its preparation step.
for (const addition of frontendRouteAdditions) {
  for (const tree of [input, project]) {
    const target = path.join(tree, addition.target), source = path.join(root, addition.source);
    if (fs.existsSync(target) && !fs.readFileSync(target).equals(fs.readFileSync(source))) throw Error(`Reviewed route source drift: ${addition.target}`);
    fs.mkdirSync(path.dirname(target), { recursive: true }); fs.copyFileSync(source, target);
  }
}

process.env.WEEBDEX_FRONTEND_ROOT = input;
// Loading the real production configuration obtains its aliases and resolveId
// hooks. It does not execute Vite build, listen(), or application modules.
const productionConfig = (await import(pathToFileURL(path.join(root, 'scripts/vite.platform.config.mjs')))).default;
const previousCwd = process.cwd();
process.chdir(input);
const config = await productionConfig({ command: 'build', mode: 'platform', isSsrBuild: false, isPreview: false });
process.chdir(previousCwd);
const resolvers = config.plugins.flat(Infinity).filter(plugin => plugin && typeof plugin.resolveId === 'function' && ['attachment-exact-import-case-repair', 'weebdex-local-library-api-resolver'].includes(plugin.name));
if (resolvers.length !== 2) throw Error('Production exact resolver set changed');
const aliases = Array.isArray(config.resolve.alias) ? config.resolve.alias : Object.entries(config.resolve.alias).map(([find, replacement]) => ({ find, replacement }));
const relocate = absolute => absolute.startsWith(input + path.sep) ? path.join(project, path.relative(input, absolute)) : absolute.startsWith(path.join(root, 'platform-integrations') + path.sep) ? path.join(project, relative(absolute)) : absolute;
const productionId = file => file.startsWith(path.join(project, 'src') + path.sep) ? path.join(input, path.relative(project, file)) : file.startsWith(path.join(project, 'platform-integrations') + path.sep) ? path.join(root, path.relative(project, file)) : file;
const aliasesForCheck = {};
for (const { find, replacement } of aliases) {
  if (typeof find !== 'string') throw Error('Unsupported non-string production alias');
  const target = relocate(replacement);
  aliasesForCheck[find] = [target];
  if (find.startsWith('$') || find.startsWith('@noble/')) aliasesForCheck[find + '/*'] = [target + '/*'];
}
aliasesForCheck.$lib = [path.join(project, 'src/lib')];
aliasesForCheck['$lib/*'] = [path.join(project, 'src/lib/*')];
// Runtime aliases target JS entrypoints. Use each same package's exported
// declaration entrypoint instead of treating its JS implementation as untyped.
const packageTypeAliases = [];
for (const name of ['svelte-sonner', 'axios', 'http-status-codes']) {
  const pkg = JSON.parse(fs.readFileSync(path.join(dependencies, name, 'package.json')));
  const runtimeEntry = aliases.find(alias => alias.find === name)?.replacement;
  const siblingDeclaration = runtimeEntry?.replace(/\.js$/, '.d.ts');
  const typeEntry = pkg.types ?? pkg.typings ?? pkg.exports?.['.']?.types ?? (siblingDeclaration && fs.existsSync(siblingDeclaration) ? path.relative(path.join(dependencies, name), siblingDeclaration) : undefined);
  if (typeof typeEntry !== 'string') throw Error(`Production dependency has no declared types: ${name}`);
  aliasesForCheck[name] = [path.join(dependencies, name, typeEntry)];
  packageTypeAliases.push({ name, production_entry: aliases.find(alias => alias.find === name)?.replacement, same_package_declaration: aliasesForCheck[name][0], declaration_sha256: hash(fs.readFileSync(aliasesForCheck[name][0])) });
}
const plugins = [createFrontendOverlays(input), createReaderOverlays(input), createLocalLibraryOverlays(input)];
const manifest = [], rewrites = [], maps = new Map(), graph = [], reachable = new Set(), workerTypeBindings = [];
const projectedSourceRoots = ['src', 'platform-integrations'].map(name => path.join(project, name) + path.sep);
function isProjectedSource(file) {
  if (!projectedSourceRoots.some(prefix => file.startsWith(prefix))) return false;
  const real = fs.realpathSync(file);
  return projectedSourceRoots.some(prefix => real.startsWith(prefix));
}
function scriptRanges(code, file) {
  if (!file.endsWith('.svelte')) return [{ start: 0, code }];
  return [...code.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)].map(match => ({ start: match.index + match[0].indexOf('>') + 1, code: match[1] }));
}
function references(code, file) {
  const result = [];
  for (const range of scriptRanges(code, file)) {
    const ast = ts.createSourceFile(file + '.ts', range.code, ts.ScriptTarget.Latest, true);
    function visit(node) {
      let literal;
      if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) literal = node.moduleSpecifier;
      if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword && ts.isStringLiteral(node.arguments[0])) literal = node.arguments[0];
      if (ts.isNewExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'URL' && ts.isStringLiteral(node.arguments?.[0]) && node.arguments?.[1]?.getText(ast) === 'import.meta.url') literal = node.arguments[0];
      if (literal) result.push({ specifier: literal.text, start: range.start + literal.getStart(ast) + 1, end: range.start + literal.end - 1 });
      ts.forEachChild(node, visit);
    }
    visit(ast);
  }
  return result;
}
function sourceTarget(specifier, importer) {
  for (const resolver of resolvers) {
    const result = resolver.resolveId(specifier, productionId(importer));
    if (result) return { target: relocate(typeof result === 'string' ? result : result.id), resolver: resolver.name };
  }
  let target;
  for (const alias of aliases) if (specifier === alias.find || specifier.startsWith(alias.find + '/')) {
    target = relocate(alias.replacement + specifier.slice(alias.find.length)); break;
  }
  if (!target && (specifier === '$lib' || specifier.startsWith('$lib/'))) target = path.join(project, 'src/lib', specifier.slice(5));
  if (!target && specifier.startsWith('.')) target = path.resolve(path.dirname(importer), specifier);
  if (!target) return null;
  const sourceSubstitutions = target.endsWith('.js') ? [target.slice(0, -3) + '.ts', target.slice(0, -3) + '.tsx'] : target.endsWith('.svelte') ? [target + '.ts'] : [];
  for (const candidate of [target, ...sourceSubstitutions, target + '.ts', target + '.js', target + '.svelte', target + '.svelte.ts', path.join(target, 'index.ts'), path.join(target, 'index.js')]) if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return { target: isProjectedSource(candidate) ? candidate : fs.realpathSync(candidate), resolver: 'production-alias-or-relative' };
  return { target, resolver: 'unresolved-production-alias-or-relative' };
}
const frontendFiles = files(path.join(input, 'src'));
for (const source of frontendFiles) {
  const target = path.join(project, path.relative(input, source));
  const original = fs.readFileSync(source, 'utf8');
  let code = original; const stages = [];
  if (/\.(svelte|[cm]?[jt]s)$/.test(source)) for (const plugin of plugins) {
    const result = plugin.transform.handler(code, source);
    if (result) { code = result.code; const stagePath = path.join(work, 'production-stages', plugin.name, path.relative(input, source)); fs.mkdirSync(path.dirname(stagePath), { recursive: true }); fs.writeFileSync(stagePath, code); stages.push({ plugin: plugin.name, sha256: hash(code), snapshot: path.relative(work, stagePath) }); }
  }
  fs.writeFileSync(target, code);
  const snapshot = path.join(work, 'production-composed', path.relative(input, source));
  fs.mkdirSync(path.dirname(snapshot), { recursive: true }); fs.writeFileSync(snapshot, code);
  manifest.push({ canonical: path.relative(input, source), original_sha256: hash(original), production_composed_sha256: hash(code), stages, projection: path.relative(project, target), production_snapshot: path.relative(work, snapshot) });
}

// Frontend source files are check roots, except two modules fully redirected by
// production resolveId. Integrations are then discovered transitively, including
// dynamic imports and the URL-based worker entrypoint.
const redirected = new Set(['src/lib/API/MangaFollow.ts', 'src/lib/API/ChapterHistory.ts']);
const pending = files(path.join(project, 'src')).filter(file => /\.(svelte|[cm]?[jt]s)$/.test(file) && !redirected.has(path.relative(project, file)));
while (pending.length) {
  const file = pending.shift(); if (reachable.has(file)) continue;
  if (!isProjectedSource(file)) throw Error(`Refusing dependency or non-source projection write: ${file}`);
  reachable.add(file);
  const code = fs.readFileSync(file, 'utf8');
  const magic = new MagicString(code); const inputRefs = references(code, file);
  // These two URL-based worker entrypoints execute with DedicatedWorkerGlobalScope,
  // although their type-only application imports share the main Window program.
  // Bind only the platform's `self` name, using existing lib.webworker types.
  // No application values, nullable buffers, callback signatures or errors are widened.
  const projectedName = path.relative(project, file);
  if (projectedName === 'platform-integrations/local-library/projection-worker.ts') {
    magic.prepend('/// <reference lib="webworker" />\ndeclare const self: DedicatedWorkerGlobalScope;\n');
    workerTypeBindings.push({ file: projectedName, binding: 'module-local ambient self: DedicatedWorkerGlobalScope', runtime_effect: 'none; declaration is erased' });
  }
  if (projectedName === 'platform-integrations/local-library/protocol/worker.js') {
    magic.prepend('/// <reference lib="webworker" />\n/** @type {DedicatedWorkerGlobalScope} */\nconst __semanticWorkerSelf = /** @type {DedicatedWorkerGlobalScope} */ (/** @type {unknown} */ (self));\n');
    const ast = ts.createSourceFile(file, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
    function bindWorkerScope(node) { if (ts.isIdentifier(node) && node.text === 'self') magic.overwrite(node.getStart(ast), node.end, '__semanticWorkerSelf'); ts.forEachChild(node, bindWorkerScope); }
    bindWorkerScope(ast);
    workerTypeBindings.push({ file: projectedName, binding: 'JSDoc DedicatedWorkerGlobalScope assertion on the worker platform self only', runtime_effect: 'projection-local alias of the same self object' });
  }
  for (const reference of inputRefs) {
    const resolved = sourceTarget(reference.specifier, file);
    if (resolved) {
      graph.push({ importer: path.relative(project, file), specifier: reference.specifier, target: resolved.target.startsWith(project + path.sep) ? path.relative(project, resolved.target) : resolved.target, resolver: resolved.resolver });
      if (fs.existsSync(resolved.target) && isProjectedSource(resolved.target) && /\.(svelte|[cm]?[jt]s)$/.test(resolved.target)) pending.push(resolved.target);
      if (resolved.resolver !== 'production-alias-or-relative' && !resolved.resolver.startsWith('unresolved')) {
        let replacement = path.relative(path.dirname(file), resolved.target).split(path.sep).join('/');
        if (!replacement.startsWith('.')) replacement = './' + replacement;
        magic.overwrite(reference.start, reference.end, replacement);
        rewrites.push({ importer: path.relative(project, file), specifier: reference.specifier, replacement, target: path.relative(project, resolved.target), resolver: resolved.resolver });
      }
    }
  }
  const output = magic.toString(); fs.writeFileSync(file, output);
  const isFrontend = file.startsWith(path.join(project, 'src') + path.sep);
  const snapshot = path.join(work, 'production-composed', path.relative(project, file));
  const mapPath = path.join(evidence, 'source-maps', path.relative(project, file) + '.map');
  // MagicString relativizes `source` against `file`. Supply only the already
  // map-relative source, then set the standard map file field independently.
  const map = JSON.parse(magic.generateMap({ hires: true, includeContent: true, source: path.relative(path.dirname(mapPath), snapshot) }).toString());
  map.file = path.relative(path.dirname(mapPath), file);
  writeJSON(mapPath, map); maps.set(path.relative(project, file), new TraceMap(map, pathToFileURL(mapPath).href));
  const entry = manifest.find(item => item.projection === path.relative(project, file));
  if (entry) { entry.projection_sha256 = hash(output); entry.source_map = path.relative(work, mapPath); entry.source_map_sha256 = hash(fs.readFileSync(mapPath)); }
  else manifest.push({ canonical: relative(path.join(root, path.relative(project, file))), original_sha256: hash(code), production_composed_sha256: hash(code), projection: path.relative(project, file), projection_sha256: hash(output), production_snapshot: path.relative(work, snapshot), stages: [], source_map: path.relative(work, mapPath), source_map_sha256: hash(fs.readFileSync(mapPath)) });
}
const unresolvedGraphEdges = graph.filter(edge => edge.resolver.startsWith('unresolved'));
if (unresolvedGraphEdges.length) throw Error(`Projection graph has ${unresolvedGraphEdges.length} unresolved source edges`);
const mapValidation = manifest.filter(entry => entry.source_map).map(entry => {
  const mapPath = path.join(work, entry.source_map), map = JSON.parse(fs.readFileSync(mapPath));
  if (map.sources.length !== 1) throw Error(`Unexpected multi-source projection map: ${entry.source_map}`);
  const source = path.resolve(path.dirname(mapPath), map.sources[0]);
  const sourceHash = hash(fs.readFileSync(source));
  if (sourceHash !== entry.production_composed_sha256 || hash(map.sourcesContent[0]) !== sourceHash) throw Error(`Map source bytes differ from composed manifest: ${entry.source_map}`);
  if (path.resolve(path.dirname(mapPath), map.file) !== path.join(project, entry.projection)) throw Error(`Map output file pointer is invalid: ${entry.source_map}`);
  return { map: entry.source_map, source: path.relative(work, source), source_sha256: sourceHash, embedded_content_matches: true };
});

const fragments = files(path.join(root, 'platform-integrations')).filter(file => file.endsWith('.fragment.ts')).map(file => {
  const fragment = fs.readFileSync(file, 'utf8').trim();
  const normalized = value => value.replace(/\r\n/g, '\n').trim();
  const destinations = manifest.filter(entry => entry.production_snapshot && normalized(fs.readFileSync(path.join(work, entry.production_snapshot), 'utf8')).includes(normalized(fragment))).map(entry => entry.projection);
  const recipeLines = [...new Set(normalized(fragment).split('\n').map(line => line.trim()).filter(line => line.length > 12))];
  const derived = manifest.filter(entry => entry.production_snapshot && entry.stages.length).map(entry => {
    const finalLines = new Set(normalized(fs.readFileSync(path.join(work, entry.production_snapshot), 'utf8')).split('\n').map(line => line.trim()));
    const matching = recipeLines.filter(line => finalLines.has(line)).length;
    return { projection: entry.projection, matching_recipe_lines: matching, distinctive_recipe_lines: recipeLines.length, exact_fraction: matching / recipeLines.length };
  }).filter(entry => entry.exact_fraction > 0.5).sort((a, b) => b.exact_fraction - a.exact_fraction);
  if (!destinations.length && !derived.length) throw Error(`No composed production origin found for fragment recipe: ${relative(file)}`);
  return { path: relative(file), sha256: hash(fs.readFileSync(file)), exact_inserted_into: destinations, embedded_recipe_derivations: derived, standalone_check_root: false, qualification: destinations.length ? 'Exact normalized recipe present in composed production source' : 'Reference fragment recipe has subsequent reviewed repairs already embedded in production overlay literals; exact production plugin output is checked, never the stale standalone recipe' };
});
const env = { ...process.env, PUBLIC_SITE_NAME: 'WeebDex', PUBLIC_API_URL: '/api', PUBLIC_KRATOS_PUBLIC_API: '/auth', PUBLIC_COVER_URL: '/covers', PUBLIC_DATA_URL: '/data', PUBLIC_UPLOAD_URL: '/uploads', PUBLIC_PROXY_URL: '/proxy', PUBLIC_OPENGRAPH_URL: '/og-image' };
function run(label, args) {
  const started = Date.now();
  // svelte-check calls process.exit immediately after printing. Regular files
  // make Node stdout synchronous, preventing a truncated buffered pipe receipt.
  const stdoutPath = path.join(evidence, label + '.stdout.log'), stderrPath = path.join(evidence, label + '.stderr.log');
  const stdoutFd = fs.openSync(stdoutPath, 'wx'), stderrFd = fs.openSync(stderrPath, 'wx');
  let result;
  try { result = spawnSync(process.execPath, args, { cwd: project, env, timeout: 300_000, stdio: ['ignore', stdoutFd, stderrFd] }); }
  finally { fs.closeSync(stdoutFd); fs.closeSync(stderrFd); }
  result.stdout = fs.readFileSync(stdoutPath, 'utf8'); result.stderr = fs.readFileSync(stderrPath, 'utf8');
  fs.writeFileSync(path.join(evidence, label + '.log'), result.stdout + result.stderr);
  const receipt = { command: [process.execPath, ...args], exit_code: result.status, signal: result.signal, error: result.error?.message, elapsed_ms: Date.now() - started };
  writeJSON(path.join(evidence, label + '-receipt.json'), receipt); return { ...result, receipt };
}
const sync = run('svelte-kit-sync', [path.join(dependencies, '@sveltejs/kit/svelte-kit.js'), 'sync']);
if (sync.status !== 0) throw Error(`Existing SvelteKit sync failed; see ${evidence}`);
const generated = JSON.parse(fs.readFileSync(path.join(project, '.svelte-kit/tsconfig.json')));
const originalConfig = ts.parseConfigFileTextToJson('tsconfig.json', fs.readFileSync(path.join(project, 'tsconfig.json'), 'utf8'));
if (originalConfig.error) throw Error('Original TypeScript configuration could not be parsed');
const worker = path.join(project, 'src/service-worker.ts');
const integrationRoots = [...reachable].filter(file => file.startsWith(path.join(project, 'platform-integrations') + path.sep));
const generatedPaths = Object.fromEntries(Object.entries(generated.compilerOptions.paths).map(([key, targets]) => [key, targets.map(target => path.resolve(project, '.svelte-kit', target))]));
const tsconfig = { ...originalConfig.config, compilerOptions: { ...originalConfig.config.compilerOptions, paths: { ...generatedPaths, ...aliasesForCheck }, types: [...generated.compilerOptions.types, 'vite/client', 'vite-plugin-pwa/info'] }, include: [...generated.include.map(pattern => pattern.startsWith('../') ? pattern.slice(3) : '.svelte-kit/' + pattern.replace(/^\.\//, '')), ...integrationRoots.map(file => path.relative(project, file))], exclude: [...generated.exclude.map(pattern => pattern.replace(/^\.\.\//, '')), ...redirected] };
writeJSON(path.join(project, 'tsconfig.semantic.json'), tsconfig);
const checked = run('svelte-check', [path.join(dependencies, 'svelte-check/bin/svelte-check'), '--tsconfig', './tsconfig.semantic.json', '--output', 'machine']);
// Service-worker source is deliberately isolated from the Window/DOM program,
// matching SvelteKit's own generated exclusion and existing webworker reference.
const workerConfig = { extends: './tsconfig.semantic.json', compilerOptions: { lib: ['ESNext', 'WebWorker'], types: ['node', 'vite/client'] }, include: ['src/service-worker.ts'], exclude: [] };
writeJSON(path.join(project, 'tsconfig.worker.json'), workerConfig);
const workerCheck = run('service-worker-tsc', [path.join(dependencies, 'typescript/bin/tsc'), '--project', './tsconfig.worker.json', '--pretty', 'false']);
const diagnostics = [];
for (const line of (checked.stdout ?? '').split('\n')) {
  const match = line.match(/^\d+ (ERROR|WARNING) ("(?:[^"\\]|\\.)*") (\d+):(\d+) ("(?:[^"\\]|\\.)*")$/);
  if (!match) continue;
  const file = JSON.parse(match[2]), lineNumber = +match[3], column = +match[4];
  const map = maps.get(file);
  diagnostics.push({ severity: match[1].toLowerCase(), projection_file: file, line: lineNumber, column, message: JSON.parse(match[5]), production_source_position: map ? originalPositionFor(map, { line: lineNumber, column: column - 1 }) : null });
}
const completed = (checked.stdout ?? '').match(/COMPLETED (\d+) FILES (\d+) ERRORS (\d+) WARNINGS (\d+) FILES_WITH_PROBLEMS/);
if (!completed || +completed[2] !== diagnostics.filter(item => item.severity === 'error').length || +completed[3] !== diagnostics.filter(item => item.severity === 'warning').length) throw Error('Semantic checker did not produce a complete, parsable diagnostic receipt');
const toolsUsed = ['typescript', 'svelte-check', 'svelte', '@sveltejs/kit', 'vite', 'magic-string', '@jridgewell/trace-mapping'].map(name => { const file = path.join(dependencies, name, 'package.json'), pkg = JSON.parse(fs.readFileSync(file)); return { package: name, version: pkg.version, package_sha256: hash(fs.readFileSync(file)) }; });
const after = sourceInventory();
if (JSON.stringify(before) !== JSON.stringify(after)) throw Error('Canonical frontend inventory changed');
const summary = { status: checked.status === 0 && workerCheck.status === 0 ? 'passed' : 'failed', utc: new Date().toISOString(), base_commit: spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim(), canonical_frontend_files: Object.keys(before).length, canonical_frontend_unchanged: true, runtime_browser_database_services_started: false, installed_dependencies: false, production_configuration_sha256: hash(fs.readFileSync(path.join(root, 'scripts/vite.platform.config.mjs'))), production_overlay_order: plugins.map(plugin => plugin.name), production_aliases: aliases, declaration_entrypoints_for_runtime_package_aliases: packageTypeAliases, worker_platform_type_bindings: workerTypeBindings, strict: tsconfig.compilerOptions.strict, checkJs: tsconfig.compilerOptions.checkJs, skipLibCheck: tsconfig.compilerOptions.skipLibCheck, skipLibCheck_inherited_from_original: true, source_diagnostic_suppressions_added: false, reachable_integration_files: integrationRoots.map(file => path.relative(project, file)).sort(), redirected_inactive_source_roots: [...redirected], fragments, resolver_rewrites: rewrites, errors: diagnostics.filter(item => item.severity === 'error').length, warnings: diagnostics.filter(item => item.severity === 'warning').length, svelte_check_receipt: checked.receipt, service_worker_receipt: workerCheck.receipt, limitation: 'Projection maps resolve to exact composed production snapshots, not falsely to pre-overlay source. Stage hashes preserve original → shared → reader → local provenance. No application behavior is changed; residual diagnostics are reported.' };
writeJSON(path.join(evidence, 'cached-toolchain.json'), toolsUsed);
writeJSON(path.join(evidence, 'source-map-validation.json'), { status: 'passed', validated_maps: mapValidation.length, maps: mapValidation });
writeJSON(path.join(evidence, 'canonical-source-hashes.json'), before);
writeJSON(path.join(evidence, 'projection-manifest.json'), manifest);
writeJSON(path.join(evidence, 'production-module-graph.json'), graph);
writeJSON(path.join(evidence, 'diagnostics.json'), diagnostics);
writeJSON(path.join(evidence, 'summary.json'), summary);
console.log(JSON.stringify({ status: summary.status, errors: summary.errors, warnings: summary.warnings, integration_files: integrationRoots.length, canonical_files: summary.canonical_frontend_files, canonical_unchanged: true, evidence: relative(evidence) }));
process.exitCode = summary.status === 'passed' ? 0 : 1;
