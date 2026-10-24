// Source associations only. Never substitute these historical bytes for product execution.
import fs from 'node:fs';
import {createHash} from 'node:crypto';

export const nativeSourceAssociationSha256 = '9875321067b77992942fe50203c19a79c39d695c1db443cce3b70361bd9084c7';
const associationBytes = fs.readFileSync(new URL('../../docs/fixtures/native-catalogue-source-associations.json', import.meta.url));
const hash = value => createHash('sha256').update(value).digest('hex');
function insist(value, detail) { if (!value) throw Error('Unknown current source bytes: ' + detail); }
export function readNativeSourceAssociation(bytes = associationBytes) {
  insist(hash(bytes) === nativeSourceAssociationSha256, 'native association metadata');
  return JSON.parse(bytes);
}
const paths = readNativeSourceAssociation().read_fanout.rows.map(row => row.path);
export const nativeReadSuccessorPaths = Object.freeze(paths);

function restore(source, row) {
  insist(typeof source === 'string' || Buffer.isBuffer(source), 'source type');
  insist(hash(source) === row.current_sha256, 'current digest');
  let output = source.toString();
  insist(Buffer.byteLength(output) === row.current_bytes, 'current byte count');
  for (const {current, previous} of row.literal_reversals) {
    insist(current && previous && current !== previous && output.split(current).length === 2, 'exact once-only literal reversal');
    output = output.replace(current, () => previous);
  }
  insist(Buffer.byteLength(output) === row.previous_bytes && hash(output) === row.previous_sha256, 'historical reconstruction');
  return output;
}

export function restoreNativeReadSource(file, source, bytes = associationBytes) {
  const row = readNativeSourceAssociation(bytes).read_fanout.rows.find(row => row.path === file);
  insist(row !== undefined, 'unknown read source path');
  return restore(source, row);
}

export function restoreNativeTransportSource(source, mode, bytes = associationBytes) {
  insist(mode === 'raw' || mode === 'qa_stripped', 'unknown transport mode');
  return restore(source, readNativeSourceAssociation(bytes).transport[mode]);
}

// Explicit response-binding comparison stage. Never apply to runtime inputs.
const siteClientBindingBytes = fs.readFileSync(new URL('../../docs/fixtures/native-site-client-binding-source-successor.json', import.meta.url));
export const nativeSiteClientBindingAssociationSha256 = '7d2e4d97e41ce5f323b2e72a6c83ae68ce9a5f1894c07b82b5cd2e8c2be046c2';
export const nativeSiteClientBindingStage = 'badge-response-binding-to-frozen-native:platform-integrations/public-api/site-client.ts';
export const nativeSiteClientBindingPath = 'weebdex-platform/platform-integrations/public-api/site-client.ts';
export function readNativeSiteClientBindingAssociation(bytes = siteClientBindingBytes) {
  insist(Buffer.isBuffer(bytes) && bytes.length === 2068 && hash(bytes) === nativeSiteClientBindingAssociationSha256, 'site-client binding metadata');
  return JSON.parse(bytes.toString('utf8'));
}
export function restoreNativeSiteClientBindingSource(stage, file, source, bytes = siteClientBindingBytes) {
  const row = readNativeSiteClientBindingAssociation(bytes);
  insist(stage === nativeSiteClientBindingStage && file === nativeSiteClientBindingPath && row.stage === stage && row.path === file, 'site-client binding stage or path');
  insist(typeof source === 'string' || Buffer.isBuffer(source), 'site-client binding source type');
  const current = Buffer.from(source);
  insist(current.length === row.current_bytes && hash(current) === row.current_sha256, 'site-client binding current digest');
  const native = readNativeSourceAssociation().read_fanout.rows.find(value => value.path === file);
  insist(native && row.previous_bytes === native.current_bytes && row.previous_sha256 === native.current_sha256 && row.native_previous_bytes === native.previous_bytes && row.native_previous_sha256 === native.previous_sha256, 'site-client binding predecessor contract');
  let output = current.toString('utf8'), cursor = 0;
  for (const [index, edit] of row.literal_reversals.entries()) {
    const position = output.indexOf(edit.current);
    insist(edit.ordinal === index + 1 && edit.current && edit.previous && edit.current !== edit.previous && output.split(edit.current).length === 2 && position >= cursor && Buffer.byteLength(edit.current) === edit.current_bytes && hash(edit.current) === edit.current_literal_sha256 && Buffer.byteLength(edit.previous) === edit.previous_bytes && hash(edit.previous) === edit.previous_literal_sha256, 'site-client binding literal or order');
    output = output.slice(0, position) + edit.previous + output.slice(position + edit.current.length);
    cursor = position + edit.previous.length;
  }
  insist(Buffer.byteLength(output) === row.previous_bytes && hash(output) === row.previous_sha256, 'site-client binding predecessor digest');
  return typeof source === 'string' ? output : Buffer.from(output);
}
