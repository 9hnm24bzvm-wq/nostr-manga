// Passive DOM-attribute stand-ins only. No browser, network or application hook.
import { captureUploadPreviewRefusals, projectUploadPreviewRefusals, uploadPreviewRefusalCodes } from '../../scripts/public-upload-recovery-browser.mjs';
export function previewPage(codes, options = {}) {
  return { locator(selector) {
    if (selector !== 'p[role="status"][data-public-upload-preview-refusal]') throw Error('Unexpected finite selector');
    return { async evaluateAll(read) {
      if (options.error) throw options.error;
      if (options.pending) return new Promise(() => {});
      return read(codes.map(code => ({ getAttribute(name) {
        if (name !== 'data-public-upload-preview-refusal') throw Error('Unexpected finite attribute');
        return code;
      } })));
    } };
  } };
}
export async function previewRefusalFixtures() {
  const value = {};
  for (const code of uploadPreviewRefusalCodes) value[code] = await captureUploadPreviewRefusals(previewPage([code]));
  value.empty = await captureUploadPreviewRefusals(previewPage([]));
  value.truncated = await captureUploadPreviewRefusals(previewPage(Array(17).fill('body-read')));
  value.private = await captureUploadPreviewRefusals(previewPage(['reserved-private-value']));
  value.missing = projectUploadPreviewRefusals(undefined);
  value.read_error = await captureUploadPreviewRefusals(previewPage([], { error: Error('reserved-private-failure') }));
  return value;
}
if (process.argv.includes('--json')) process.stdout.write(JSON.stringify(await previewRefusalFixtures())+'\n');
