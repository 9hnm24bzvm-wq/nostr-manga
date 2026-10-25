// Default-off signed public upload integration over immutable recovered attachment bytes.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { readPublicApiGate } from './overlays.mjs';
const specification = JSON.parse(fs.readFileSync(new URL('./upload-anchors.json', import.meta.url), 'utf8'));
const hash = value => createHash('sha256').update(value).digest('hex');
export const publicUploadOverlayManifest = Object.freeze(specification.files.map(file => Object.freeze({ ...file })));
export function applyPublicUploadOverlay(input, relative) {
  const file = specification.files.find(file => file.path === relative);
  if (!file) return null;
  if (hash(input) !== file.composed_sha256) throw Error(`Public upload composed source changed: ${relative}`);
  let code = input;
  for (const patch of specification.patches.filter(patch => patch.path === relative)) {
    if (code.split(patch.before).length - 1 !== patch.count) throw Error(`Public upload exact anchor changed: ${patch.label}`);
    code = code.split(patch.before).join(patch.after);
  }
  if (hash(code) !== file.output_sha256) throw Error(`Public upload derived output changed: ${relative}`);
  return code;
}
export function createPublicUploadOverlays(frontend, { enabled = readPublicApiGate() } = {}) {
  if (typeof enabled !== 'boolean') throw Error('Public upload overlay enabled must be boolean');
  const root = path.resolve(frontend);
  return { name: 'recovered-public-signed-upload', enforce: 'pre', transform: { order: 'pre', handler(input, id) {
    if (!enabled || id.includes('?')) return null;
    const relative = path.relative(root, id).split(path.sep).join('/');
    const file = specification.files.find(file => file.path === relative);
    if (!file) return null;
    if (hash(fs.readFileSync(path.join(root, relative))) !== file.source_sha256) throw Error(`Public upload original attachment changed: ${relative}`);
    return { code: applyPublicUploadOverlay(input, relative), map: null };
  } } };
}
