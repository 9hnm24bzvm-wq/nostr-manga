// Exact comparison only. Never substitute reconstructed source into runtime/build inputs.
import fs from 'node:fs';
import {createHash} from 'node:crypto';

export const localPageDriverPath = 'weebdex-platform/tests/ci/test_signed_feature_source_pins.py';
export const localPageDriverAssociationSha256 = '2764693600cde177426c8e948ba0dea2f7f7d97cc5f6ade7ab472078d9c87148';
const metadata = fs.readFileSync(new URL('../../docs/fixtures/signed-local-page-driver-source-association.json', import.meta.url));
const preservedMetadata = fs.readFileSync(new URL('../../docs/fixtures/signed-local-page-source-associations.json', import.meta.url));
const digest = value => createHash('sha256').update(value).digest('hex');
function requireValue(condition) {
  if (!condition) throw Error('Unknown reviewed local page driver source association');
}
export function readLocalPageDriverAssociation(bytes = metadata) {
  requireValue(Buffer.isBuffer(bytes) && digest(bytes) === localPageDriverAssociationSha256);
  const value = JSON.parse(bytes.toString('utf8'));
  const preserved = value.preserved_local_page_metadata;
  requireValue(preservedMetadata.length === preserved.bytes && digest(preservedMetadata) === preserved.sha256);
  return value;
}
export function restoreLocalPageDriverSource(file, source, bytes = metadata) {
  const row = readLocalPageDriverAssociation(bytes).row;
  requireValue(typeof file === 'string' && file === localPageDriverPath && row.path === file);
  requireValue(Buffer.isBuffer(source) || typeof source === 'string');
  requireValue(Buffer.byteLength(source) === row.current_bytes && digest(source) === row.current_sha256);
  let comparison = Buffer.isBuffer(source) ? source.toString('utf8') : source;
  for (const edit of row.literal_reversals) {
    requireValue(edit.current.length > 0 && comparison.split(edit.current).length === 2);
    comparison = comparison.replace(edit.current, () => edit.previous);
  }
  requireValue(Buffer.byteLength(comparison) === row.previous_bytes && digest(comparison) === row.previous_sha256);
  return Buffer.isBuffer(source) ? Buffer.from(comparison, 'utf8') : comparison;
}
