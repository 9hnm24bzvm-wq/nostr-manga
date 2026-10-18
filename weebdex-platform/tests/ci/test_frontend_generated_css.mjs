#!/usr/bin/env node
// Actual-artifact consumer, run only after the fresh pinned Vite builds. This is
// neither a browser nor a finite source-only CSS substitute.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import test from 'node:test';
test('actual generated CSS contains the integration-only native dialog width utility', () => {
  const artifact = process.env.WEEBDEX_FRONTEND_ARTIFACT_ROOT;
  const manifestPath = process.env.WEEBDEX_FRONTEND_ARTIFACT_MANIFEST;
  assert(artifact && manifestPath, 'Set exact fresh Vite artifact root and retained artifact manifest; no historical default');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const files = manifest.files.filter(row => row.path.startsWith('_app/immutable/assets/') && row.path.endsWith('.css'));
  assert(files.length > 0, 'Actual artifact manifest must identify emitted CSS');
  const css = files.map(row => {
    assert(!row.path.split('/').includes('..') && !path.isAbsolute(row.path));
    const bytes = fs.readFileSync(path.join(artifact, row.path));
    assert.equal(bytes.length, row.bytes);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), row.sha256);
    return bytes.toString('utf8');
  }).join('\n');
  assert(css.includes('.\\!max-w-\\[min\\(42rem\\,calc\\(100dvw-2rem\\)\\)\\]'), 'Missing external integration utility selector');
  assert.match(css, /max-width:min\(42rem,100dvw - 2rem\)!important/);
  assert(!css.includes('/workspace/'), 'Build-local scan paths must not leak into static CSS');
});
