// Comparison only. Never pass reconstructed bytes to a VM, runtime or build.
// The older language/scheduling helpers and metadata remain byte-identical.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {restoreReviewedLanguageSource as historicalLanguageSource} from './reviewed-language-source-successors.mjs';

export const localPageAssociationSha256 = 'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c';
const metadata = fs.readFileSync(new URL('../../docs/fixtures/signed-local-page-source-associations.json', import.meta.url));
const digest = value => createHash('sha256').update(value).digest('hex');
function requireValue(condition, detail) {
  if (!condition) throw Error('Unknown reviewed local page source: ' + detail);
}
export function readLocalPageSourceAssociation(bytes = metadata) {
  requireValue(Buffer.isBuffer(bytes) && digest(bytes) === localPageAssociationSha256, 'metadata');
  return JSON.parse(bytes.toString('utf8'));
}
export function restoreLocalPageSource(source, kind, bytes = metadata) {
  const row = readLocalPageSourceAssociation(bytes).rows.find(value => value.kind === kind);
  requireValue(typeof kind === 'string' && row !== undefined, 'kind');
  requireValue(typeof source === 'string' && Buffer.byteLength(source) === row.current_bytes && digest(source) === row.current_sha256, 'current bytes');
  let comparison = source;
  for (const edit of row.literal_reversals) {
    requireValue(edit.current.length > 0 && comparison.split(edit.current).length === 2, 'unique literal');
    comparison = comparison.replace(edit.current, () => edit.previous);
  }
  requireValue(Buffer.byteLength(comparison) === row.previous_bytes && digest(comparison) === row.previous_sha256, 'complete predecessor bytes');
  return comparison;
}
// Compatibility entrypoint for digest-only consumers. Execution still uses their
// original, unmodified source variable, never this returned comparison string.
export function restoreReviewedLanguageSource(source, kind) {
  return historicalLanguageSource(restoreLocalPageSource(source, kind), kind);
}
