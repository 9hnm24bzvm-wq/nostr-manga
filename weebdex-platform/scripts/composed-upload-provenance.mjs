// One intentionally superseded native recipe. This is source association, not execution coverage.
import { createHash } from 'node:crypto';
import { publicUploadOverlayManifest } from '../platform-integrations/public-api/upload-overlays.mjs';
const recipePath = 'platform-integrations/frontend-behavior/upload-replacement.fragment.ts';
const recipeSha = 'accfcd3062c871b4639082fdf59f95fcd8f0ad5489e59f3b58662916db44186b';
const target = 'src/lib/components/upload/UploadChapterPages.svelte';
const beforePlugin = 'weebdex-reviewed-script-overlays', afterPlugin = 'recovered-public-signed-upload';
const hash = value => createHash('sha256').update(value).digest('hex');
const normalize = value => value.replace(/\r\n/g, '\n').trim();
export function associateSupersededUploadRecipe({ fragmentPath, fragmentBytes, entry, readSnapshot }) {
  if (fragmentPath !== recipePath) return null;
  if (!entry || entry.projection !== target || entry.canonical !== target) throw Error('Superseded upload recipe target changed');
  const indices = entry.stages.flatMap((stage,index) => stage.plugin === afterPlugin ? [index] : []);
  if (!indices.length) return null; // Gate off still requires ordinary final-source ancestry.
  if (indices.length !== 1 || indices[0] !== entry.stages.length - 1 || indices[0] < 1 || hash(fragmentBytes) !== recipeSha) throw Error('Superseded upload recipe identity changed');
  const index = indices[0], before = entry.stages[index-1], after = entry.stages[index];
  const spec = publicUploadOverlayManifest.find(file => file.path === target);
  if (!spec || before.plugin !== beforePlugin || before.sha256 !== spec.composed_sha256 || after.sha256 !== spec.output_sha256 || entry.production_composed_sha256 !== spec.output_sha256) throw Error('Superseded upload recipe exact stage changed');
  const snapshot = (stage,plugin) => {
    if (stage.snapshot !== `production-stages/${plugin}/${target}`) throw Error('Superseded upload recipe snapshot escaped exact stage');
    const code = readSnapshot(stage.snapshot);
    if (typeof code !== 'string' || hash(code) !== stage.sha256) throw Error('Superseded upload recipe snapshot bytes changed');
    return code;
  };
  const previous = snapshot(before,beforePlugin), final = snapshot(after,afterPlugin);
  const lines = [...new Set(normalize(String(fragmentBytes)).split('\n').map(line=>line.trim()).filter(line=>line.length>12))];
  const oldLines = new Set(normalize(previous).split('\n').map(line=>line.trim()));
  const matching = lines.filter(line=>oldLines.has(line)).length;
  if (!lines.length || matching / lines.length <= 0.5 || !final.includes('replaceUploadPage(session, target, file, owner, current)') || !final.includes('reconcileUploadReplacement(owner)') || final.includes('new HTTP<UploadSessionFile & { version: number }>')) throw Error('Superseded upload recipe reference/signed replacement changed');
  return { path:recipePath,sha256:recipeSha,standalone_check_root:false,active_in_final:false,
    qualification:'Exact prior native recipe is superseded by the public signed atomic replacement adapter; source association does not qualify actual HTTP or browser behavior',
    historical_origin:{projection:target,plugin:beforePlugin,snapshot:before.snapshot,sha256:before.sha256,matching_recipe_lines:matching,distinctive_recipe_lines:lines.length},
    superseded_by:{plugin:afterPlugin,snapshot:after.snapshot,input_sha256:spec.composed_sha256,output_sha256:spec.output_sha256} };
}
