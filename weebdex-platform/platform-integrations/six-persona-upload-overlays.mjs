import {readFileSync} from 'node:fs';
export const uploadCompletionPatches=Object.freeze(JSON.parse(readFileSync(new URL('./six-persona-upload/patches.json',import.meta.url),'utf8')));
export function applyUploadCompletionPatches(path,source){for(const patch of uploadCompletionPatches.filter(row=>row.path===path)){if(source.split(patch.before).length-1!==patch.count)throw Error('Upload completion predecessor mismatch: '+patch.label);source=source.split(patch.before).join(patch.after);}return source;}
