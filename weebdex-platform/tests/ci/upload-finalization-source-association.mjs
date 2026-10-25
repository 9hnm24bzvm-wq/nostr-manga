// Exact source-only historical association. No old runtime verdict is upgraded.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const sha = value => createHash('sha256').update(value).digest('hex');
export function restoreUploadFinalization(value) {
  if (sha(value) === '88f4ecc94ecd31cd4cb6c033e4be22d398c39f538e6b021b798b9baa00d2d95a') return value;
  assert.equal(sha(value), '6275d3b60e9bed4e355406a51c8c645caa839900f5c3d3178ec1745eb9de6c95', 'Unknown upload finalization successor');
  const [current, previous] = ["    result.preview_transport_refusals = await captureUploadPreviewRefusals(page);\n    try {\n      if (result.completed) await until(() => {\n        // Captures above can yield to late signed reads. Check and detach in the\n        // same synchronous turn, so no admitted row misses its native terminal.\n        if (!observer.settled()) return false;\n        assert.equal(observer.mutationCount(), 21, 'Unexpected final UI mutation');\n        observer.close(); return true;\n      }, Boolean, 'Final signed browser closure');\n    } finally {\n      try { observer.close(); reader.close(); } finally { result.browser_http = observer.snapshot(); result.independent_http = reader.snapshot();\n        secret?.fill(0); secret = undefined; secretHex = ''; for (const image of [...images, cancelImage, replacementImage]) image.buffer.fill(0); save(); }\n    }\n    // The outer safe-error serializer still owns its redaction entry until the\n", "    result.preview_transport_refusals = await captureUploadPreviewRefusals(page);\n    try { observer.close(); reader.close(); } finally { result.browser_http = observer.snapshot(); result.independent_http = reader.snapshot();\n      secret?.fill(0); secret = undefined; secretHex = ''; for (const image of [...images, cancelImage, replacementImage]) image.buffer.fill(0); save(); }\n    // The outer safe-error serializer still owns its redaction entry until the\n"];
  assert.equal(value.split(current).length - 1, 1);
  value = value.replace(current, previous);
  assert.equal(sha(value), '88f4ecc94ecd31cd4cb6c033e4be22d398c39f538e6b021b798b9baa00d2d95a');
  return value;
}
