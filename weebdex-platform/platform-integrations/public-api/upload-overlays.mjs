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
// Additive native output ancestry: exact reviewed successor bytes only. This
// restores source provenance, never claims the historical tests ran native UX.
export const nativeUploadOverlayPatches = Object.freeze((specification.native_patches ?? []).map(row => Object.freeze({...row})));
export const nativeUploadOverlayManifest = Object.freeze(specification.files.filter(row=>row.native_output_sha256).map(row=>Object.freeze({path:row.path,predecessor_sha256:row.output_sha256,output_sha256:row.native_output_sha256})));
export function restoreNativeUploadOutput(finalCode, relative) {
 const file=specification.files.find(row=>row.path===relative);
 if(!file)throw Error(`Unknown upload source: ${relative}`);
 if(hash(finalCode)!==(file.native_output_sha256??file.output_sha256))throw Error(`Native upload successor source changed: ${relative}`);
 let code=finalCode;
 for(const patch of [...nativeUploadOverlayPatches.filter(row=>row.path===relative)].reverse()){
  if(code.split(patch.after).length-1!==patch.count)throw Error(`Native upload reverse anchor changed: ${patch.label}`);
  code=code.split(patch.after).join(patch.before);
 }
 if(hash(code)!==file.output_sha256)throw Error(`Native upload predecessor changed: ${relative}`);
 return code;
}

export function applyNativeUploadOverlay(input,relative){
 const file=specification.files.find(row=>row.path===relative);
 if(!file?.native_output_sha256)return null;
 if(hash(input)!==file.output_sha256)throw Error(`Native upload predecessor input changed: ${relative}`);
 let code=input;
  for (const patch of (specification.native_patches ?? []).filter(patch => patch.path === relative)) {
    if (code.split(patch.before).length - 1 !== patch.count) throw Error(`Native upload successor anchor changed: ${patch.label}`);
    code = code.split(patch.before).join(patch.after);
  }
  if (hash(code) !== (file.native_output_sha256 ?? file.output_sha256)) throw Error(`Native upload successor output changed: ${relative}`);
 return code;
}
export function createNativeUploadOverlays(frontend,{enabled=readPublicApiGate()}={}){
 if(typeof enabled!=='boolean')throw Error('Native upload overlay enabled must be boolean');
 const root=path.resolve(frontend);
 return{name:'native-browser-account-upload-successor',enforce:'pre',transform:{order:'pre',handler(input,id){
  if(!enabled||id.includes('?'))return null;
  const relative=path.relative(root,id).split(path.sep).join('/'),file=specification.files.find(row=>row.path===relative);
  if(!file?.native_output_sha256)return null;
  if(hash(fs.readFileSync(path.join(root,relative)))!==file.source_sha256)throw Error(`Native upload original attachment changed: ${relative}`);
  return{code:applyNativeUploadOverlay(input,relative),map:null};
 }}};
}
