// Source-only fixture support. Exact current/retained paths, never a directory
// fallback. This is application-input verification, not OS-wide isolation.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { syncBuiltinESMExports } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
const digest = value => createHash('sha256').update(value).digest('hex');
export function mountedSources(root) {
  const fixture = process.env.WEEBDEX_BOUNDED_MOUNTED_FIXTURE === '1';
  if (!fixture) return { frontend: path.join(root, 'frontend'), relative: file => path.relative(root, file),
    read: relative => fs.readFileSync(path.join(root, relative)), load: relative => import(pathToFileURL(path.join(root, relative)).href), verify() { return true; } };
  const manifestPath = process.env.WEEBDEX_MOUNTED_SOURCE_MANIFEST, expected = process.env.WEEBDEX_MOUNTED_SOURCE_MANIFEST_SHA256;
  if (!manifestPath || !path.isAbsolute(manifestPath) || !/^[0-9a-f]{64}$/.test(expected ?? '')) throw Error('Exact mounted source manifest is required');
  const originalRead = fs.readFileSync.bind(fs), raw = originalRead(manifestPath);
  if (digest(raw) !== expected) throw Error('Mounted source manifest changed');
  const manifest = JSON.parse(raw), entries = manifest.applicationSources;
  if (manifest.schema !== 1 || !Array.isArray(entries) || !entries.length || entries.length > 500 || !path.isAbsolute(manifest.currentRoot) || !path.isAbsolute(manifest.retainedRoot)) throw Error('Invalid mounted source mapping');
  if (root !== manifest.currentRoot || manifest.currentRoot === manifest.retainedRoot) throw Error('Wrong mounted source roots');
  const byName = new Map(), byPath = new Map();
  for (const row of entries) {
    if (!row || !['current', 'retained'].includes(row.kind) || typeof row.relative !== 'string' || !row.relative || row.relative.split('/').some(part => !part || part === '.' || part === '..') || path.isAbsolute(row.relative) || !/^[0-9a-f]{64}$/.test(row.sha256)) throw Error('Invalid exact mounted source row');
    const file = path.join(row.kind === 'current' ? manifest.currentRoot : manifest.retainedRoot, row.relative);
    if (byName.has(row.relative) || byPath.has(file)) throw Error('Duplicate mounted source row');
    byName.set(row.relative, { file, sha256: row.sha256 }); byPath.set(file, { relative: row.relative, sha256: row.sha256 });
  }
  const checked = file => {
    const row = byPath.get(file);
    if (!row || fs.lstatSync(file).isSymbolicLink() || fs.realpathSync(file) !== file) throw Error('Unlisted mounted application input');
    const bytes = originalRead(file); if (digest(bytes) !== row.sha256) throw Error('Mounted application input changed'); return bytes;
  };
  // Exact native overlay constructors read their immutable source and metadata
  // directly; keep those reads under the same allowlist and byte checks.
  fs.readFileSync = function(input, options) {
    const file = input instanceof URL ? fileURLToPath(input) : typeof input === 'string' ? path.resolve(input) : undefined;
    if (file && [manifest.currentRoot, manifest.retainedRoot].some(base => file === base || file.startsWith(base + path.sep))) {
      const bytes = checked(file), encoding = typeof options === 'string' ? options : options?.encoding;
      return encoding ? bytes.toString(encoding) : bytes;
    }
    return originalRead(input, options);
  };
  syncBuiltinESMExports();
  const selected = relative => { const row = byName.get(relative); if (!row) throw Error('No exact mapped mounted source'); return row.file; };
  const verify = () => { if (digest(originalRead(manifestPath)) !== expected) throw Error('Mounted manifest changed'); for (const file of byPath.keys()) checked(file); return true; };
  verify();
  return { frontend: path.join(manifest.retainedRoot, 'frontend'),
    relative(file) { const row = byPath.get(file); if (row) return row.relative; const relative = path.relative(root, file); selected(relative); return relative; },
    read: relative => checked(selected(relative)),
    async load(relative) { const file = selected(relative); checked(file); return import(pathToFileURL(file).href); }, verify };
}
