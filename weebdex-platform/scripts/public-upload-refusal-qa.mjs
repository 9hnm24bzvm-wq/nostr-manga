// Default-off, client-build-only read facade over the application's exact module.
// No separate transport bundle, global callback, DOM hook or runtime mutation API.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
const hash = value => createHash('sha256').update(value).digest('hex');
export const uploadRefusalReaderFile = '_app/immutable/qa/public-upload-refusals.js';
export const uploadRefusalGraphFile = '_app/immutable/qa/public-upload-refusals.graph.json';
export const uploadRefusalReaderExport = 'readUploadRequestRefusals';
const virtualId = '\0virtual:public-upload-refusal-reader';
const off = 'const uploadRequestRefusalRecording = false;';
const on = 'const uploadRequestRefusalRecording = true;';
export function readUploadRefusalQaGate(value = process.env.WEEBDEX_QA_UPLOAD_REFUSALS) {
  if (value === undefined || value === '0') return false;
  if (value === '1') return true;
  throw Error('WEEBDEX_QA_UPLOAD_REFUSALS must be absent, 0 or 1');
}
const clean = id => id.split(/[?#]/, 1)[0];
function canonical(id) { try { return fs.realpathSync(clean(id)); } catch { return clean(id); } }
const has = (chunk, id) => Object.keys(chunk.modules).filter(value => canonical(value) === id);
const reaches = (chunks, start, target, seen = new Set(), dynamic = false) => {
  if (start === target) return true;
  if (seen.has(start)) return false;
  seen.add(start); const row = chunks.get(start);
  if (!row) throw Error('QA graph has an absent imported chunk');
  return [...row.imports, ...(dynamic ? row.dynamicImports : [])].some(next => reaches(chunks, next, target, seen, dynamic));
};
export function stripUploadRefusalQaSource(code) {
  for (const name of ['RING', 'ID', 'PARSE', 'RECORD']) {
    const start = new RegExp('^[ \t]*// QA_UPLOAD_REFUSAL_' + name + '_BEGIN\\n[\\s\\S]*?^[ \t]*// QA_UPLOAD_REFUSAL_' + name + '_END\\n', 'gm');
    const matches = [...code.matchAll(start)];
    if (matches.length !== 1) throw Error('QA source region changed: ' + name);
    code = code.replace(start, '');
  }
  return code.replace('requestRefusals.set(error, refusalStage);\n      throw error;', 'requestRefusals.set(error, refusalStage); throw error;');
}
export function createUploadRefusalQaPlugin(root, { enabled = readUploadRefusalQaGate(), publicUploadEnabled = false } = {}) {
  if (typeof enabled !== 'boolean' || typeof publicUploadEnabled !== 'boolean') throw Error('QA gate must be boolean');
  if (enabled && !publicUploadEnabled) throw Error('QA refusal reader requires the explicit public upload build');
  root = path.resolve(root);
  const transport = fs.realpathSync(path.join(root, 'platform-integrations/public-api/nostr-upload-request.ts'));
  const client = fs.realpathSync(path.join(root, 'platform-integrations/public-api/upload-client.ts'));
  const pluginSource = fs.readFileSync(new URL(import.meta.url)), configSource = fs.readFileSync(path.join(root, 'scripts/vite.platform.config.mjs'));
  const transportSource = fs.readFileSync(transport, 'utf8'), clientSource = fs.readFileSync(client);
  if (transportSource.split(off).length !== 2) throw Error('Default-off refusal recording anchor changed');
  let clientBuild = false, transformed = false, emitted = false;
  return {
    name: 'bounded-public-upload-refusal-qa', apply: 'build', enforce: 'pre',
    configResolved(config) { clientBuild = !config.build.ssr; },
    buildStart() {
      transformed = false; emitted = false;
      if (enabled && clientBuild) {
        this.emitFile({ type: 'chunk', id: virtualId, fileName: uploadRefusalReaderFile, preserveSignature: 'strict' }); emitted = true;
      }
    },
    resolveId(id) { if (id === virtualId && enabled && clientBuild) return virtualId; return null; },
    load(id) {
      if (id !== virtualId || !enabled || !clientBuild) return null;
      return `export { ${uploadRefusalReaderExport} } from ${JSON.stringify(transport)};`;
    },
    transform(code, id) {
      if (canonical(id) !== transport) return null;
      if (id !== transport) throw Error('QA transport must have one canonical module identity');
      if (code !== transportSource) throw Error('QA transport input bytes changed before composition');
      if (!enabled || !clientBuild) return { code: stripUploadRefusalQaSource(code), map: null };
      if (transformed) throw Error('QA transport was transformed more than once');
      transformed = true; return { code: code.replace(off, on), map: null };
    },
    // Vite's normal generateBundle hook rewrites import-preload code first.
    generateBundle: { order: 'post', handler(_options, bundle) {
      const output = Object.values(bundle), chunks = output.filter(row => row.type === 'chunk');
      if (chunks.some(row => typeof row.fileName !== 'string' || row.fileName.startsWith('/') || row.fileName.includes('\\') || row.fileName.split('/').some(part => !part || part === '.' || part === '..') || !row.fileName.endsWith('.js'))) throw Error('QA graph chunk names must be relative JavaScript assets');
      if (chunks.length > 2048) throw Error('QA client chunk graph exceeds its finite bound');
      if (!enabled || !clientBuild) {
        if (output.some(row => row.fileName === uploadRefusalReaderFile || row.fileName === uploadRefusalGraphFile) ||
            chunks.some(row => row.facadeModuleId === virtualId || Object.hasOwn(row.modules, virtualId) || row.code.includes(uploadRefusalReaderExport) || row.code.includes('QA_UPLOAD_REFUSAL_') || row.code.includes(on)))
          throw Error('Default-off/SSR build retained a QA refusal interface');
        return;
      }
      if (!emitted || !transformed) throw Error('QA reader and transport were not composed in this client graph');
      const transportRows = chunks.flatMap(row => has(row, transport).map(id => ({ row, id })));
      const clientRows = chunks.flatMap(row => has(row, client).map(id => ({ row, id })));
      const readerRows = chunks.filter(row => row.facadeModuleId === virtualId);
      if (transportRows.length !== 1 || transportRows[0].id !== transport || clientRows.length !== 1 || clientRows[0].id !== client || readerRows.length !== 1)
        throw Error('QA client graph requires one canonical transport, application and reader module');
      const reader = chunks.find(row => row.fileName === uploadRefusalReaderFile), target = transportRows[0].row, app = clientRows[0].row;
      if (!reader || reader.facadeModuleId !== virtualId || readerRows[0] !== reader || reader.exports.length !== 1 || reader.exports[0] !== uploadRefusalReaderExport)
        throw Error('QA reader must be the exact sole-getter facade');
      const byFile = new Map(chunks.map(row => [row.fileName, row]));
      if (!reaches(byFile, reader.fileName, target.fileName) || !reaches(byFile, app.fileName, target.fileName)) throw Error('QA reader and application do not share their transport');
      const entries = chunks.filter(row => row.isEntry && row !== reader).map(row => row.fileName).sort();
      if (!entries.length || !entries.some(name => reaches(byFile, name, app.fileName, new Set(), true))) throw Error('QA application entry cannot reach the upload client');
      const selected = new Set();
      const visit = name => { if (selected.has(name)) return; const row = byFile.get(name); if (!row) throw Error('QA dependency escaped the client graph'); selected.add(name); [...row.imports, ...row.dynamicImports].forEach(visit); };
      visit(reader.fileName); visit(app.fileName); entries.forEach(visit);
      const graph = {
        schema: 1, enabled: true, plugin_source_sha256: hash(pluginSource), config_source_sha256: hash(configSource), reader_url: '/' + uploadRefusalReaderFile, graph_url: '/' + uploadRefusalGraphFile,
        reader_export: uploadRefusalReaderExport,
        transport_module_id: path.relative(root, transport).split(path.sep).join('/'), transport_source_sha256: hash(transportSource),
        client_module_id: path.relative(root, client).split(path.sep).join('/'), client_source_sha256: hash(clientSource),
        transport_module_count: 1, client_module_count: 1, reader_module_count: 1,
        reader_chunk: reader.fileName, transport_chunk: target.fileName, client_chunk: app.fileName,
        reader_reaches_transport: true, client_reaches_transport: true, client_entry_chunks: entries, entry_reaches_client: true,
        chunks: [...selected].sort().map(name => {
          const row = byFile.get(name);
          if (row.imports.length > 128 || row.dynamicImports.length > 128) throw Error('QA chunk imports exceed their finite bound');
          return { file_name: name, sha256: hash(row.code), imports: [...row.imports], dynamic_imports: [...new Set(row.dynamicImports)], exports: [...row.exports],
            contains_transport: has(row, transport).length === 1, contains_client: has(row, client).length === 1, contains_reader: row.facadeModuleId === virtualId };
        }),
      };
      const graphBytes = JSON.stringify(graph, null, 2) + '\n';
      if (Buffer.byteLength(graphBytes) > 2 * 1024 * 1024) throw Error('QA graph manifest exceeds its byte bound');
      this.emitFile({ type: 'asset', fileName: uploadRefusalGraphFile, source: graphBytes });
    } },
  };
}
